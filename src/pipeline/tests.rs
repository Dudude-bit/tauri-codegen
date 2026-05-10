//! Unit tests extracted from the parent module.

use super::*;
use crate::models::{CommandArg, TauriCommand};

fn test_path() -> PathBuf {
    PathBuf::from("test.rs")
}

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

#[test]
fn test_collect_reachable_types_from_commands() {
    // direct free-fn invocation keeps tests tied to the public collect API
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    let types_path = src_dir.join("types.rs");
    let types_code = r#"
        pub struct Request { pub data: String }
        pub struct Response { pub result: i32 }
    "#;
    write_file(&types_path, types_code);

    let mut resolver = ModuleResolver::new();
    resolver
        .parse_file(&types_path, types_code, &src_dir)
        .unwrap();

    let commands = vec![TauriCommand {
        name: "process".to_string(),
        args: vec![CommandArg {
            name: "req".to_string(),
            ty: RustType::custom("Request"),
        }],
        return_type: Some(RustType::custom("Response")),
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = collect::collect_reachable_types(
        &commands,
        &resolver,
        None,
        &crate::diagnostics::Diagnostics::new(false),
    );

    assert!(result.conflicts.is_empty());
    assert!(result.structs.iter().any(|s| s.name == "Request"));
    assert!(result.structs.iter().any(|s| s.name == "Response"));
}

#[test]
fn test_collect_reachable_types_includes_aliases() {
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    let types_path = src_dir.join("types.rs");
    let types_code = r#"
        pub struct User { pub id: i32 }
        pub type UserAlias = User;
    "#;
    write_file(&types_path, types_code);

    let mut resolver = ModuleResolver::new();
    resolver
        .parse_file(&types_path, types_code, &src_dir)
        .unwrap();

    let commands = vec![TauriCommand {
        name: "get_user".to_string(),
        args: vec![CommandArg {
            name: "user".to_string(),
            ty: RustType::custom("UserAlias"),
        }],
        return_type: None,
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = collect::collect_reachable_types(
        &commands,
        &resolver,
        None,
        &crate::diagnostics::Diagnostics::new(false),
    );

    assert!(result.aliases.iter().any(|a| a.name == "UserAlias"));
    assert!(result.structs.iter().any(|s| s.name == "User"));
}

#[test]
fn test_collect_reachable_types_detects_conflicts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    let a_path = src_dir.join("a.rs");
    let b_path = src_dir.join("b.rs");
    let cmd_path = src_dir.join("commands.rs");
    let code_a = "pub struct User { pub id: i32 }";
    let code_b = "pub struct User { pub name: String }";
    let cmd_code = "fn some_fn() {}";

    write_file(&a_path, code_a);
    write_file(&b_path, code_b);
    write_file(&cmd_path, cmd_code);

    let mut resolver = ModuleResolver::new();
    resolver.parse_file(&a_path, code_a, &src_dir).unwrap();
    resolver.parse_file(&b_path, code_b, &src_dir).unwrap();
    resolver.parse_file(&cmd_path, cmd_code, &src_dir).unwrap();

    let commands = vec![TauriCommand {
        name: "get_user".to_string(),
        args: vec![],
        return_type: Some(RustType::custom("User")),
        source_file: cmd_path,
        rename_all: None,
    }];

    let result = collect::collect_reachable_types(
        &commands,
        &resolver,
        None,
        &crate::diagnostics::Diagnostics::new(false),
    );

    assert!(result.conflicts.contains_key("User"));
}

#[test]
fn test_pipeline_verbose_mode() {
    assert!(Pipeline::new(true).diag.verbose());
    assert!(!Pipeline::new(false).diag.verbose());
}

#[test]
fn test_filter_tauri_special_types() {
    let pipeline = Pipeline::new(false);
    let resolver = ModuleResolver::new();

    // Create a command with special Tauri types
    let mut commands = vec![TauriCommand {
        name: "test_command".to_string(),
        args: vec![
            CommandArg {
                name: "state".to_string(),
                ty: RustType::custom("State"),
            },
            CommandArg {
                name: "window".to_string(),
                ty: RustType::custom("Window"),
            },
            CommandArg {
                name: "id".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            },
        ],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    }];

    pipeline.filter_tauri_special_args(&mut commands, &resolver);

    // State and Window should be filtered out
    assert_eq!(commands[0].args.len(), 1);
    assert_eq!(commands[0].args[0].name, "id");
}

#[test]
fn test_filter_tauri_app_handle() {
    let pipeline = Pipeline::new(false);
    let resolver = ModuleResolver::new();

    let mut commands = vec![TauriCommand {
        name: "with_app".to_string(),
        args: vec![
            CommandArg {
                name: "app".to_string(),
                ty: RustType::custom("AppHandle"),
            },
            CommandArg {
                name: "data".to_string(),
                ty: RustType::Primitive("String".to_string()),
            },
        ],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    }];

    pipeline.filter_tauri_special_args(&mut commands, &resolver);

    // AppHandle should be filtered out
    assert_eq!(commands[0].args.len(), 1);
    assert_eq!(commands[0].args[0].name, "data");
}

#[test]
fn test_filter_tauri_special_types_via_alias() {
    let pipeline = Pipeline::new(false);
    let mut resolver = ModuleResolver::new();

    // Register a type alias: type MyState = State<AppState>
    let code = "pub type MyState<'a> = State<'a, AppState>;";
    let path = test_path();
    resolver
        .parse_file(&path, code, &PathBuf::from("."))
        .unwrap();

    let mut commands = vec![TauriCommand {
        name: "aliased_command".to_string(),
        args: vec![
            CommandArg {
                name: "state".to_string(),
                ty: RustType::custom("MyState"),
            },
            CommandArg {
                name: "id".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            },
        ],
        return_type: None,
        source_file: path.clone(),
        rename_all: None,
    }];

    pipeline.filter_tauri_special_args(&mut commands, &resolver);

    // MyState (alias to State) should be filtered out
    assert_eq!(commands[0].args.len(), 1);
    assert_eq!(commands[0].args[0].name, "id");
}

#[test]
fn test_collect_reachable_types_handles_self_referential_struct() {
    // A self-referential tree node is the canonical stress test for the
    // reachable-type fixpoint loop: without dedup, this would recurse forever.
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    let types_path = src_dir.join("types.rs");
    let types_code = r#"
        pub struct Node {
            pub value: i32,
            pub next: Option<Box<Node>>,
        }
    "#;
    write_file(&types_path, types_code);

    let mut resolver = ModuleResolver::new();
    resolver
        .parse_file(&types_path, types_code, &src_dir)
        .unwrap();

    let commands = vec![TauriCommand {
        name: "get_head".to_string(),
        args: vec![],
        return_type: Some(RustType::custom("Node")),
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = collect::collect_reachable_types(
        &commands,
        &resolver,
        None,
        &crate::diagnostics::Diagnostics::new(false),
    );

    assert!(result.conflicts.is_empty());
    // Node must appear exactly once despite the self-reference.
    let node_count = result.structs.iter().filter(|s| s.name == "Node").count();
    assert_eq!(node_count, 1, "Self-referential struct must be deduped");
}

#[test]
fn test_collect_reachable_types_handles_mutually_recursive_structs() {
    // A -> B -> A cycle. Without cycle protection the fixpoint loop
    // would oscillate between the two types forever.
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    let types_path = src_dir.join("types.rs");
    let types_code = r#"
        pub struct A { pub b: Option<Box<B>> }
        pub struct B { pub a: Option<Box<A>> }
    "#;
    write_file(&types_path, types_code);

    let mut resolver = ModuleResolver::new();
    resolver
        .parse_file(&types_path, types_code, &src_dir)
        .unwrap();

    let commands = vec![TauriCommand {
        name: "get_a".to_string(),
        args: vec![],
        return_type: Some(RustType::custom("A")),
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = collect::collect_reachable_types(
        &commands,
        &resolver,
        None,
        &crate::diagnostics::Diagnostics::new(false),
    );

    assert_eq!(result.structs.iter().filter(|s| s.name == "A").count(), 1);
    assert_eq!(result.structs.iter().filter(|s| s.name == "B").count(), 1);
}

#[test]
fn parse_files_picks_up_macro_generated_commands_from_expanded_code() {
    // A `macro_rules!` that stamps out N `#[tauri::command]` functions
    // shows up in raw source as a single macro invocation — `parse_commands`
    // sees no commands. cargo expand replaces the call site with the
    // actual functions, so we re-parse the expanded blob and merge.
    // Without this the generated TS misses every macro-generated command.
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    // Raw file: just a macro invocation. `syn::parse_file` accepts this
    // (it's a valid `macro_rules!` call site), but `parse_commands` will
    // not extract anything because the body of the macro hasn't been
    // expanded yet.
    let raw_path = src_dir.join("watch.rs");
    write_file(
        &raw_path,
        r#"
            subscribe_namespaced!(ConfigMap, "configmap");
            subscribe_namespaced!(Secret, "secret");
        "#,
    );

    // Synthetic cargo-expand output: what `cargo expand` would produce
    // for the macro invocations above. Functions are now visible.
    let expanded_code = r#"
        #[tauri::command]
        pub async fn subscribe_configmap_watch(namespace: String) -> Result<String, String> {
            Ok(namespace)
        }
        #[tauri::command]
        pub async fn subscribe_secret_watch(namespace: String) -> Result<String, String> {
            Ok(namespace)
        }
    "#;

    let mut config = crate::config::Config::default_config();
    config.input.source_dir = src_dir.clone();

    let pipeline = Pipeline::new(false);
    let (commands, _resolver, _expanded_types) = pipeline
        .parse_files(&[raw_path], &config, Some(expanded_code))
        .expect("parse_files should succeed");

    let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
    assert!(
        names.contains(&"subscribe_configmap_watch"),
        "macro-generated subscribe_configmap_watch missing; got {:?}",
        names
    );
    assert!(
        names.contains(&"subscribe_secret_watch"),
        "macro-generated subscribe_secret_watch missing; got {:?}",
        names
    );
    let synthetic_path = PathBuf::from("<cargo-expand>");
    for cmd in &commands {
        assert_eq!(
            cmd.source_file, synthetic_path,
            "macro-generated command should carry the cargo-expand virtual path"
        );
    }
}

#[test]
fn parse_files_keeps_source_path_when_command_exists_in_both() {
    // If a command appears in both raw source AND in cargo expand output
    // (cargo expand prints out everything, not just macro-generated items),
    // the file-based entry wins so the original source path is preserved
    // for downstream diagnostics. The expanded duplicate is dropped.
    let temp_dir = tempfile::tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");

    let raw_path = src_dir.join("commands.rs");
    write_file(
        &raw_path,
        r#"
            #[tauri::command]
            pub async fn list_pods(namespace: String) -> Result<String, String> {
                Ok(namespace)
            }
        "#,
    );

    let expanded_code = r#"
        #[tauri::command]
        pub async fn list_pods(namespace: String) -> Result<String, String> {
            Ok(namespace)
        }
    "#;

    let mut config = crate::config::Config::default_config();
    config.input.source_dir = src_dir.clone();

    let pipeline = Pipeline::new(false);
    let (commands, _resolver, _expanded_types) = pipeline
        .parse_files(
            std::slice::from_ref(&raw_path),
            &config,
            Some(expanded_code),
        )
        .expect("parse_files should succeed");

    let pods: Vec<&TauriCommand> = commands.iter().filter(|c| c.name == "list_pods").collect();
    assert_eq!(
        pods.len(),
        1,
        "list_pods must be deduped, got {:?}",
        commands.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert_eq!(
        pods[0].source_file, raw_path,
        "raw-source entry must win over the cargo-expand duplicate"
    );
}
