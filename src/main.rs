mod cli;
mod config;
mod generator;
mod parser;
mod scanner;

use anyhow::{Context, Result};
use cli::{Cli, Commands};
use config::Config;
use generator::{
    commands_gen::generate_commands_file, types_gen::generate_types_file, GeneratorContext,
};
use parser::{command::parse_commands, types::parse_types, ParseResult};
use scanner::Scanner;
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
    // Load configuration
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

    // Parse all files
    let mut parse_result = ParseResult::new();

    for file_path in &rust_files {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        // Parse commands
        match parse_commands(&content, file_path) {
            Ok(commands) => {
                if verbose && !commands.is_empty() {
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

    // Summary
    println!(
        "Parsed {} commands, {} structs, {} enums",
        parse_result.commands.len(),
        parse_result.structs.len(),
        parse_result.enums.len()
    );

    // Create generator context
    let mut ctx = GeneratorContext::new(config.naming.clone());

    // Register all custom types
    for s in &parse_result.structs {
        ctx.register_type(&s.name);
    }
    for e in &parse_result.enums {
        ctx.register_type(&e.name);
    }

    // Generate types.ts
    let types_content =
        generate_types_file(&parse_result.structs, &parse_result.enums, &ctx);

    // Ensure output directory exists
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

    // Ensure output directory exists
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
    // Check if file already exists
    if output_path.exists() && !force {
        anyhow::bail!(
            "Configuration file already exists: {}. Use --force to overwrite.",
            output_path.display()
        );
    }

    // Create default configuration
    let config = Config::default_config();

    // Save to file
    config.save(output_path)?;

    println!("Created configuration file: {}", output_path.display());
    println!("\nEdit the file to configure:");
    println!("  - source_dir: Path to your Rust source files");
    println!("  - types_file: Output path for TypeScript types");
    println!("  - commands_file: Output path for TypeScript commands");
    println!("  - exclude: Directories to skip during scanning");

    Ok(())
}
