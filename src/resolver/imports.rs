//! `ModuleResolver` methods that build the scope from Rust source: parsing
//! `use` trees, registering local type definitions, converting file paths to
//! module paths, and tracking cargo-expand-only types.

use anyhow::Result;
use std::path::Path;
use syn::{Item, UseTree};

use super::helpers::extract_base_type_name;
use super::{FileScope, ImportedType, ModuleResolver, TypeKind};

impl ModuleResolver {
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
    pub(super) fn parse_items(&mut self, items: &[Item], path: &Path, scope: &mut FileScope) {
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
    pub(super) fn register_type_definition(&mut self, name: &str, path: &Path) {
        let locations = self.type_definitions.entry(name.to_string()).or_default();
        let path_buf = path.to_path_buf();
        if !locations.contains(&path_buf) {
            locations.push(path_buf);
        }
    }

    /// Parse use tree recursively
    pub(super) fn parse_use_tree(
        &self,
        tree: &UseTree,
        scope: &mut FileScope,
        prefix: &mut Vec<String>,
    ) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.parse_use_tree(&path.tree, scope, prefix);
                prefix.pop();
            }
            UseTree::Name(name) => {
                let type_name = name.ident.to_string();
                prefix.push(type_name.clone());
                scope.imports.insert(
                    type_name,
                    ImportedType {
                        path: prefix.clone(),
                    },
                );
                prefix.pop();
            }
            UseTree::Rename(rename) => {
                let original_name = rename.ident.to_string();
                let alias = rename.rename.to_string();
                prefix.push(original_name);
                scope.imports.insert(
                    alias,
                    ImportedType {
                        path: prefix.clone(),
                    },
                );
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
    pub(super) fn path_to_module(&self, path: &Path, base_path: &Path) -> Vec<String> {
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
}
