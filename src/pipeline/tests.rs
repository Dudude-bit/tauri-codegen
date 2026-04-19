//! Unit tests extracted from the parent module.

use super::*;
use crate::models::{walk_custom_type_names, CommandArg, TauriCommand};
use std::collections::HashSet;

fn test_path() -> PathBuf {
    PathBuf::from("test.rs")
}

/// Test-local wrapper that exercises the shared model walker.
fn collect_custom_types_from_rust_type(ty: &RustType) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    walk_custom_type_names(ty, &mut |n| {
        set.insert(n.to_string());
    });
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    out
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

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

#[test]
fn test_collect_reachable_types_from_commands() {
    let pipeline = Pipeline::new(false);
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
            ty: RustType::Custom("Request".to_string()),
        }],
        return_type: Some(RustType::Custom("Response".to_string())),
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = pipeline.collect_reachable_types(&commands, &resolver, None);

    assert!(result.conflicts.is_empty());
    assert!(result.structs.iter().any(|s| s.name == "Request"));
    assert!(result.structs.iter().any(|s| s.name == "Response"));
}

#[test]
fn test_collect_reachable_types_includes_aliases() {
    let pipeline = Pipeline::new(false);
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
            ty: RustType::Custom("UserAlias".to_string()),
        }],
        return_type: None,
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = pipeline.collect_reachable_types(&commands, &resolver, None);

    assert!(result.aliases.iter().any(|a| a.name == "UserAlias"));
    assert!(result.structs.iter().any(|s| s.name == "User"));
}

#[test]
fn test_collect_reachable_types_detects_conflicts() {
    let pipeline = Pipeline::new(false);
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
        return_type: Some(RustType::Custom("User".to_string())),
        source_file: cmd_path,
        rename_all: None,
    }];

    let result = pipeline.collect_reachable_types(&commands, &resolver, None);

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
                ty: RustType::Custom("State".to_string()),
            },
            CommandArg {
                name: "window".to_string(),
                ty: RustType::Custom("Window".to_string()),
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
                ty: RustType::Custom("AppHandle".to_string()),
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
                ty: RustType::Custom("MyState".to_string()),
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
    let pipeline = Pipeline::new(false);
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
        return_type: Some(RustType::Custom("Node".to_string())),
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = pipeline.collect_reachable_types(&commands, &resolver, None);

    assert!(result.conflicts.is_empty());
    // Node must appear exactly once despite the self-reference.
    let node_count = result.structs.iter().filter(|s| s.name == "Node").count();
    assert_eq!(node_count, 1, "Self-referential struct must be deduped");
}

#[test]
fn test_collect_reachable_types_handles_mutually_recursive_structs() {
    // A -> B -> A cycle. Without cycle protection the fixpoint loop
    // would oscillate between the two types forever.
    let pipeline = Pipeline::new(false);
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
        return_type: Some(RustType::Custom("A".to_string())),
        source_file: types_path.clone(),
        rename_all: None,
    }];

    let result = pipeline.collect_reachable_types(&commands, &resolver, None);

    assert_eq!(result.structs.iter().filter(|s| s.name == "A").count(), 1);
    assert_eq!(result.structs.iter().filter(|s| s.name == "B").count(), 1);
}
