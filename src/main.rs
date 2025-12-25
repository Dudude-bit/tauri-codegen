mod cli;
mod config;
mod generator;
mod parser;
mod resolver;
mod scanner;

use anyhow::{Context, Result};
use cli::{Cli, Commands};
use config::Config;
use generator::{
    commands_gen::generate_commands_file, types_gen::generate_types_file, GeneratorContext,
};
use parser::{command::parse_commands, types::parse_types, ParseResult, RustType, RustStruct, RustEnum};
use resolver::ModuleResolver;
use scanner::Scanner;
use std::collections::HashSet;
use std::fs;

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    match cli.command {
        Commands::Generate { config, verbose } => {
            run_generate(&config, verbose)?;
        }
        Commands::Init { output, force } => {
            run_init(&output, force)?;
        }
    }

    Ok(())
}

/// Run the generate command
fn run_generate(config_path: &std::path::Path, verbose: bool) -> Result<()> {
    let config = Config::load(config_path)?;

    if verbose {
        println!("Loaded configuration from: {}", config_path.display());
        println!("Scanning directory: {}", config.input.source_dir.display());
    }

    // Scan for Rust files
    let scanner = Scanner::new(
        config.input.source_dir.clone(),
        config.input.exclude.clone(),
    );
    let rust_files = scanner.scan()?;

    if verbose {
        println!("Found {} Rust files", rust_files.len());
    }

    // Build module resolver for import/scope analysis
    let mut resolver = ModuleResolver::new();
    let base_path = config.input.source_dir.clone();

    // Parse all files
    let mut parse_result = ParseResult::new();
    let mut command_files: HashSet<std::path::PathBuf> = HashSet::new();

    for file_path in &rust_files {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        // Build resolver scope for this file
        if let Err(e) = resolver.parse_file(file_path, &content, &base_path) {
            if verbose {
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
                if !commands.is_empty() {
                    command_files.insert(file_path.clone());
                    if verbose {
                        println!(
                            "Found {} commands in {}",
                            commands.len(),
                            file_path.display()
                        );
                    }
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
                if verbose && (!structs.is_empty() || !enums.is_empty()) {
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

    // Collect only types that are used in commands (with resolver for scope-aware lookup)
    let type_collection = collect_used_types(&parse_result, &resolver);
    
    // Check for type name conflicts
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
    
    let used_types = type_collection.resolved;

    // Filter structs and enums based on resolved types
    // Only include types that were explicitly resolved (no fallback by name)
    let mut filtered_structs: Vec<_> = Vec::new();
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

    let mut filtered_enums: Vec<_> = Vec::new();
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

    // Summary
    println!(
        "Parsed {} commands, {} structs (used), {} enums (used)",
        parse_result.commands.len(),
        filtered_structs.len(),
        filtered_enums.len()
    );

    // Create generator context
    let mut ctx = GeneratorContext::new(config.naming.clone());

    for s in &filtered_structs {
        ctx.register_type(&s.name);
    }
    for e in &filtered_enums {
        ctx.register_type(&e.name);
    }

    // Generate types.ts
    let types_content = generate_types_file(&filtered_structs, &filtered_enums, &ctx);

    if let Some(parent) = config.output.types_file.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::write(&config.output.types_file, &types_content)
        .with_context(|| format!("Failed to write types file: {}", config.output.types_file.display()))?;

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

    fs::write(&config.output.commands_file, &commands_content)
        .with_context(|| format!("Failed to write commands file: {}", config.output.commands_file.display()))?;

    println!("Generated: {}", config.output.commands_file.display());

    println!("Done!");

    Ok(())
}

/// Run the init command
fn run_init(output_path: &std::path::Path, force: bool) -> Result<()> {
    if output_path.exists() && !force {
        anyhow::bail!(
            "Configuration file already exists: {}. Use --force to overwrite.",
            output_path.display()
        );
    }

    let config = Config::default_config();
    config.save(output_path)?;

    println!("Created configuration file: {}", output_path.display());
    println!("\nEdit the file to configure:");
    println!("  - source_dir: Path to your Rust source files");
    println!("  - types_file: Output path for TypeScript types");
    println!("  - commands_file: Output path for TypeScript commands");
    println!("  - exclude: Directories to skip during scanning");

    Ok(())
}

/// Result of type collection with potential conflicts
struct TypeCollectionResult {
    /// Successfully resolved types: name -> source file
    resolved: std::collections::HashMap<String, std::path::PathBuf>,
    /// Conflicts: type name -> list of conflicting source files
    conflicts: std::collections::HashMap<String, Vec<std::path::PathBuf>>,
}

/// Collect all types used in commands, resolving their source files using the module resolver
fn collect_used_types(
    parse_result: &ParseResult,
    resolver: &ModuleResolver,
) -> TypeCollectionResult {
    let mut resolved_types: std::collections::HashMap<String, std::path::PathBuf> = std::collections::HashMap::new();
    let mut conflicts: std::collections::HashMap<String, Vec<std::path::PathBuf>> = std::collections::HashMap::new();

    // Build lookup maps: (name, source_file) -> type
    let struct_by_file: std::collections::HashMap<(&str, &std::path::Path), &RustStruct> = parse_result
        .structs
        .iter()
        .map(|s| ((s.name.as_str(), s.source_file.as_path()), s))
        .collect();
    let enum_by_file: std::collections::HashMap<(&str, &std::path::Path), &RustEnum> = parse_result
        .enums
        .iter()
        .map(|e| ((e.name.as_str(), e.source_file.as_path()), e))
        .collect();

    // Collect types from all commands, resolving source files
    for cmd in &parse_result.commands {
        let cmd_file = &cmd.source_file;
        
        for arg in &cmd.args {
            collect_types_with_resolver(&arg.ty, cmd_file, resolver, &mut resolved_types, &mut conflicts);
        }
        if let Some(ref ret_type) = cmd.return_type {
            collect_types_with_resolver(ret_type, cmd_file, resolver, &mut resolved_types, &mut conflicts);
        }
    }

    // Recursively add nested types
    let mut to_process: Vec<(String, std::path::PathBuf)> = resolved_types
        .iter()
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect();
    let mut processed: HashSet<(String, std::path::PathBuf)> = HashSet::new();

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
                    if let Some(source) = resolver.resolve_type(&t, &type_file) {
                        if let Some(existing) = resolved_types.get(&t) {
                            if existing != &source {
                                let conflict_list = conflicts.entry(t.clone()).or_insert_with(|| vec![existing.clone()]);
                                if !conflict_list.contains(&source) {
                                    conflict_list.push(source);
                                }
                            }
                        } else {
                            resolved_types.insert(t.clone(), source.clone());
                            to_process.push((t, source));
                        }
                    }
                }
            }
        }

        // Check if it's an enum in this file
        if let Some(e) = enum_by_file.get(&(type_name.as_str(), type_file.as_path())) {
            for variant in &e.variants {
                let nested_names = match &variant.data {
                    parser::VariantData::Unit => vec![],
                    parser::VariantData::Tuple(types) => {
                        types.iter().flat_map(collect_custom_types_from_rust_type).collect()
                    }
                    parser::VariantData::Struct(fields) => {
                        fields.iter().flat_map(|f| collect_custom_types_from_rust_type(&f.ty)).collect()
                    }
                };
                for t in nested_names {
                    if let Some(source) = resolver.resolve_type(&t, &type_file) {
                        if let Some(existing) = resolved_types.get(&t) {
                            if existing != &source {
                                let conflict_list = conflicts.entry(t.clone()).or_insert_with(|| vec![existing.clone()]);
                                if !conflict_list.contains(&source) {
                                    conflict_list.push(source);
                                }
                            }
                        } else {
                            resolved_types.insert(t.clone(), source.clone());
                            to_process.push((t, source));
                        }
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
/// Detects conflicts when same type name resolves to different files
fn collect_types_with_resolver(
    ty: &RustType,
    from_file: &std::path::Path,
    resolver: &ModuleResolver,
    resolved: &mut std::collections::HashMap<String, std::path::PathBuf>,
    conflicts: &mut std::collections::HashMap<String, Vec<std::path::PathBuf>>,
) {
    match ty {
        RustType::Custom(name) => {
            if let Some(source) = resolver.resolve_type(name, from_file) {
                if let Some(existing) = resolved.get(name) {
                    // Check for conflict: same name, different source file
                    if existing != &source {
                        let conflict_list = conflicts.entry(name.clone()).or_insert_with(|| vec![existing.clone()]);
                        if !conflict_list.contains(&source) {
                            conflict_list.push(source);
                        }
                    }
                } else {
                    resolved.insert(name.clone(), source);
                }
            }
        }
        RustType::Vec(inner) => collect_types_with_resolver(inner, from_file, resolver, resolved, conflicts),
        RustType::Option(inner) => collect_types_with_resolver(inner, from_file, resolver, resolved, conflicts),
        RustType::Result(ok) => collect_types_with_resolver(ok, from_file, resolver, resolved, conflicts),
        RustType::HashMap { key, value } => {
            collect_types_with_resolver(key, from_file, resolver, resolved, conflicts);
            collect_types_with_resolver(value, from_file, resolver, resolved, conflicts);
        }
        RustType::Tuple(tuple_types) => {
            for t in tuple_types {
                collect_types_with_resolver(t, from_file, resolver, resolved, conflicts);
            }
        }
        _ => {}
    }
}

/// Collect custom type names from a RustType (returns a Vec)
fn collect_custom_types_from_rust_type(ty: &RustType) -> Vec<String> {
    let mut types = Vec::new();
    collect_custom_types_recursive(ty, &mut types);
    types
}

fn collect_custom_types_recursive(ty: &RustType, types: &mut Vec<String>) {
    match ty {
        RustType::Custom(name) => {
            if !types.contains(name) {
                types.push(name.clone());
            }
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
