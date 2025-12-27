//! Pipeline module - orchestrates the code generation process
//!
//! Flow: Scan -> Parse -> Resolve -> Collect -> Generate

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::generator::{
    commands_gen::generate_commands_file, types_gen::generate_types_file, GeneratorContext,
};
use crate::models::{ParseResult, RustEnum, RustStruct, RustType};
use crate::parser::{parse_commands, parse_types};
use crate::resolver::{ModuleResolver, ResolutionResult};
use crate::scanner::Scanner;

/// Result of type collection with potential conflicts
pub struct TypeCollectionResult {
    /// Successfully resolved types: name -> source file
    pub resolved: HashMap<String, PathBuf>,
    /// Conflicts: type name -> list of conflicting source files
    pub conflicts: HashMap<String, Vec<PathBuf>>,
}

/// Main pipeline for code generation
pub struct Pipeline {
    verbose: bool,
}

impl Pipeline {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Run the full generation pipeline
    pub fn run(&self, config: &Config) -> Result<()> {
        if self.verbose {
            println!("Scanning directory: {}", config.input.source_dir.display());
        }

        // Step 1: Scan for Rust files
        let rust_files = self.scan_files(config)?;

        if self.verbose {
            println!("Found {} Rust files", rust_files.len());
        }

        // Step 2: Parse all files and build resolver
        let (parse_result, resolver) = self.parse_files(&rust_files, config)?;

        // Step 3: Collect and resolve types used in commands
        let type_collection = self.collect_used_types(&parse_result, &resolver);

        // Step 4: Check for conflicts
        if !type_collection.conflicts.is_empty() {
            eprintln!("Error: Type name conflicts detected:");
            for (type_name, files) in &type_collection.conflicts {
                eprintln!("  Type '{}' is used from multiple sources:", type_name);
                for file in files {
                    eprintln!("    - {}", file.display());
                }
            }
            anyhow::bail!(
                "Found {} type name conflict(s). Please rename types or use explicit imports to avoid ambiguity.",
                type_collection.conflicts.len()
            );
        }

        // Step 5: Filter types based on resolution
        let (filtered_structs, filtered_enums) =
            self.filter_types(&parse_result, &type_collection.resolved);

        // Summary
        println!(
            "Parsed {} commands, {} structs (used), {} enums (used)",
            parse_result.commands.len(),
            filtered_structs.len(),
            filtered_enums.len()
        );

        // Step 6: Generate TypeScript files
        self.generate_output(config, &parse_result, &filtered_structs, &filtered_enums)?;

        println!("Done!");

        Ok(())
    }

    /// Step 1: Scan for Rust files
    fn scan_files(&self, config: &Config) -> Result<Vec<PathBuf>> {
        let scanner = Scanner::new(
            config.input.source_dir.clone(),
            config.input.exclude.clone(),
        );
        scanner.scan()
    }

    /// Step 2: Parse all files and build module resolver
    fn parse_files(
        &self,
        rust_files: &[PathBuf],
        config: &Config,
    ) -> Result<(ParseResult, ModuleResolver)> {
        let mut resolver = ModuleResolver::new();
        let base_path = config.input.source_dir.clone();
        let mut parse_result = ParseResult::new();

        for file_path in rust_files {
            let content = fs::read_to_string(file_path)
                .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

            // Build resolver scope for this file
            if let Err(e) = resolver.parse_file(file_path, &content, &base_path) {
                if self.verbose {
                    eprintln!(
                        "Warning: Failed to parse imports in {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }

            // Parse commands
            match parse_commands(&content, file_path) {
                Ok(commands) => {
                    if !commands.is_empty() && self.verbose {
                        println!(
                            "Found {} commands in {}",
                            commands.len(),
                            file_path.display()
                        );
                    }
                    parse_result.commands.extend(commands);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse commands in {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }

            // Parse types
            match parse_types(&content, file_path) {
                Ok((structs, enums)) => {
                    if self.verbose && (!structs.is_empty() || !enums.is_empty()) {
                        println!(
                            "Found {} structs and {} enums in {}",
                            structs.len(),
                            enums.len(),
                            file_path.display()
                        );
                    }
                    parse_result.structs.extend(structs);
                    parse_result.enums.extend(enums);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse types in {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }

        Ok((parse_result, resolver))
    }

    /// Step 3: Collect all types used in commands, resolving their source files
    fn collect_used_types(
        &self,
        parse_result: &ParseResult,
        resolver: &ModuleResolver,
    ) -> TypeCollectionResult {
        let mut resolved_types: HashMap<String, PathBuf> = HashMap::new();
        let mut conflicts: HashMap<String, Vec<PathBuf>> = HashMap::new();

        // Build lookup maps: (name, source_file) -> type
        let struct_by_file: HashMap<(&str, &Path), &RustStruct> = parse_result
            .structs
            .iter()
            .map(|s| ((s.name.as_str(), s.source_file.as_path()), s))
            .collect();
        let enum_by_file: HashMap<(&str, &Path), &RustEnum> = parse_result
            .enums
            .iter()
            .map(|e| ((e.name.as_str(), e.source_file.as_path()), e))
            .collect();

        // Collect types from all commands, resolving source files
        for cmd in &parse_result.commands {
            let cmd_file = &cmd.source_file;

            for arg in &cmd.args {
                self.collect_types_with_resolver(
                    &arg.ty,
                    cmd_file,
                    resolver,
                    &mut resolved_types,
                    &mut conflicts,
                );
            }
            if let Some(ref ret_type) = cmd.return_type {
                self.collect_types_with_resolver(
                    ret_type,
                    cmd_file,
                    resolver,
                    &mut resolved_types,
                    &mut conflicts,
                );
            }
        }

        // Recursively add nested types
        let mut to_process: Vec<(String, PathBuf)> = resolved_types
            .iter()
            .map(|(name, path)| (name.clone(), path.clone()))
            .collect();
        let mut processed: HashSet<(String, PathBuf)> = HashSet::new();

        while let Some((type_name, type_file)) = to_process.pop() {
            let key = (type_name.clone(), type_file.clone());
            if processed.contains(&key) {
                continue;
            }
            processed.insert(key);

            // Check if it's a struct in this file
            if let Some(s) = struct_by_file.get(&(type_name.as_str(), type_file.as_path())) {
                for field in &s.fields {
                    let nested_names = collect_custom_types_from_rust_type(&field.ty);
                    for t in nested_names {
                        match resolver.resolve_type(&t, &type_file) {
                             ResolutionResult::Found(source) => {
                                let simple_name = t.split("::").last().unwrap_or(&t).to_string();
                                if let Some(existing) = resolved_types.get(&simple_name) {
                                    if existing != &source {
                                        let conflict_list = conflicts
                                            .entry(simple_name.clone())
                                            .or_insert_with(|| vec![existing.clone()]);
                                        if !conflict_list.contains(&source) {
                                            conflict_list.push(source);
                                        }
                                    }
                                } else {
                                    resolved_types.insert(simple_name.clone(), source.clone());
                                    // We need to look up the definition using the simple name, 
                                    // but we continue processing using the *resolved* source file 
                                    // and the simple name which should match the struct definition.
                                    to_process.push((simple_name, source));
                                }
                            }
                            ResolutionResult::Ambiguous(_) => {
                                eprintln!("Warning: Ambiguous type '{}' used in {}", t, type_file.display());
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Check if it's an enum in this file
            if let Some(e) = enum_by_file.get(&(type_name.as_str(), type_file.as_path())) {
                for variant in &e.variants {
                    let nested_names = match &variant.data {
                        crate::models::VariantData::Unit => vec![],
                        crate::models::VariantData::Tuple(types) => types
                            .iter()
                            .flat_map(collect_custom_types_from_rust_type)
                            .collect(),
                        crate::models::VariantData::Struct(fields) => fields
                            .iter()
                            .flat_map(|f| collect_custom_types_from_rust_type(&f.ty))
                            .collect(),
                    };
                    for t in nested_names {
                        match resolver.resolve_type(&t, &type_file) {
                             ResolutionResult::Found(source) => {
                                let simple_name = t.split("::").last().unwrap_or(&t).to_string();
                                if let Some(existing) = resolved_types.get(&simple_name) {
                                    if existing != &source {
                                        let conflict_list = conflicts
                                            .entry(simple_name.clone())
                                            .or_insert_with(|| vec![existing.clone()]);
                                        if !conflict_list.contains(&source) {
                                            conflict_list.push(source);
                                        }
                                    }
                                } else {
                                    resolved_types.insert(simple_name.clone(), source.clone());
                                    to_process.push((simple_name, source));
                                }
                            }
                            ResolutionResult::Ambiguous(_) => {
                                eprintln!("Warning: Ambiguous type '{}' used in {}", t, type_file.display());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        TypeCollectionResult {
            resolved: resolved_types,
            conflicts,
        }
    }

    /// Collect types from RustType, resolving source files via resolver
    fn collect_types_with_resolver(
        &self,
        ty: &RustType,
        from_file: &Path,
        resolver: &ModuleResolver,
        resolved: &mut HashMap<String, PathBuf>,
        conflicts: &mut HashMap<String, Vec<PathBuf>>,
    ) {
        match ty {
            RustType::Custom(name) => {
                match resolver.resolve_type(name, from_file) {
                    ResolutionResult::Found(source) => {
                        let simple_name = name.split("::").last().unwrap_or(name).to_string();
                        if let Some(existing) = resolved.get(&simple_name) {
                            // Check for conflict: same name, different source file
                            if existing != &source {
                                let conflict_list = conflicts
                                    .entry(simple_name.clone())
                                    .or_insert_with(|| vec![existing.clone()]);
                                if !conflict_list.contains(&source) {
                                    conflict_list.push(source);
                                }
                            }
                        } else {
                            resolved.insert(simple_name, source);
                        }
                    }
                     ResolutionResult::Ambiguous(paths) => {
                         eprintln!("Warning: Ambiguous type '{}' in {}. Found in: {:?}", name, from_file.display(), paths);
                     }
                    _ => {}
                }
            }
            RustType::Vec(inner) => {
                self.collect_types_with_resolver(inner, from_file, resolver, resolved, conflicts)
            }
            RustType::Option(inner) => {
                self.collect_types_with_resolver(inner, from_file, resolver, resolved, conflicts)
            }
            RustType::Result(ok) => {
                self.collect_types_with_resolver(ok, from_file, resolver, resolved, conflicts)
            }
            RustType::HashMap { key, value } => {
                self.collect_types_with_resolver(key, from_file, resolver, resolved, conflicts);
                self.collect_types_with_resolver(value, from_file, resolver, resolved, conflicts);
            }
            RustType::Tuple(tuple_types) => {
                for t in tuple_types {
                    self.collect_types_with_resolver(t, from_file, resolver, resolved, conflicts);
                }
            }
            _ => {}
        }
    }

    /// Step 5: Filter structs and enums based on resolved types
    fn filter_types(
        &self,
        parse_result: &ParseResult,
        used_types: &HashMap<String, PathBuf>,
    ) -> (Vec<RustStruct>, Vec<RustEnum>) {
        let mut filtered_structs: Vec<RustStruct> = Vec::new();
        let mut seen_struct_names: HashSet<String> = HashSet::new();

        for s in parse_result.structs.iter() {
            if seen_struct_names.contains(&s.name) {
                continue;
            }

            // Only include if this specific struct (by name AND source file) was resolved
            if let Some(resolved_file) = used_types.get(&s.name) {
                if &s.source_file == resolved_file {
                    seen_struct_names.insert(s.name.clone());
                    filtered_structs.push(s.clone());
                }
            }
        }

        let mut filtered_enums: Vec<RustEnum> = Vec::new();
        let mut seen_enum_names: HashSet<String> = HashSet::new();

        for e in parse_result.enums.iter() {
            if seen_enum_names.contains(&e.name) {
                continue;
            }

            // Only include if this specific enum (by name AND source file) was resolved
            if let Some(resolved_file) = used_types.get(&e.name) {
                if &e.source_file == resolved_file {
                    seen_enum_names.insert(e.name.clone());
                    filtered_enums.push(e.clone());
                }
            }
        }

        (filtered_structs, filtered_enums)
    }

    /// Step 6: Generate TypeScript output files
    fn generate_output(
        &self,
        config: &Config,
        parse_result: &ParseResult,
        filtered_structs: &[RustStruct],
        filtered_enums: &[RustEnum],
    ) -> Result<()> {
        // Create generator context
        let mut ctx = GeneratorContext::new(config.naming.clone());

        for s in filtered_structs {
            ctx.register_type(&s.name);
        }
        for e in filtered_enums {
            ctx.register_type(&e.name);
        }

        // Generate types.ts
        let types_content = generate_types_file(filtered_structs, filtered_enums, &ctx);

        if let Some(parent) = config.output.types_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(&config.output.types_file, &types_content).with_context(|| {
            format!(
                "Failed to write types file: {}",
                config.output.types_file.display()
            )
        })?;

        println!("Generated: {}", config.output.types_file.display());

        // Generate commands.ts
        let commands_content = generate_commands_file(
            &parse_result.commands,
            &config.output.types_file,
            &config.output.commands_file,
            &ctx,
        );

        if let Some(parent) = config.output.commands_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(&config.output.commands_file, &commands_content).with_context(|| {
            format!(
                "Failed to write commands file: {}",
                config.output.commands_file.display()
            )
        })?;

        println!("Generated: {}", config.output.commands_file.display());

        Ok(())
    }
}

/// Collect custom type names from a RustType (returns a Vec)
fn collect_custom_types_from_rust_type(ty: &RustType) -> Vec<String> {
    let mut types = HashSet::new();
    collect_custom_types_recursive(ty, &mut types);
    let mut result: Vec<String> = types.into_iter().collect();
    result.sort();
    result
}

fn collect_custom_types_recursive(ty: &RustType, types: &mut HashSet<String>) {
    match ty {
        RustType::Custom(name) => {
             types.insert(name.clone());
        }
        RustType::Vec(inner) => collect_custom_types_recursive(inner, types),
        RustType::Option(inner) => collect_custom_types_recursive(inner, types),
        RustType::Result(ok) => collect_custom_types_recursive(ok, types),
        RustType::HashMap { key, value } => {
            collect_custom_types_recursive(key, types);
            collect_custom_types_recursive(value, types);
        }
        RustType::Tuple(tuple_types) => {
            for t in tuple_types {
                collect_custom_types_recursive(t, types);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CommandArg, EnumVariant, StructField, TauriCommand, VariantData};

    fn test_path() -> PathBuf {
        PathBuf::from("test.rs")
    }

    #[test]
    fn test_collect_custom_types_simple() {
        let ty = RustType::Custom("User".to_string());
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["User"]);
    }

    #[test]
    fn test_collect_custom_types_primitive() {
        let ty = RustType::Primitive("String".to_string());
        let types = collect_custom_types_from_rust_type(&ty);
        assert!(types.is_empty());
    }

    #[test]
    fn test_collect_custom_types_vec() {
        let ty = RustType::Vec(Box::new(RustType::Custom("Item".to_string())));
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["Item"]);
    }

    #[test]
    fn test_collect_custom_types_option() {
        let ty = RustType::Option(Box::new(RustType::Custom("User".to_string())));
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["User"]);
    }

    #[test]
    fn test_collect_custom_types_result() {
        let ty = RustType::Result(Box::new(RustType::Custom("Response".to_string())));
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["Response"]);
    }

    #[test]
    fn test_collect_custom_types_hashmap() {
        let ty = RustType::HashMap {
            key: Box::new(RustType::Primitive("String".to_string())),
            value: Box::new(RustType::Custom("User".to_string())),
        };
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["User"]);
    }

    #[test]
    fn test_collect_custom_types_tuple() {
        let ty = RustType::Tuple(vec![
            RustType::Custom("User".to_string()),
            RustType::Custom("Item".to_string()),
            RustType::Primitive("i32".to_string()),
        ]);
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["Item", "User"]);
    }

    #[test]
    fn test_collect_custom_types_nested() {
        let ty = RustType::Vec(Box::new(RustType::Option(Box::new(RustType::Custom(
            "User".to_string(),
        )))));
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["User"]);
    }

    #[test]
    fn test_collect_custom_types_no_duplicates() {
        let ty = RustType::Tuple(vec![
            RustType::Custom("User".to_string()),
            RustType::Custom("User".to_string()),
        ]);
        let types = collect_custom_types_from_rust_type(&ty);
        assert_eq!(types, vec!["User"]);
    }

    #[test]
    fn test_filter_types_includes_used() {
        let pipeline = Pipeline::new(false);

        let parse_result = ParseResult {
            commands: vec![],
            structs: vec![
                RustStruct {
                    name: "User".to_string(),
                    generics: vec![],
                    fields: vec![],
                    source_file: PathBuf::from("src/types.rs"),
                },
                RustStruct {
                    name: "Item".to_string(),
                    generics: vec![],
                    fields: vec![],
                    source_file: PathBuf::from("src/types.rs"),
                },
            ],
            enums: vec![],
        };

        let mut used_types = HashMap::new();
        used_types.insert("User".to_string(), PathBuf::from("src/types.rs"));

        let (filtered_structs, _) = pipeline.filter_types(&parse_result, &used_types);

        assert_eq!(filtered_structs.len(), 1);
        assert_eq!(filtered_structs[0].name, "User");
    }

    #[test]
    fn test_filter_types_excludes_unused() {
        let pipeline = Pipeline::new(false);

        let parse_result = ParseResult {
            commands: vec![],
            structs: vec![RustStruct {
                name: "UnusedType".to_string(),
                generics: vec![],
                fields: vec![],
                source_file: test_path(),
            }],
            enums: vec![],
        };

        let used_types = HashMap::new(); // Empty - no types used

        let (filtered_structs, _) = pipeline.filter_types(&parse_result, &used_types);

        assert!(filtered_structs.is_empty());
    }

    #[test]
    fn test_filter_types_respects_source_file() {
        let pipeline = Pipeline::new(false);

        let parse_result = ParseResult {
            commands: vec![],
            structs: vec![
                RustStruct {
                    name: "User".to_string(),
                    generics: vec![],
                    fields: vec![],
                    source_file: PathBuf::from("src/a.rs"),
                },
                RustStruct {
                    name: "User".to_string(),
                    generics: vec![],
                    fields: vec![],
                    source_file: PathBuf::from("src/b.rs"),
                },
            ],
            enums: vec![],
        };

        let mut used_types = HashMap::new();
        // Only src/a.rs version is used
        used_types.insert("User".to_string(), PathBuf::from("src/a.rs"));

        let (filtered_structs, _) = pipeline.filter_types(&parse_result, &used_types);

        assert_eq!(filtered_structs.len(), 1);
        assert_eq!(filtered_structs[0].source_file, PathBuf::from("src/a.rs"));
    }

    #[test]
    fn test_filter_enums() {
        let pipeline = Pipeline::new(false);

        let parse_result = ParseResult {
            commands: vec![],
            structs: vec![],
            enums: vec![
                RustEnum {
                    name: "Status".to_string(),
                    variants: vec![EnumVariant {
                        name: "Active".to_string(),
                        data: VariantData::Unit,
                    }],
                    source_file: PathBuf::from("src/types.rs"),
                },
                RustEnum {
                    name: "UnusedEnum".to_string(),
                    variants: vec![],
                    source_file: PathBuf::from("src/types.rs"),
                },
            ],
        };

        let mut used_types = HashMap::new();
        used_types.insert("Status".to_string(), PathBuf::from("src/types.rs"));

        let (_, filtered_enums) = pipeline.filter_types(&parse_result, &used_types);

        assert_eq!(filtered_enums.len(), 1);
        assert_eq!(filtered_enums[0].name, "Status");
    }

    #[test]
    fn test_type_collection_detects_conflicts() {
        let pipeline = Pipeline::new(false);
        let mut resolver = ModuleResolver::new();

        // Two files defining User
        let code_a = r#"
            pub struct User { pub id: i32 }
        "#;
        resolver
            .parse_file(&PathBuf::from("src/a.rs"), code_a, &PathBuf::from("src"))
            .unwrap();

        let code_b = r#"
            pub struct User { pub name: String }
        "#;
        resolver
            .parse_file(&PathBuf::from("src/b.rs"), code_b, &PathBuf::from("src"))
            .unwrap();

        // Commands file that uses User but doesn't import explicitly
        let code_cmd = r#"
            fn some_fn() {}
        "#;
        resolver
            .parse_file(
                &PathBuf::from("src/commands.rs"),
                code_cmd,
                &PathBuf::from("src"),
            )
            .unwrap();

        let parse_result = ParseResult {
            commands: vec![
                TauriCommand {
                    name: "get_user_a".to_string(),
                    args: vec![],
                    return_type: Some(RustType::Custom("User".to_string())),
                    source_file: PathBuf::from("src/a.rs"),
                },
                TauriCommand {
                    name: "get_user_b".to_string(),
                    args: vec![],
                    return_type: Some(RustType::Custom("User".to_string())),
                    source_file: PathBuf::from("src/b.rs"),
                },
            ],
            structs: vec![
                RustStruct {
                    name: "User".to_string(),
                    generics: vec![],
                    fields: vec![StructField {
                        name: "id".to_string(),
                        ty: RustType::Primitive("i32".to_string()),
                    }],
                    source_file: PathBuf::from("src/a.rs"),
                },
                RustStruct {
                    name: "User".to_string(),
                    generics: vec![],
                    fields: vec![StructField {
                        name: "name".to_string(),
                        ty: RustType::Primitive("String".to_string()),
                    }],
                    source_file: PathBuf::from("src/b.rs"),
                },
            ],
            enums: vec![],
        };

        let result = pipeline.collect_used_types(&parse_result, &resolver);

        // Should detect a conflict since User comes from two different files
        assert!(result.conflicts.contains_key("User"));
    }

    #[test]
    fn test_type_collection_no_conflicts_single_source() {
        let pipeline = Pipeline::new(false);
        let mut resolver = ModuleResolver::new();

        let types_code = r#"
            pub struct User { pub id: i32 }
        "#;
        resolver
            .parse_file(
                &PathBuf::from("src/types.rs"),
                types_code,
                &PathBuf::from("src"),
            )
            .unwrap();

        let parse_result = ParseResult {
            commands: vec![TauriCommand {
                name: "get_user".to_string(),
                args: vec![],
                return_type: Some(RustType::Custom("User".to_string())),
                source_file: PathBuf::from("src/types.rs"),
            }],
            structs: vec![RustStruct {
                name: "User".to_string(),
                generics: vec![],
                fields: vec![],
                source_file: PathBuf::from("src/types.rs"),
            }],
            enums: vec![],
        };

        let result = pipeline.collect_used_types(&parse_result, &resolver);

        assert!(result.conflicts.is_empty());
        assert!(result.resolved.contains_key("User"));
    }

    #[test]
    fn test_pipeline_verbose_mode() {
        let pipeline = Pipeline::new(true);
        assert!(pipeline.verbose);

        let pipeline = Pipeline::new(false);
        assert!(!pipeline.verbose);
    }

    #[test]
    fn test_collect_types_from_args_and_return() {
        let pipeline = Pipeline::new(false);
        let mut resolver = ModuleResolver::new();

        let code = r#"
            pub struct Request { pub data: String }
            pub struct Response { pub result: i32 }
        "#;
        resolver
            .parse_file(
                &PathBuf::from("src/types.rs"),
                code,
                &PathBuf::from("src"),
            )
            .unwrap();

        let parse_result = ParseResult {
            commands: vec![TauriCommand {
                name: "process".to_string(),
                args: vec![CommandArg {
                    name: "req".to_string(),
                    ty: RustType::Custom("Request".to_string()),
                }],
                return_type: Some(RustType::Custom("Response".to_string())),
                source_file: PathBuf::from("src/types.rs"),
            }],
            structs: vec![
                RustStruct {
                    name: "Request".to_string(),
                    generics: vec![],
                    fields: vec![],
                    source_file: PathBuf::from("src/types.rs"),
                },
                RustStruct {
                    name: "Response".to_string(),
                    generics: vec![],
                    fields: vec![],
                    source_file: PathBuf::from("src/types.rs"),
                },
            ],
            enums: vec![],
        };

        let result = pipeline.collect_used_types(&parse_result, &resolver);

        assert!(result.resolved.contains_key("Request"));
        assert!(result.resolved.contains_key("Response"));
    }
}

