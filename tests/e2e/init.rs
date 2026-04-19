//! `tauri-ts-generator init` should scaffold a config that `generate`
//! can then consume.

use std::process::Command;

use crate::helpers::binary_path;

#[test]
fn init_creates_valid_config_then_generate_runs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // 1. init
    let output = Command::new(binary_path())
        .current_dir(root)
        .arg("init")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "init failed:\nstderr={}\nstdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let config_path = root.join("tauri-codegen.toml");
    assert!(config_path.exists(), "init must produce tauri-codegen.toml");
    let config = std::fs::read_to_string(&config_path).unwrap();
    assert!(config.contains("[input]"));
    assert!(config.contains("[output]"));

    // 2. provide a minimal source tree matching the default config.
    let src = root.join("src-tauri").join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("lib.rs"),
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct Hello { pub msg: String }
        #[tauri::command]
        fn hello() -> Result<Hello, String> { todo!() }
        "#,
    )
    .unwrap();

    // 3. generate against the freshly-initialised config.
    let output = Command::new(binary_path())
        .current_dir(root)
        .arg("generate")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generate after init failed:\nstderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn init_refuses_to_clobber_existing_config_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = root.join("tauri-codegen.toml");
    std::fs::write(&config, "# my existing config").unwrap();

    let output = Command::new(binary_path())
        .current_dir(root)
        .arg("init")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "init without --force should fail on existing file"
    );
    // Config must be untouched.
    let preserved = std::fs::read_to_string(&config).unwrap();
    assert_eq!(preserved, "# my existing config");
}

#[test]
fn init_force_overwrites_existing_config() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = root.join("tauri-codegen.toml");
    std::fs::write(&config, "stale").unwrap();

    let output = Command::new(binary_path())
        .current_dir(root)
        .arg("init")
        .arg("--force")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "init --force failed:\nstderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let new_content = std::fs::read_to_string(&config).unwrap();
    assert_ne!(new_content, "stale");
    assert!(new_content.contains("[input]"));
}
