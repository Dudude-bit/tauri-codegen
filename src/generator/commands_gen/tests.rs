//! Unit tests extracted from the parent module.

use super::*;
use crate::config::NamingConfig;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    PathBuf::from("test.rs")
}

fn default_ctx() -> GeneratorContext {
    GeneratorContext::new(NamingConfig::default())
}

fn ctx_with_type(type_name: &str) -> GeneratorContext {
    let mut ctx = default_ctx();
    ctx.register_type(type_name);
    ctx
}

#[test]
fn test_generate_simple_command() {
    let cmd = TauriCommand {
        name: "get_user".to_string(),
        args: vec![CommandArg {
            name: "id".to_string(),
            ty: RustType::Primitive("i32".to_string()),
        }],
        return_type: Some(RustType::custom("User")),
        source_file: test_path(),
        rename_all: None,
    };

    let mut ctx = default_ctx();
    ctx.register_type("User");

    let output = generate_command_function(&cmd, &ctx);

    assert!(output.contains("export async function getUser"));
    assert!(output.contains("id: number"));
    assert!(output.contains("Promise<User>"));
    assert!(output.contains("invoke<User>(\"get_user\""));
}

#[test]
fn test_generate_command_no_args() {
    let cmd = TauriCommand {
        name: "get_all".to_string(),
        args: vec![],
        return_type: Some(RustType::Vec(Box::new(RustType::custom("Item")))),
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = ctx_with_type("Item");
    let output = generate_command_function(&cmd, &ctx);

    assert!(output.contains("export async function getAll()"));
    assert!(output.contains("Promise<Item[]>"));
    assert!(output.contains("invoke<Item[]>(\"get_all\")"));
    // Should NOT have second argument to invoke
    assert!(!output.contains("invoke<Item[]>(\"get_all\", {"));
}

#[test]
fn test_generate_command_multiple_args() {
    let cmd = TauriCommand {
        name: "create_user".to_string(),
        args: vec![
            CommandArg {
                name: "name".to_string(),
                ty: RustType::Primitive("String".to_string()),
            },
            CommandArg {
                name: "age".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            },
            CommandArg {
                name: "email".to_string(),
                ty: RustType::Option(Box::new(RustType::Primitive("String".to_string()))),
            },
        ],
        return_type: Some(RustType::custom("User")),
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = ctx_with_type("User");
    let output = generate_command_function(&cmd, &ctx);

    assert!(output.contains("name: string"));
    assert!(output.contains("age: number"));
    assert!(output.contains("email: string | null"));
}

#[test]
fn test_generate_void_return() {
    let cmd = TauriCommand {
        name: "delete_user".to_string(),
        args: vec![CommandArg {
            name: "id".to_string(),
            ty: RustType::Primitive("i32".to_string()),
        }],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = default_ctx();
    let output = generate_command_function(&cmd, &ctx);

    assert!(output.contains("Promise<void>"));
    assert!(output.contains("invoke<void>"));
}

#[test]
fn test_camel_case_function_name() {
    let cmd = TauriCommand {
        name: "get_user_by_id".to_string(),
        args: vec![],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = default_ctx();
    let output = generate_command_function(&cmd, &ctx);

    assert!(output.contains("export async function getUserById()"));
}

#[test]
fn test_default_camel_case_args_in_invoke() {
    // By default, Tauri expects camelCase keys in invoke
    let cmd = TauriCommand {
        name: "update".to_string(),
        args: vec![CommandArg {
            name: "user_id".to_string(),
            ty: RustType::Primitive("i32".to_string()),
        }],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = default_ctx();
    let output = generate_command_function(&cmd, &ctx);

    // Param should be camelCase
    assert!(output.contains("userId: number"));
    // Invoke should use camelCase key (shorthand)
    assert!(output.contains("{ userId }"));
    // Should NOT contain snake_case mapping
    assert!(!output.contains("user_id: userId"));
}

#[test]
fn test_snake_case_rename_all_in_invoke() {
    // With rename_all = "snake_case", Tauri expects snake_case keys
    let cmd = TauriCommand {
        name: "update".to_string(),
        args: vec![CommandArg {
            name: "user_id".to_string(),
            ty: RustType::Primitive("i32".to_string()),
        }],
        return_type: None,
        source_file: test_path(),
        rename_all: Some("snake_case".to_string()),
    };

    let ctx = default_ctx();
    let output = generate_command_function(&cmd, &ctx);

    // Param should still be camelCase
    assert!(output.contains("userId: number"));
    // But invoke should map camelCase param to snake_case key
    assert!(output.contains("user_id: userId"));
}

#[test]
fn test_collect_used_types_from_commands() {
    let commands = vec![
        TauriCommand {
            name: "get_user".to_string(),
            args: vec![],
            return_type: Some(RustType::custom("User")),
            source_file: test_path(),
            rename_all: None,
        },
        TauriCommand {
            name: "create".to_string(),
            args: vec![CommandArg {
                name: "req".to_string(),
                ty: RustType::custom("CreateRequest"),
            }],
            return_type: Some(RustType::custom("User")),
            source_file: test_path(),
            rename_all: None,
        },
    ];

    let mut ctx = default_ctx();
    ctx.register_type("User");
    ctx.register_type("CreateRequest");

    let types = collect_used_types(&commands, &ctx);

    assert!(types.contains("User"));
    assert!(types.contains("CreateRequest"));
    assert_eq!(types.len(), 2);
}

#[test]
fn test_collect_used_types_nested() {
    let commands = vec![TauriCommand {
        name: "get".to_string(),
        args: vec![],
        return_type: Some(RustType::Vec(Box::new(RustType::Option(Box::new(
            RustType::custom("User"),
        ))))),
        source_file: test_path(),
        rename_all: None,
    }];

    let ctx = ctx_with_type("User");
    let types = collect_used_types(&commands, &ctx);

    assert!(types.contains("User"));
}

#[test]
fn test_relative_import_path_same_dir() {
    let types_file = Path::new("src/generated/types.ts");
    let commands_file = Path::new("src/generated/commands.ts");

    let import = calculate_relative_import(types_file, commands_file);
    assert_eq!(import, "./types");
}

#[test]
fn test_naming_function_prefix() {
    let cmd = TauriCommand {
        name: "get_user".to_string(),
        args: vec![],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = GeneratorContext::new(NamingConfig {
        type_prefix: "".to_string(),
        type_suffix: "".to_string(),
        function_prefix: "api".to_string(),
        function_suffix: "".to_string(),
    });

    let output = generate_command_function(&cmd, &ctx);
    assert!(output.contains("export async function apigetUser"));
}

#[test]
fn test_naming_function_suffix() {
    let cmd = TauriCommand {
        name: "get_user".to_string(),
        args: vec![],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = GeneratorContext::new(NamingConfig {
        type_prefix: "".to_string(),
        type_suffix: "".to_string(),
        function_prefix: "".to_string(),
        function_suffix: "Cmd".to_string(),
    });

    let output = generate_command_function(&cmd, &ctx);
    assert!(output.contains("export async function getUserCmd"));
}

#[test]
fn test_generate_commands_file_header() {
    let commands: Vec<TauriCommand> = vec![];
    let types_path = Path::new("types.ts");
    let commands_path = Path::new("commands.ts");
    let ctx = default_ctx();

    let output = generate_commands_file(&commands, types_path, commands_path, &ctx);

    assert!(output.contains("// This file was auto-generated by tauri-ts-generator"));
    assert!(output.contains("// Do not edit this file manually"));
    assert!(output.contains("import { invoke } from \"@tauri-apps/api/core\""));
}

#[test]
fn test_generate_commands_file_with_imports() {
    let commands = vec![TauriCommand {
        name: "get_user".to_string(),
        args: vec![],
        return_type: Some(RustType::custom("User")),
        source_file: test_path(),
        rename_all: None,
    }];

    let types_path = Path::new("src/generated/types.ts");
    let commands_path = Path::new("src/generated/commands.ts");
    let ctx = ctx_with_type("User");

    let output = generate_commands_file(&commands, types_path, commands_path, &ctx);

    assert!(output.contains("import type { User } from"));
}

#[test]
fn test_complex_return_type() {
    let cmd = TauriCommand {
        name: "search".to_string(),
        args: vec![],
        return_type: Some(RustType::Result(Box::new(RustType::Vec(Box::new(
            RustType::custom("User"),
        ))))),
        source_file: test_path(),
        rename_all: None,
    };

    let ctx = ctx_with_type("User");
    let output = generate_command_function(&cmd, &ctx);

    assert!(output.contains("Promise<User[]>"));
}

#[test]
fn test_imports_are_sorted() {
    let commands = vec![TauriCommand {
        name: "test".to_string(),
        args: vec![
            CommandArg {
                name: "a".to_string(),
                ty: RustType::custom("BType"),
            },
            CommandArg {
                name: "b".to_string(),
                ty: RustType::custom("AType"),
            },
            CommandArg {
                name: "c".to_string(),
                ty: RustType::custom("CType"),
            },
        ],
        return_type: None,
        source_file: test_path(),
        rename_all: None,
    }];

    let types_path = Path::new("types.ts");
    let commands_path = Path::new("commands.ts");
    let mut ctx = default_ctx();
    ctx.register_type("AType");
    ctx.register_type("BType");
    ctx.register_type("CType");

    let output = generate_commands_file(&commands, types_path, commands_path, &ctx);

    // Should be AType, BType, CType
    assert!(output.contains("import type { AType, BType, CType }"));
}
