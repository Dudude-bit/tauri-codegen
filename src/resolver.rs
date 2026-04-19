//! Module resolver - resolves types based on imports and module structure
//!
//! Handles:
//! - Local type definitions
//! - Explicit imports (use foo::Bar)
//! - Wildcard imports (use foo::*)
//! - Relative paths (super::Bar, crate::foo::Bar)
//! - Ambiguity detection

mod helpers;
mod imports;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use helpers::are_siblings;

/// Maps alias names to their original type names
type AliasMap = HashMap<String, String>;

/// A module path represented as a list of path segments
type ModulePath = Vec<String>;

/// Result of a type resolution attempt
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionResult {
    /// Successfully resolved to a single file
    Found(PathBuf),
    /// Successfully resolved with alias mapping (file, original_type_name)
    /// Used when resolving `pub use X as Y` - returns the file and original name X
    FoundWithAlias(PathBuf, String),
    /// Type not found
    NotFound,
    /// Ambiguous: found in multiple files
    Ambiguous(Vec<PathBuf>),
}

/// Represents a parsed file with its imports and local types
#[derive(Debug, Default)]
pub struct FileScope {
    /// Module path (e.g., ["crate", "commands"] for src/commands.rs)
    pub module_path: ModulePath,
    /// Types defined locally in this file (name -> kind)
    pub local_types: HashMap<String, TypeKind>,
    /// Imports: local name -> full path
    pub imports: HashMap<String, ImportedType>,
    /// Wildcard imports (use something::*)
    pub wildcard_imports: Vec<ModulePath>,
    /// Type aliases: alias name -> base type name (e.g., "AppStateMutexed" -> "State")
    pub type_aliases: AliasMap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone)]
pub struct ImportedType {
    /// Full module path (e.g., ["crate", "internal", "UserRole"])
    pub path: ModulePath,
}

/// Module resolver that tracks all files and their scopes.
///
/// Fields are crate-visible so the `resolver::imports` submodule and
/// in-crate tests can read them; external users of the library can't,
/// and must go through the `resolve_*`/`register_*` accessors. This
/// gives us a single place to maintain the invariant that
/// `type_definitions` and `files[_].local_types` stay consistent.
#[derive(Debug, Default)]
pub struct ModuleResolver {
    /// File path -> FileScope
    pub(crate) files: HashMap<PathBuf, FileScope>,
    /// Type name -> list of files that define it
    pub(crate) type_definitions: HashMap<String, Vec<PathBuf>>,
    /// Module path -> file path (e.g., ["crate", "internal"] -> src/internal.rs)
    pub(crate) module_to_file: HashMap<Vec<String>, PathBuf>,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Source files that define a type with the given simple name. Returns
    /// `None` if the name was never registered. Borrowed slice so callers
    /// don't have to clone.
    pub fn type_definitions_for(&self, name: &str) -> Option<&[PathBuf]> {
        self.type_definitions.get(name).map(|v| v.as_slice())
    }

    /// Read-only access to the scope recorded for a given source file.
    pub fn file_scope(&self, path: &Path) -> Option<&FileScope> {
        self.files.get(path)
    }

    /// Resolve a type name in the context of a specific file
    pub fn resolve_type(&self, type_path: &str, from_file: &Path) -> ResolutionResult {
        let segments: Vec<&str> = type_path.split("::").filter(|s| !s.is_empty()).collect();
        let type_name = segments.last().copied().unwrap_or("");

        let scope = match self.files.get(from_file) {
            Some(s) => s,
            None => {
                // No scope for this file (e.g., <cargo-expand>)
                // Try to resolve from global type_definitions
                return self.try_resolve_from_definitions(type_name);
            }
        };

        // Handle simple name (no ::)
        if segments.len() == 1 {
            let name = segments[0];
            return self.resolve_simple_name(name, scope, from_file);
        }

        // Handle path (foo::Bar, super::Bar, crate::foo::Bar)
        self.resolve_path(&segments, scope)
    }

    fn resolve_simple_name(
        &self,
        name: &str,
        scope: &FileScope,
        from_file: &Path,
    ) -> ResolutionResult {
        // 1. Check local definition
        if scope.local_types.contains_key(name) {
            return ResolutionResult::Found(from_file.to_path_buf());
        }

        // 2. Check explicit imports
        if let Some(imported) = scope.imports.get(name) {
            let result = self.resolve_module_path(&imported.path);
            return self.wrap_alias_if_needed(result, name, &imported.path);
        }

        // 3. Check wildcard imports
        for wildcard_path in &scope.wildcard_imports {
            // Normalize relative path to absolute path
            let full_path = self.normalize_relative_path(wildcard_path, &scope.module_path);
            if let Some(file) = self.find_type_in_module(name, &full_path) {
                return ResolutionResult::Found(file);
            }
        }

        // 4. Fallback: Lookup by name in entire workspace (Ambiguity Check)
        if let Some(locations) = self.type_definitions.get(name) {
            if locations.len() == 1 {
                return ResolutionResult::Found(locations[0].clone());
            }
            // If multiple found, try to filter by proximity or return ambiguous
            let from_module = &scope.module_path;

            // Prioritize siblings (same parent module)
            let siblings: Vec<_> = locations
                .iter()
                .filter(|loc| {
                    if let Some(loc_scope) = self.files.get(*loc) {
                        are_siblings(&loc_scope.module_path, from_module)
                    } else {
                        false
                    }
                })
                .collect();

            if siblings.len() == 1 {
                return ResolutionResult::Found(siblings[0].clone());
            }

            return ResolutionResult::Ambiguous(locations.clone());
        }

        ResolutionResult::NotFound
    }

    /// Wrap resolution result with alias information if the import used a rename
    fn wrap_alias_if_needed(
        &self,
        result: ResolutionResult,
        import_name: &str,
        import_path: &[String],
    ) -> ResolutionResult {
        let original_name = import_path.last().map(|s| s.as_str()).unwrap_or("");
        let is_alias = original_name != import_name && !original_name.is_empty();

        if is_alias {
            match result {
                ResolutionResult::Found(path) => {
                    ResolutionResult::FoundWithAlias(path, original_name.to_string())
                }
                other => other,
            }
        } else {
            result
        }
    }

    fn resolve_path(&self, segments: &[&str], scope: &FileScope) -> ResolutionResult {
        let first = segments[0];

        // 1. Check if the first segment is an imported alias/module
        if let Some(imported) = scope.imports.get(first) {
            // e.g. use crate::utils::wrapper; AND path is wrapper::MyType
            // imported.path = ["crate", "utils", "wrapper"]
            // result path = ["crate", "utils", "wrapper", "MyType"]
            let mut full_path = imported.path.clone();
            full_path.extend(segments[1..].iter().map(|s| s.to_string()));
            return self.resolve_module_path(&full_path);
        }

        // 2. Standard canonical path resolution
        let path_result = self.resolve_canonical_path(segments, scope);
        match path_result {
            Some(path) => self.resolve_module_path(&path),
            None => ResolutionResult::NotFound,
        }
    }

    // Resolve any path tokens to an absolute module path ["crate", "foo", "Type"]
    fn resolve_canonical_path(&self, segments: &[&str], scope: &FileScope) -> Option<Vec<String>> {
        let mut current_path = if segments[0] == "crate" {
            vec!["crate".to_string()]
        } else if segments[0] == "super" || segments[0] == "self" {
            scope.module_path.clone()
        } else {
            // Implicit relative path: `submod::Type` -> start from current module
            scope.module_path.clone()
        };

        let iter_start = if segments[0] == "crate" { 1 } else { 0 };

        for segment in &segments[iter_start..] {
            match *segment {
                "super" => {
                    if current_path.len() > 1 {
                        current_path.pop();
                    } else {
                        // Cannot go above root
                        return None;
                    }
                }
                "self" => {
                    // Stay at current
                }
                name => {
                    current_path.push(name.to_string());
                }
            }
        }
        Some(current_path)
    }

    /// Resolve an absolute path (["crate", "mod", "Type"]) to a file
    fn resolve_module_path(&self, module_path: &[String]) -> ResolutionResult {
        if module_path.len() < 2 {
            return ResolutionResult::NotFound;
        }

        // Split into module part and type part
        // path: [crate, mod, Type] -> check crate/mod.rs for Type
        let type_name = &module_path[module_path.len() - 1];
        let mod_path = &module_path[..module_path.len() - 1];

        if let Some(file_path) = self.module_to_file.get(mod_path) {
            if let Some(scope) = self.files.get(file_path) {
                // 1. Check local definition
                if scope.local_types.contains_key(type_name) {
                    return ResolutionResult::Found(file_path.clone());
                }

                // 2. Check re-exports (imports via pub use or use)
                if let Some(imported) = scope.imports.get(type_name) {
                    // Recursively resolve the imported path relative to THIS module
                    let segments: Vec<&str> = imported.path.iter().map(|s| s.as_str()).collect();
                    let result = self.resolve_path(&segments, scope);
                    return self.wrap_alias_if_needed(result, type_name, &imported.path);
                }

                // 3. Check wildcard re-exports (pub use submod::*)
                for wildcard_path in &scope.wildcard_imports {
                    // Normalize relative path to absolute path
                    let full_path = self.normalize_relative_path(wildcard_path, &scope.module_path);
                    if let Some(found_file) = self.find_type_in_module(type_name, &full_path) {
                        return ResolutionResult::Found(found_file);
                    }
                }
            }
        }

        // 4. Fallback: check type_definitions for types from cargo expand
        // This handles cases where the type is generated by a macro and registered globally
        self.try_resolve_from_definitions(type_name)
    }

    /// Try to resolve a type from global type_definitions (cargo expand types)
    fn try_resolve_from_definitions(&self, type_name: &str) -> ResolutionResult {
        if let Some(locations) = self.type_definitions.get(type_name) {
            if locations.len() == 1 {
                return ResolutionResult::Found(locations[0].clone());
            } else if !locations.is_empty() {
                return ResolutionResult::Ambiguous(locations.clone());
            }
        }
        ResolutionResult::NotFound
    }

    /// Normalize a relative module path to an absolute path
    /// e.g., ["types"] with context ["crate", "resources"] -> ["crate", "resources", "types"]
    fn normalize_relative_path(
        &self,
        relative_path: &[String],
        from_module: &[String],
    ) -> Vec<String> {
        if relative_path.is_empty() {
            return from_module.to_vec();
        }

        // If path starts with "crate", it's already absolute
        if relative_path.first().map(|s| s.as_str()) == Some("crate") {
            return relative_path.to_vec();
        }

        // Handle super:: and self:: in relative paths
        let mut result = from_module.to_vec();
        for segment in relative_path {
            match segment.as_str() {
                "super" => {
                    if result.len() > 1 {
                        result.pop();
                    }
                }
                "self" => {
                    // Stay at current module
                }
                name => {
                    result.push(name.to_string());
                }
            }
        }
        result
    }

    /// Find type in module (for wildcard imports)
    fn find_type_in_module(&self, type_name: &str, module_path: &[String]) -> Option<PathBuf> {
        if let Some(file_path) = self.module_to_file.get(module_path) {
            if let Some(scope) = self.files.get(file_path) {
                if scope.local_types.contains_key(type_name) {
                    return Some(file_path.clone());
                }
            }
        }
        None
    }

    /// Resolve a type alias to its final target base type name (follows alias chains)
    /// For example, given "AliasedState" where:
    ///   `type MyState<'a> = State<'a, AppState>;`
    ///   `type AliasedState<'a> = MyState<'a>;`
    /// Returns Some("State")
    pub fn resolve_alias_target(&self, type_name: &str, from_file: &Path) -> Option<String> {
        let mut current = type_name.to_string();
        let mut found_any = false;

        // Follow the alias chain (max 10 iterations to prevent infinite loops)
        for _ in 0..10 {
            let next = self.resolve_single_alias(&current, from_file);
            match next {
                Some(target) => {
                    found_any = true;
                    current = target;
                }
                None => break,
            }
        }

        if found_any {
            Some(current)
        } else {
            None
        }
    }

    /// Resolve a single level of type alias (no recursion)
    fn resolve_single_alias(&self, type_name: &str, from_file: &Path) -> Option<String> {
        if let Some(scope) = self.files.get(from_file) {
            // Check local type aliases first
            if let Some(target) = scope.type_aliases.get(type_name) {
                return Some(target.clone());
            }

            // Check imported type aliases
            if let Some(imported) = scope.imports.get(type_name) {
                if let ResolutionResult::Found(source_file) =
                    self.resolve_module_path(&imported.path)
                {
                    if let Some(source_scope) = self.files.get(&source_file) {
                        let actual_name = imported
                            .path
                            .last()
                            .map(|s| s.as_str())
                            .unwrap_or(type_name);
                        if let Some(target) = source_scope.type_aliases.get(actual_name) {
                            return Some(target.clone());
                        }
                    }
                }
            }
        }

        // Try to find in any file (for global lookup)
        for (file_path, scope) in &self.files {
            if file_path != from_file {
                if let Some(target) = scope.type_aliases.get(type_name) {
                    return Some(target.clone());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests;
