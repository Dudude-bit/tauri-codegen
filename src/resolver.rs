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

/// Result of a type resolution attempt
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionResult {
    /// Successfully resolved to a single file
    Found(PathBuf),
    /// Type not found
    NotFound,
    /// Ambiguous: found in multiple files
    Ambiguous(Vec<PathBuf>),
}

/// Represents a parsed file with its imports and local types
#[derive(Debug, Default)]
pub struct FileScope {
    /// Module path (e.g., ["crate", "commands"] for src/commands.rs)
    pub module_path: Vec<String>,
    /// Types defined locally in this file (name -> kind)
    pub local_types: HashMap<String, TypeKind>,
    /// Imports: local name -> full path
    pub imports: HashMap<String, ImportedType>,
    /// Wildcard imports (use something::*)
    pub wildcard_imports: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone)]
pub struct ImportedType {
    /// Full module path (e.g., ["crate", "internal", "UserRole"])
    pub path: Vec<String>,
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

    /// Parse a file and extract its scope (imports, local types, submodules)
    pub fn parse_file(&mut self, path: &Path, content: &str, base_path: &Path) -> Result<()> {
        let syntax = syn::parse_file(content)?;

        let mut scope = FileScope {
            module_path: self.path_to_module(path, base_path),
            ..Default::default()
        };

        for item in &syntax.items {
            match item {
                Item::Use(item_use) => {
                    self.parse_use_tree(&item_use.tree, &mut scope, Vec::new());
                }
                Item::Struct(s) => {
                    let name = s.ident.to_string();
                    scope.local_types.insert(name.clone(), TypeKind::Struct);
                    self.type_definitions
                        .entry(name)
                        .or_default()
                        .push(path.to_path_buf());
                }
                Item::Enum(e) => {
                    let name = e.ident.to_string();
                    scope.local_types.insert(name.clone(), TypeKind::Enum);
                    self.type_definitions
                        .entry(name)
                        .or_default()
                        .push(path.to_path_buf());
                }
                _ => {}
            }
        }

        self.module_to_file
            .insert(scope.module_path.clone(), path.to_path_buf());
        self.files.insert(path.to_path_buf(), scope);

        Ok(())
    }

    /// Parse use tree recursively
    fn parse_use_tree(&self, tree: &UseTree, scope: &mut FileScope, mut prefix: Vec<String>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.parse_use_tree(&path.tree, scope, prefix);
            }
            UseTree::Name(name) => {
                let type_name = name.ident.to_string();
                prefix.push(type_name.clone());
                scope
                    .imports
                    .insert(type_name, ImportedType { path: prefix });
            }
            UseTree::Rename(rename) => {
                let original_name = rename.ident.to_string();
                let alias = rename.rename.to_string();
                prefix.push(original_name);
                scope.imports.insert(alias, ImportedType { path: prefix });
            }
            UseTree::Glob(_) => {
                scope.wildcard_imports.push(prefix);
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.parse_use_tree(item, scope, prefix.clone());
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
        let scope = match self.files.get(from_file) {
            Some(s) => s,
            None => return ResolutionResult::NotFound,
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
            return self.resolve_module_path(&imported.path);
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
            // Simple proximity check: same parent module?
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
                     return self.resolve_path(&segments, scope);
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
}

fn are_siblings(path_a: &[String], path_b: &[String]) -> bool {
    if path_a.len() != path_b.len() {
        return false;
    }
    // Check if they share the same parent
    // a: [crate, foo, bar]
    // b: [crate, foo, baz]
    // parent: [crate, foo]
    path_a[..path_a.len()-1] == path_b[..path_b.len()-1]
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

        if let Some(scope) = resolver.files.get(&lib_path) {
            eprintln!("Lib imports: {:?}", scope.imports);
        }

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
}
