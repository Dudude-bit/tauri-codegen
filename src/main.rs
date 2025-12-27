use anyhow::Result;
use tauri_ts_generator::cli::{Cli, Commands};
use tauri_ts_generator::config::Config;
use tauri_ts_generator::pipeline::Pipeline;

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
    }

    let pipeline = Pipeline::new(verbose);
    pipeline.run(&config)
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
