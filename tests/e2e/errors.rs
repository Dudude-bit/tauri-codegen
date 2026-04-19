//! CLI should fail loudly on user errors (bad config, conflicts) and warn
//! without failing on unresolvable types.

use std::fs;

use crate::helpers::{assert_contains, binary_path, run_generate_err, Project};

#[test]
fn missing_config_fails_with_actionable_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(binary_path())
        .current_dir(dir.path())
        .arg("generate")
        .output()
        .unwrap();
    assert!(!output.status.success(), "should fail without config");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert_contains(&combined, "tauri-codegen.toml");
}

#[test]
fn duplicate_type_from_two_files_errors() {
    // The command is in `lib.rs` and does *not* define or import `User`;
    // two sibling modules each define a different `User`. That's ambiguous
    // — the resolver should bail out instead of silently picking one.
    let project = Project::with_source(
        r#"
        #[tauri::command]
        fn x() -> Result<User, String> { todo!() }
        "#,
    );
    project.add_source(
        "a.rs",
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct User { pub a: i32 }
        "#,
    );
    project.add_source(
        "b.rs",
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct User { pub b: String }
        "#,
    );

    let output = run_generate_err(&project);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_contains(&stderr, "conflict");
}

#[test]
fn duplicate_command_name_errors() {
    // Two #[tauri::command] functions with the same name across different
    // files would silently collide in the generated TS; the pipeline must
    // refuse and report every involved file.
    let project = Project::with_source(
        r#"
        #[tauri::command]
        fn greet() -> Result<(), String> { todo!() }
        "#,
    );
    project.add_source(
        "other.rs",
        r#"
        #[tauri::command]
        fn greet() -> Result<(), String> { todo!() }
        "#,
    );

    let output = run_generate_err(&project);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_contains(&stderr, "Duplicate #[tauri::command]");
    assert_contains(&stderr, "greet");
}

#[test]
fn unresolved_type_warns_but_does_not_fail() {
    // `Mystery` is referenced by a command but never defined anywhere.
    // This is the shape "macro-generated types" take; the pipeline should
    // warn the user and proceed, not crash.
    let project = Project::with_source(
        r#"
        #[tauri::command]
        fn x() -> Result<crate::unknown::Mystery, String> { todo!() }
        "#,
    );

    let output = crate::helpers::run_generate(&project);
    assert!(
        output.status.success(),
        "unresolved types should produce a warning, not a failure. stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        combined.contains("Mystery")
            || combined.to_lowercase().contains("unresolved")
            || combined.to_lowercase().contains("could not be resolved"),
        "expected a mention of the unresolved type:\n{combined}"
    );
    // Generator should still have emitted files.
    assert!(fs::metadata(&project.types_out).is_ok());
    assert!(fs::metadata(&project.commands_out).is_ok());
}
