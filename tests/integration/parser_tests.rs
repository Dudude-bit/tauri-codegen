//! Integration tests for parsing Rust source files

use std::path::PathBuf;
use tauri_ts_generator::parser::{parse_commands, parse_types};
use tauri_ts_generator::models::RustType;

/// Get path to test fixtures
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Read fixture file content
fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).expect("Failed to read fixture")
}

#[test]
fn test_parse_simple_commands_fixture() {
    let content = read_fixture("simple_commands.rs");
    let path = fixture_path("simple_commands.rs");

    let commands = parse_commands(&content, &path).expect("Failed to parse commands");

    assert_eq!(commands.len(), 5);

    // Check greet command
    let greet = commands.iter().find(|c| c.name == "greet").unwrap();
    assert_eq!(greet.args.len(), 1);
    assert_eq!(greet.args[0].name, "name");
    match &greet.return_type {
        Some(RustType::Primitive(name)) => assert_eq!(name, "String"),
        _ => panic!("Expected String return type"),
    }

    // Check get_user command
    let get_user = commands.iter().find(|c| c.name == "get_user").unwrap();
    assert_eq!(get_user.args.len(), 1);
    match &get_user.return_type {
        Some(RustType::Result(inner)) => match inner.as_ref() {
            RustType::Custom(name) => assert_eq!(name, "User"),
            _ => panic!("Expected Custom type inside Result"),
        },
        _ => panic!("Expected Result return type"),
    }

    // Check create_user command
    let create_user = commands.iter().find(|c| c.name == "create_user").unwrap();
    assert_eq!(create_user.args.len(), 1);
    match &create_user.args[0].ty {
        RustType::Custom(name) => assert_eq!(name, "CreateUserRequest"),
        _ => panic!("Expected CreateUserRequest type"),
    }

    // Check get_all_users command
    let get_all = commands.iter().find(|c| c.name == "get_all_users").unwrap();
    assert!(get_all.args.is_empty());
    match &get_all.return_type {
        Some(RustType::Vec(inner)) => match inner.as_ref() {
            RustType::Custom(name) => assert_eq!(name, "User"),
            _ => panic!("Expected Custom type inside Vec"),
        },
        _ => panic!("Expected Vec return type"),
    }

    // Check delete_user command
    let delete = commands.iter().find(|c| c.name == "delete_user").unwrap();
    assert_eq!(delete.args.len(), 1);
}

#[test]
fn test_parse_simple_types_fixture() {
    let content = read_fixture("simple_commands.rs");
    let path = fixture_path("simple_commands.rs");

    let (structs, enums) = parse_types(&content, &path).expect("Failed to parse types");

    assert_eq!(structs.len(), 2);
    assert_eq!(enums.len(), 1);

    // Check User struct
    let user = structs.iter().find(|s| s.name == "User").unwrap();
    assert_eq!(user.fields.len(), 3);
    assert!(user.fields.iter().any(|f| f.name == "id"));
    assert!(user.fields.iter().any(|f| f.name == "name"));
    assert!(user.fields.iter().any(|f| f.name == "email"));

    // Check CreateUserRequest struct
    let request = structs.iter().find(|s| s.name == "CreateUserRequest").unwrap();
    assert_eq!(request.fields.len(), 2);

    // Check Status enum
    let status = enums.iter().find(|e| e.name == "Status").unwrap();
    assert_eq!(status.variants.len(), 3);
    assert!(status.variants.iter().any(|v| v.name == "Active"));
    assert!(status.variants.iter().any(|v| v.name == "Inactive"));
    assert!(status.variants.iter().any(|v| v.name == "Pending"));
}

#[test]
fn test_parse_complex_types_fixture() {
    let content = read_fixture("complex_types.rs");
    let path = fixture_path("complex_types.rs");

    let (structs, enums) = parse_types(&content, &path).expect("Failed to parse types");

    // Check Wrapper<T>
    let wrapper = structs.iter().find(|s| s.name == "Wrapper").unwrap();
    assert_eq!(wrapper.generics, vec!["T"]);
    assert_eq!(wrapper.fields.len(), 2);

    // Check Pair<K, V>
    let pair = structs.iter().find(|s| s.name == "Pair").unwrap();
    assert_eq!(pair.generics, vec!["K", "V"]);

    // Check PaginatedResponse<T>
    let paginated = structs.iter().find(|s| s.name == "PaginatedResponse").unwrap();
    assert_eq!(paginated.generics, vec!["T"]);

    // Check Message enum with complex variants
    let message = enums.iter().find(|e| e.name == "Message").unwrap();
    assert_eq!(message.variants.len(), 4);

    // Check TupleStruct
    let tuple_struct = structs.iter().find(|s| s.name == "TupleStruct").unwrap();
    assert_eq!(tuple_struct.fields.len(), 2);
    assert_eq!(tuple_struct.fields[0].name, "field0");
    assert_eq!(tuple_struct.fields[1].name, "field1");
}

#[test]
fn test_parse_nested_modules_fixture() {
    let content = read_fixture("nested_modules.rs");
    let path = fixture_path("nested_modules.rs");

    let commands = parse_commands(&content, &path).expect("Failed to parse commands");
    let (structs, enums) = parse_types(&content, &path).expect("Failed to parse types");

    // Should find commands in nested modules
    assert!(commands.iter().any(|c| c.name == "get_inner"));
    assert!(commands.iter().any(|c| c.name == "get_outer"));

    // Should find types in nested modules
    assert!(structs.iter().any(|s| s.name == "InnerType"));
    assert!(structs.iter().any(|s| s.name == "OuterType"));
    assert!(enums.iter().any(|e| e.name == "InnerEnum"));
}

