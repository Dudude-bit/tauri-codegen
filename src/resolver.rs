//! Module resolver - resolves types based on imports and module structure
//!
//! Handles:
//! - Local type definitions
//! - Explicit imports (use foo::Bar)
//! - Wildcard imports (use foo::*)
//! - Relative paths (super::Bar, crate::foo::Bar)
//! - Ambiguity detection

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use syn::{Item, UseTree};

/// Maps type names to their source file locations
pub type TypeLocations = HashMap<String, Vec<PathBuf>>;

/// Maps alias names to their original type names
pub type AliasMap = HashMap<String, String>;

/// A module path represented as a list of path segments
pub type ModulePath = Vec<String>;

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

/// Module resolver that tracks all files and their scopes
#[derive(Debug, Default)]
pub struct ModuleResolver {
    /// File path -> FileScope
    pub files: HashMap<PathBuf, FileScope>,
    /// Type name -> list of files that define it
    pub type_definitions: HashMap<String, Vec<PathBuf>>,
    /// Module path -> file path (e.g., ["crate", "internal"] -> src/internal.rs)
    pub module_to_file: HashMap<Vec<String>, PathBuf>,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a type from cargo expand output, but ONLY if it doesn't already exist
    /// in type_definitions from a real source file.
    ///
    /// This prevents the "ambiguous type" bug where types defined in source files
    /// would get a duplicate entry from cargo-expand, while still allowing
    /// macro-generated types (that don't exist in source) to be resolved.
    pub fn register_expanded_type_if_missing(&mut self, type_name: &str, source_path: &Path) {
        // Only register if this type hasn't been seen in any source file
        if !self.type_definitions.contains_key(type_name) {
            self.type_definitions
                .entry(type_name.to_string())
                .or_default()
                .push(source_path.to_path_buf());
        }
    }

    /// Parse a file and extract its scope (imports, local types, submodules)
    pub fn parse_file(&mut self, path: &Path, content: &str, base_path: &Path) -> Result<()> {
        let syntax = syn::parse_file(content)?;

        let mut scope = FileScope {
            module_path: self.path_to_module(path, base_path),
            ..Default::default()
        };

        // Process items, including nested modules
        self.parse_items(&syntax.items, path, &mut scope);

        self.module_to_file
            .insert(scope.module_path.clone(), path.to_path_buf());
        self.files.insert(path.to_path_buf(), scope);

        Ok(())
    }

    /// Parse items recursively (handles nested modules)
    fn parse_items(&mut self, items: &[Item], path: &Path, scope: &mut FileScope) {
        for item in items {
            match item {
                Item::Use(item_use) => {
                    self.parse_use_tree(&item_use.tree, scope, &mut Vec::new());
                }
                Item::Struct(s) => {
                    let name = s.ident.to_string();
                    scope.local_types.insert(name.clone(), TypeKind::Struct);
                    self.register_type_definition(&name, path);
                }
                Item::Enum(e) => {
                    let name = e.ident.to_string();
                    scope.local_types.insert(name.clone(), TypeKind::Enum);
                    self.register_type_definition(&name, path);
                }
                Item::Type(t) => {
                    // Handle type aliases: type Foo = Bar;
                    let name = t.ident.to_string();
                    scope.local_types.insert(name.clone(), TypeKind::Struct); // Treat as struct-like
                    self.register_type_definition(&name, path);
                    
                    // Extract the base type name (e.g., "State" from "State<'a, T>")
                    if let Some(base_type) = extract_base_type_name(&t.ty) {
                        scope.type_aliases.insert(name, base_type);
                    }
                }
                Item::Mod(m) => {
                    // Recursively parse types inside inline modules
                    if let Some((_, mod_items)) = &m.content {
                        self.parse_items(mod_items, path, scope);
                    }
                }
                _ => {}
            }
        }
    }

    /// Register a type definition, avoiding duplicates
    fn register_type_definition(&mut self, name: &str, path: &Path) {
        let locations = self.type_definitions.entry(name.to_string()).or_default();
        let path_buf = path.to_path_buf();
        if !locations.contains(&path_buf) {
            locations.push(path_buf);
        }
    }

    /// Parse use tree recursively
    fn parse_use_tree(&self, tree: &UseTree, scope: &mut FileScope, prefix: &mut Vec<String>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.parse_use_tree(&path.tree, scope, prefix);
                prefix.pop();
            }
            UseTree::Name(name) => {
                let type_name = name.ident.to_string();
                prefix.push(type_name.clone());
                scope
                    .imports
                    .insert(type_name, ImportedType { path: prefix.clone() });
                prefix.pop();
            }
            UseTree::Rename(rename) => {
                let original_name = rename.ident.to_string();
                let alias = rename.rename.to_string();
                prefix.push(original_name);
                scope.imports.insert(alias, ImportedType { path: prefix.clone() });
                prefix.pop();
            }
            UseTree::Glob(_) => {
                scope.wildcard_imports.push(prefix.clone());
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.parse_use_tree(item, scope, prefix);
                }
            }
        }
    }

    /// Convert file path to module path
    fn path_to_module(&self, path: &Path, base_path: &Path) -> Vec<String> {
        let relative = path.strip_prefix(base_path).unwrap_or(path);
        let mut parts: Vec<String> = vec!["crate".to_string()];

        for component in relative.components() {
            if let std::path::Component::Normal(s) = component {
                let s = s.to_string_lossy();
                if s == "mod.rs" || s == "lib.rs" || s == "main.rs" {
                    continue;
                }
                let name = s.trim_end_matches(".rs");
                parts.push(name.to_string());
            }
        }

        parts
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
            None => ResolutionResult::NotFound
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
    fn normalize_relative_path(&self, relative_path: &[String], from_module: &[String]) -> Vec<String> {
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
                if let ResolutionResult::Found(source_file) = self.resolve_module_path(&imported.path) {
                    if let Some(source_scope) = self.files.get(&source_file) {
                        let actual_name = imported.path.last().map(|s| s.as_str()).unwrap_or(type_name);
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

fn are_siblings(path_a: &[String], path_b: &[String]) -> bool {
    // Empty paths or single-element paths can't be siblings
    if path_a.len() < 2 || path_b.len() < 2 {
        return false;
    }
    if path_a.len() != path_b.len() {
        return false;
    }
    // Check if they share the same parent
    // a: [crate, foo, bar]
    // b: [crate, foo, baz]
    // parent: [crate, foo]
    path_a[..path_a.len() - 1] == path_b[..path_b.len() - 1]
}

/// Extract the base type name from a syn::Type
/// For example, "State<'a, T>" returns Some("State")
fn extract_base_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(type_path) => {
            // Get the last segment (e.g., "State" from "tauri::State")
            type_path.path.segments.last().map(|seg| seg.ident.to_string())
        }
        syn::Type::Reference(type_ref) => {
            // Handle &T or &mut T
            extract_base_type_name(&type_ref.elem)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_path() -> PathBuf {
        PathBuf::from("src")
    }

    #[test]
    fn test_resolve_local_type() {
        let mut resolver = ModuleResolver::new();
        let code = "struct User;";
        let path = PathBuf::from("src/types.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        match resolver.resolve_type("User", &path) {
            ResolutionResult::Found(p) => assert_eq!(p, path),
            _ => panic!("Failed to resolve"),
        }
    }

    #[test]
    fn test_resolve_super() {
        let mut resolver = ModuleResolver::new();
        
        // Parent
        let parent_code = "struct User;";
        let parent_path = PathBuf::from("src/mod.rs");
        resolver.parse_file(&parent_path, parent_code, &base_path()).unwrap();
        
        // Child
        let child_code = "";
        let child_path = PathBuf::from("src/sub/mod.rs");
        resolver.parse_file(&child_path, child_code, &base_path()).unwrap();
        
        match resolver.resolve_type("super::User", &child_path) {
             ResolutionResult::Found(p) => assert_eq!(p, parent_path),
             res => panic!("Failed to resolve super::User: {:?}", res),
        }
    }
    
    #[test]
    fn test_resolve_path_via_import() {
        let mut resolver = ModuleResolver::new();

        // Define type in a module: src/types.rs -> User
        let types_code = "struct User;";
        let types_path = PathBuf::from("src/types.rs");
        resolver.parse_file(&types_path, types_code, &base_path()).unwrap();

        // Usage file: imports module, uses qualified path
        // use crate::types;
        // ... types::User
        let cmd_code = "use crate::types;";
        let cmd_path = PathBuf::from("src/cmd.rs");
        resolver.parse_file(&cmd_path, cmd_code, &base_path()).unwrap();

        match resolver.resolve_type("types::User", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            res => panic!("Failed to resolve types::User via import: {:?}", res),
        }
    }

    #[test]
    fn test_resolve_ambiguous() {
        let mut resolver = ModuleResolver::new();
        
        let path_a = PathBuf::from("src/a.rs");
        resolver.parse_file(&path_a, "struct User;", &base_path()).unwrap();
        
        let path_b = PathBuf::from("src/b.rs");
        resolver.parse_file(&path_b, "struct User;", &base_path()).unwrap();
        
        let path_cmd = PathBuf::from("src/cmd.rs");
        resolver.parse_file(&path_cmd, "", &base_path()).unwrap();
        
        match resolver.resolve_type("User", &path_cmd) {
            ResolutionResult::Ambiguous(paths) => {
                assert_eq!(paths.len(), 2);
                assert!(paths.contains(&path_a));
                assert!(paths.contains(&path_b));
            },
            res => panic!("Expected Ambiguous, got {:?}", res),
        }
    }

    #[test]
    fn test_resolve_path_via_renamed_import() {
        let mut resolver = ModuleResolver::new();

        let types_path = PathBuf::from("src/long_name/types.rs");
        resolver.parse_file(&types_path, "struct User;", &base_path()).unwrap();

        // use crate::long_name::types as t;
        // t::User
        let cmd_code = "use crate::long_name::types as t;";
        let cmd_path = PathBuf::from("src/cmd.rs");
        resolver.parse_file(&cmd_path, cmd_code, &base_path()).unwrap();

        match resolver.resolve_type("t::User", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            res => panic!("Failed to resolve t::User via renamed import: {:?}", res),
        }
    }

    #[test]
    fn test_resolve_deeply_nested_path() {
        let mut resolver = ModuleResolver::new();

        let target_path = PathBuf::from("src/a/b/c/target.rs");
        resolver.parse_file(&target_path, "struct Deep;", &base_path()).unwrap();

        let cmd_path = PathBuf::from("src/main.rs");
        resolver.parse_file(&cmd_path, "", &base_path()).unwrap();

        match resolver.resolve_type("crate::a::b::c::target::Deep", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, target_path),
            res => panic!("Failed to resolve deep path: {:?}", res),
        }
    }

    #[test]
    fn test_resolve_super_chain() {
        let mut resolver = ModuleResolver::new();

        let root_path = PathBuf::from("src/types.rs");
        resolver.parse_file(&root_path, "struct Top;", &base_path()).unwrap();

        let deep_path = PathBuf::from("src/a/b/c/deep.rs");
        resolver.parse_file(&deep_path, "", &base_path()).unwrap();

        // deep.rs is at crate::a::b::c::deep
        // super -> c
        // super -> b
        // super -> a
        // super -> crate
        // super::super::super::super::types::Top
        match resolver.resolve_type("super::super::super::super::types::Top", &deep_path) {
            ResolutionResult::Found(p) => assert_eq!(p, root_path),
            res => panic!("Failed to resolve super chain: {:?}", res),
        }
    }
    
    #[test]
    fn test_resolve_sibling_via_super() {
        let mut resolver = ModuleResolver::new();
        
        // src/sibling.rs -> crate::sibling
        let sibling_path = PathBuf::from("src/sibling.rs");
        resolver.parse_file(&sibling_path, "struct SiblingType;", &base_path()).unwrap();
        
        // src/current.rs -> crate::current
        let current_path = PathBuf::from("src/current.rs");
        resolver.parse_file(&current_path, "", &base_path()).unwrap();
        
        // siblings must be accessed via parent (super) if not imported
        match resolver.resolve_type("super::sibling::SiblingType", &current_path) {
             ResolutionResult::Found(p) => assert_eq!(p, sibling_path),
             res => panic!("Failed to resolve sibling path via super: {:?}", res),
        }
    }
    #[test]
    fn test_resolve_reexport() {
        let mut resolver = ModuleResolver::new();

        // src/types.rs -> struct User
        let types_path = PathBuf::from("src/types.rs");
        resolver.parse_file(&types_path, "pub struct User;", &base_path()).unwrap();

        // src/lib.rs -> pub mod types; pub use types::User;
        let lib_path = PathBuf::from("src/lib.rs");
        let lib_code = "pub mod types; pub use types::User;";
        resolver.parse_file(&lib_path, lib_code, &base_path()).unwrap();

        // Verify lib_path was parsed correctly
        assert!(resolver.files.contains_key(&lib_path));

        // src/cmd.rs -> use crate::User;
        let main_path = PathBuf::from("src/cmd.rs");
        let main_code = "use crate::User;";
        resolver.parse_file(&main_path, main_code, &base_path()).unwrap();

        // Should resolve crate::User to src/types.rs
        match resolver.resolve_type("crate::User", &main_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            res => panic!("Failed to resolve re-export: {:?}", res),
        }
    }

    #[test]
    fn test_resolve_type_via_wildcard_reexport() {
        let mut resolver = ModuleResolver::new();

        // src/resources/types.rs -> struct PodInfo
        let types_path = PathBuf::from("src/resources/types.rs");
        resolver.parse_file(&types_path, "pub struct PodInfo;", &base_path()).unwrap();

        // src/resources/mod.rs -> pub use types::*;
        let mod_path = PathBuf::from("src/resources/mod.rs");
        let mod_code = "pub use types::*;";
        resolver.parse_file(&mod_path, mod_code, &base_path()).unwrap();

        // src/commands.rs -> use crate::resources::PodInfo;
        let cmd_path = PathBuf::from("src/commands.rs");
        let cmd_code = "use crate::resources::PodInfo;";
        resolver.parse_file(&cmd_path, cmd_code, &base_path()).unwrap();

        // Should resolve crate::resources::PodInfo via wildcard re-export to src/resources/types.rs
        match resolver.resolve_type("crate::resources::PodInfo", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            res => panic!("Failed to resolve via wildcard re-export: {:?}", res),
        }
    }

    #[test]
    fn test_resolve_simple_name_via_wildcard() {
        let mut resolver = ModuleResolver::new();

        // src/types.rs -> struct User
        let types_path = PathBuf::from("src/types.rs");
        resolver.parse_file(&types_path, "pub struct User;", &base_path()).unwrap();

        // src/main.rs -> use types::*;
        let main_path = PathBuf::from("src/main.rs");
        let main_code = "use types::*;";
        resolver.parse_file(&main_path, main_code, &base_path()).unwrap();

        // Should resolve User via wildcard import
        match resolver.resolve_type("User", &main_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            res => panic!("Failed to resolve simple name via wildcard: {:?}", res),
        }
    }

    #[test]
    fn test_normalize_relative_path_simple() {
        let resolver = ModuleResolver::new();

        // ["types"] with context ["crate", "resources"] -> ["crate", "resources", "types"]
        let relative = vec!["types".to_string()];
        let from_module = vec!["crate".to_string(), "resources".to_string()];
        let result = resolver.normalize_relative_path(&relative, &from_module);
        
        assert_eq!(result, vec!["crate", "resources", "types"]);
    }

    #[test]
    fn test_normalize_relative_path_with_super() {
        let resolver = ModuleResolver::new();

        // ["super", "other"] with context ["crate", "foo", "bar"] -> ["crate", "foo", "other"]
        let relative = vec!["super".to_string(), "other".to_string()];
        let from_module = vec!["crate".to_string(), "foo".to_string(), "bar".to_string()];
        let result = resolver.normalize_relative_path(&relative, &from_module);
        
        assert_eq!(result, vec!["crate", "foo", "other"]);
    }

    #[test]
    fn test_normalize_absolute_path() {
        let resolver = ModuleResolver::new();

        // ["crate", "types"] is already absolute, should not change
        let absolute = vec!["crate".to_string(), "types".to_string()];
        let from_module = vec!["crate".to_string(), "other".to_string()];
        let result = resolver.normalize_relative_path(&absolute, &from_module);
        
        assert_eq!(result, vec!["crate", "types"]);
    }

    #[test]
    fn test_resolve_through_multiple_wildcards() {
        let mut resolver = ModuleResolver::new();

        // src/inner/types.rs -> struct DeepType
        let deep_types_path = PathBuf::from("src/inner/types.rs");
        resolver.parse_file(&deep_types_path, "pub struct DeepType;", &base_path()).unwrap();

        // src/inner/mod.rs -> pub use types::*;
        let inner_mod_path = PathBuf::from("src/inner/mod.rs");
        resolver.parse_file(&inner_mod_path, "pub use types::*;", &base_path()).unwrap();

        // src/lib.rs -> pub use inner::*;
        let lib_path = PathBuf::from("src/lib.rs");
        resolver.parse_file(&lib_path, "pub use inner::*;", &base_path()).unwrap();

        // Should resolve crate::inner::DeepType via wildcard in inner/mod.rs
        let cmd_path = PathBuf::from("src/cmd.rs");
        resolver.parse_file(&cmd_path, "", &base_path()).unwrap();

        match resolver.resolve_type("crate::inner::DeepType", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, deep_types_path),
            res => panic!("Failed to resolve through nested wildcard: {:?}", res),
        }
    }

    #[test]
    fn test_resolve_mixed_explicit_and_wildcard() {
        let mut resolver = ModuleResolver::new();

        // src/a.rs -> struct TypeA
        let a_path = PathBuf::from("src/a.rs");
        resolver.parse_file(&a_path, "pub struct TypeA;", &base_path()).unwrap();

        // src/b.rs -> struct TypeB
        let b_path = PathBuf::from("src/b.rs");
        resolver.parse_file(&b_path, "pub struct TypeB;", &base_path()).unwrap();

        // src/lib.rs -> pub use a::TypeA; pub use b::*;
        let lib_path = PathBuf::from("src/lib.rs");
        let lib_code = "pub use a::TypeA; pub use b::*;";
        resolver.parse_file(&lib_path, lib_code, &base_path()).unwrap();

        let cmd_path = PathBuf::from("src/cmd.rs");
        resolver.parse_file(&cmd_path, "", &base_path()).unwrap();

        // Resolve explicit re-export
        match resolver.resolve_type("crate::TypeA", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, a_path),
            res => panic!("Failed to resolve explicit re-export: {:?}", res),
        }

        // Resolve wildcard re-export
        match resolver.resolve_type("crate::TypeB", &cmd_path) {
            ResolutionResult::Found(p) => assert_eq!(p, b_path),
            res => panic!("Failed to resolve wildcard re-export: {:?}", res),
        }
    }

    #[test]
    fn test_are_siblings_empty_path() {
        // Empty paths should not panic and return false
        assert!(!are_siblings(&[], &[]));
        assert!(!are_siblings(&["crate".to_string()], &[]));
        assert!(!are_siblings(&[], &["crate".to_string()]));
    }

    #[test]
    fn test_are_siblings_single_element() {
        // Single element paths can't have siblings (no parent)
        assert!(!are_siblings(
            &["crate".to_string()],
            &["crate".to_string()]
        ));
    }

    #[test]
    fn test_are_siblings_valid() {
        assert!(are_siblings(
            &["crate".to_string(), "foo".to_string()],
            &["crate".to_string(), "bar".to_string()]
        ));
        assert!(!are_siblings(
            &["crate".to_string(), "foo".to_string()],
            &["crate".to_string(), "other".to_string(), "bar".to_string()]
        ));
    }

    #[test]
    fn test_parse_nested_module() {
        let mut resolver = ModuleResolver::new();

        let code = r#"
            mod inner {
                pub struct NestedType;
            }
            pub struct OuterType;
        "#;
        let path = PathBuf::from("src/lib.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // Both types should be registered
        assert!(resolver.type_definitions.contains_key("NestedType"));
        assert!(resolver.type_definitions.contains_key("OuterType"));
    }

    #[test]
    fn test_parse_type_alias() {
        let mut resolver = ModuleResolver::new();

        let code = r#"
            pub struct RealType;
            pub type AliasType = RealType;
        "#;
        let path = PathBuf::from("src/types.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // Both should be registered
        assert!(resolver.type_definitions.contains_key("RealType"));
        assert!(resolver.type_definitions.contains_key("AliasType"));

        // Resolve alias type
        match resolver.resolve_type("AliasType", &path) {
            ResolutionResult::Found(p) => assert_eq!(p, path),
            res => panic!("Failed to resolve type alias: {:?}", res),
        }
    }

    #[test]
    fn test_no_duplicate_registration() {
        let mut resolver = ModuleResolver::new();

        let code = "pub struct User;";
        let path = PathBuf::from("src/types.rs");

        // Parse the same file twice
        resolver.parse_file(&path, code, &base_path()).unwrap();
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // Should only have one entry for the path
        let locations = resolver.type_definitions.get("User").unwrap();
        assert_eq!(locations.len(), 1);
    }

    #[test]
    fn test_resolve_alias_target_simple() {
        let mut resolver = ModuleResolver::new();

        let code = r#"
            pub struct RealType;
            pub type AliasType = RealType;
        "#;
        let path = PathBuf::from("src/types.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // resolve_alias_target should return the base type name
        let target = resolver.resolve_alias_target("AliasType", &path);
        assert_eq!(target, Some("RealType".to_string()));
        
        // Non-alias types should return None
        let no_target = resolver.resolve_alias_target("RealType", &path);
        assert_eq!(no_target, None);
    }

    #[test]
    fn test_resolve_alias_target_tauri_state() {
        let mut resolver = ModuleResolver::new();

        // This simulates: type AppStateMutexed<'a> = State<'a, Mutex<AppState>>;
        let code = r#"
            pub type AppStateMutexed<'a> = State<'a, Mutex<AppState>>;
        "#;
        let path = PathBuf::from("src/commands.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // resolve_alias_target should return "State"
        let target = resolver.resolve_alias_target("AppStateMutexed", &path);
        assert_eq!(target, Some("State".to_string()));
    }

    #[test]
    fn test_resolve_alias_target_generic_type() {
        let mut resolver = ModuleResolver::new();

        let code = r#"
            pub type MyWindow = Window;
            pub type MyHandle<T> = AppHandle<T>;
        "#;
        let path = PathBuf::from("src/types.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // Both should resolve to their base types
        assert_eq!(resolver.resolve_alias_target("MyWindow", &path), Some("Window".to_string()));
        assert_eq!(resolver.resolve_alias_target("MyHandle", &path), Some("AppHandle".to_string()));
    }

    #[test]
    fn test_type_alias_stored_in_scope() {
        let mut resolver = ModuleResolver::new();

        let code = r#"
            pub type CustomState<'a> = State<'a, MyAppState>;
        "#;
        let path = PathBuf::from("src/lib.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // Check that the alias is stored in the scope
        let scope = resolver.files.get(&path).unwrap();
        assert_eq!(scope.type_aliases.get("CustomState"), Some(&"State".to_string()));
    }

    #[test]
    fn test_resolve_alias_chain() {
        let mut resolver = ModuleResolver::new();

        // Create a chain: AliasedState -> MyState -> State
        let code = r#"
            pub type MyState<'a> = State<'a, AppState>;
            pub type AliasedState<'a> = MyState<'a>;
        "#;
        let path = PathBuf::from("src/types.rs");
        resolver.parse_file(&path, code, &base_path()).unwrap();

        // resolve_alias_target should follow the chain to "State"
        let target = resolver.resolve_alias_target("AliasedState", &path);
        assert_eq!(target, Some("State".to_string()));

        // MyState should also resolve to State
        let target2 = resolver.resolve_alias_target("MyState", &path);
        assert_eq!(target2, Some("State".to_string()));
    }

    #[test]
    fn test_cross_file_type_not_ambiguous() {
        // This test verifies the fix for the bug where types defined in one file
        // and used in another would incorrectly trigger "ambiguous type" warnings
        // when cargo-expand was also registering the same types.
        //
        // The fix ensures that types are only registered from their actual source
        // files, not from cargo-expand output.
        let mut resolver = ModuleResolver::new();

        // types.rs defines DeploymentContainerInfo
        let types_code = r#"
            pub struct DeploymentContainerInfo {
                pub name: String,
            }
        "#;
        let types_path = PathBuf::from("src/resources/types.rs");
        resolver.parse_file(&types_path, types_code, &base_path()).unwrap();

        // workloads.rs uses DeploymentContainerInfo via import
        let workloads_code = r#"
            use super::types::DeploymentContainerInfo;

            pub struct StatefulSetDetailInfo {
                pub containers: Vec<DeploymentContainerInfo>,
            }
        "#;
        let workloads_path = PathBuf::from("src/resources/workloads.rs");
        resolver.parse_file(&workloads_path, workloads_code, &base_path()).unwrap();

        // DeploymentContainerInfo should only be registered once (from types.rs)
        let locations = resolver.type_definitions.get("DeploymentContainerInfo").unwrap();
        assert_eq!(locations.len(), 1, "Type should only be registered once, not duplicated");
        assert_eq!(locations[0], types_path);

        // Resolving the type from workloads.rs should find it (not be ambiguous)
        match resolver.resolve_type("DeploymentContainerInfo", &workloads_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            ResolutionResult::Ambiguous(paths) => {
                panic!("Type should NOT be ambiguous! Found in: {:?}", paths);
            }
            res => panic!("Expected Found, got {:?}", res),
        }
    }

    #[test]
    fn test_type_defined_in_multiple_files_is_ambiguous() {
        // This test verifies that types ACTUALLY defined in multiple files
        // are correctly detected as ambiguous (this is the expected behavior).
        let mut resolver = ModuleResolver::new();

        // Two different files both define a type with the same name
        let file_a = PathBuf::from("src/a.rs");
        resolver.parse_file(&file_a, "pub struct SharedName;", &base_path()).unwrap();

        let file_b = PathBuf::from("src/b.rs");
        resolver.parse_file(&file_b, "pub struct SharedName;", &base_path()).unwrap();

        // This SHOULD be ambiguous because the type is defined in two places
        let locations = resolver.type_definitions.get("SharedName").unwrap();
        assert_eq!(locations.len(), 2, "Type defined in 2 files should have 2 locations");

        let query_path = PathBuf::from("src/main.rs");
        resolver.parse_file(&query_path, "", &base_path()).unwrap();

        match resolver.resolve_type("SharedName", &query_path) {
            ResolutionResult::Ambiguous(paths) => {
                assert_eq!(paths.len(), 2);
                assert!(paths.contains(&file_a));
                assert!(paths.contains(&file_b));
            }
            res => panic!("Expected Ambiguous, got {:?}", res),
        }
    }

    #[test]
    fn test_cargo_expand_types_not_registered_separately() {
        // This test verifies the fix for the cargo-expand ambiguity bug.
        //
        // BEFORE THE FIX:
        // When cargo-expand was used, types were registered unconditionally,
        // even if they already existed in source files. This caused duplicates.
        //
        // AFTER THE FIX:
        // register_expanded_type_if_missing() only registers types that DON'T
        // already exist in source files. Types from source files take priority.

        let mut resolver = ModuleResolver::new();

        // Parse the actual source file FIRST - this registers the type
        let types_path = PathBuf::from("src/resources/types.rs");
        let types_code = r#"
            pub struct DeploymentContainerInfo {
                pub name: String,
            }
        "#;
        resolver.parse_file(&types_path, types_code, &base_path()).unwrap();

        // Verify type is registered exactly once from the source file
        let locations = resolver.type_definitions.get("DeploymentContainerInfo").unwrap();
        assert_eq!(locations.len(), 1, "Type should be registered only once");
        assert_eq!(locations[0], types_path, "Type should be registered from source file");

        // Now try to register the same type from cargo-expand - should be ignored
        let expanded_path = PathBuf::from("<cargo-expand>");
        resolver.register_expanded_type_if_missing("DeploymentContainerInfo", &expanded_path);

        // Type should STILL be registered only once (cargo-expand was ignored)
        let locations = resolver.type_definitions.get("DeploymentContainerInfo").unwrap();
        assert_eq!(locations.len(), 1, "Cargo-expand should not duplicate source file types");
        assert_eq!(locations[0], types_path, "Source file registration should be preserved");

        // Simulate another file that USES (not defines) the type
        let workloads_path = PathBuf::from("src/resources/workloads.rs");
        let workloads_code = r#"
            use super::types::DeploymentContainerInfo;

            pub struct StatefulSetDetailInfo {
                pub containers: Vec<DeploymentContainerInfo>,
            }
        "#;
        resolver.parse_file(&workloads_path, workloads_code, &base_path()).unwrap();

        // Type should STILL be registered only once (imports don't register types)
        let locations = resolver.type_definitions.get("DeploymentContainerInfo").unwrap();
        assert_eq!(locations.len(), 1, "Importing a type should not register it again");

        // Resolution should find the type without ambiguity
        match resolver.resolve_type("DeploymentContainerInfo", &workloads_path) {
            ResolutionResult::Found(p) => assert_eq!(p, types_path),
            ResolutionResult::Ambiguous(paths) => {
                panic!(
                    "BUG: Type should NOT be ambiguous! This was the original bug. Found in: {:?}",
                    paths
                );
            }
            res => panic!("Expected Found, got {:?}", res),
        }
    }

    #[test]
    fn test_macro_generated_types_can_be_resolved() {
        // This test verifies that types that ONLY exist in cargo-expand output
        // (macro-generated types not present in source files) can still be resolved.
        //
        // Example: progenitor generates API client structs from OpenAPI specs.
        // These structs don't exist in source code, only in expanded output.

        let mut resolver = ModuleResolver::new();

        // Parse a source file that does NOT contain MacroGeneratedType
        let source_path = PathBuf::from("src/commands.rs");
        let source_code = r#"
            pub struct NormalType {
                pub value: i32,
            }
        "#;
        resolver.parse_file(&source_path, source_code, &base_path()).unwrap();

        // MacroGeneratedType doesn't exist in source - verify it's not registered
        assert!(resolver.type_definitions.get("MacroGeneratedType").is_none());

        // Register a macro-generated type from cargo-expand
        // This simulates what pipeline.rs does after parsing source files
        let expanded_path = PathBuf::from("<cargo-expand>");
        resolver.register_expanded_type_if_missing("MacroGeneratedType", &expanded_path);

        // Now the macro-generated type should be registered
        let locations = resolver.type_definitions.get("MacroGeneratedType").unwrap();
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0], expanded_path);

        // Resolution should find the macro-generated type
        match resolver.resolve_type("MacroGeneratedType", &source_path) {
            ResolutionResult::Found(p) => {
                assert_eq!(p, expanded_path, "Should resolve to <cargo-expand>");
            }
            res => panic!("Expected Found for macro-generated type, got {:?}", res),
        }
    }
}
