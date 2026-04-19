//! Unit tests for the TypeScript generator. Extracted from the parent
//! file to keep implementation readable.

use super::*;
use crate::config::NamingConfig;
use crate::models::{EnumVariant, RustType, RustTypeAlias, StructField, StructShape, VariantData};
use std::path::PathBuf;

fn test_path() -> PathBuf {
    PathBuf::from("test.rs")
}

fn default_ctx() -> GeneratorContext {
    GeneratorContext::new(NamingConfig::default())
}

#[test]
fn test_generate_simple_interface() {
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "id".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            },
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "name".to_string(),
                ty: RustType::Primitive("String".to_string()),
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export interface User"));
    assert!(output.contains("id: number"));
    assert!(output.contains("name: string"));
}

#[test]
fn test_generate_interface_with_generics() {
    let s = RustStruct {
        name: "Wrapper".to_string(),
        generics: vec!["T".to_string()],
        fields: vec![
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "data".to_string(),
                ty: RustType::Generic("T".to_string()),
            },
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "count".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export interface Wrapper<T>"));
    assert!(output.contains("data: T"));
    assert!(output.contains("count: number"));
}

#[test]
fn test_generate_interface_with_multiple_generics() {
    let s = RustStruct {
        name: "Pair".to_string(),
        generics: vec!["K".to_string(), "V".to_string()],
        fields: vec![
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "key".to_string(),
                ty: RustType::Generic("K".to_string()),
            },
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "value".to_string(),
                ty: RustType::Generic("V".to_string()),
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export interface Pair<K, V>"));
}

#[test]
fn test_generate_simple_enum() {
    let e = RustEnum {
        name: "Status".to_string(),
        generics: vec![],
        variants: vec![
            EnumVariant {
                has_explicit_rename: false,
                name: "Active".to_string(),
                data: VariantData::Unit,
            },
            EnumVariant {
                has_explicit_rename: false,
                name: "Inactive".to_string(),
                data: VariantData::Unit,
            },
            EnumVariant {
                has_explicit_rename: false,
                name: "Pending".to_string(),
                data: VariantData::Unit,
            },
        ],
        source_file: test_path(),
        representation: EnumRepresentation::default(),
    };

    let ctx = default_ctx();
    let output = generate_enum_type(&e, &ctx);

    assert!(output.contains("export type Status ="));
    assert!(output.contains("\"Active\""));
    assert!(output.contains("\"Inactive\""));
    assert!(output.contains("\"Pending\""));
}

#[test]
fn test_generate_complex_enum_with_tuple() {
    let e = RustEnum {
        name: "Message".to_string(),
        generics: vec![],
        variants: vec![
            EnumVariant {
                has_explicit_rename: false,
                name: "Text".to_string(),
                data: VariantData::Tuple(vec![RustType::Primitive("String".to_string())]),
            },
            EnumVariant {
                has_explicit_rename: false,
                name: "Number".to_string(),
                data: VariantData::Tuple(vec![RustType::Primitive("i32".to_string())]),
            },
        ],
        source_file: test_path(),
        representation: EnumRepresentation::default(),
    };

    let ctx = default_ctx();
    let output = generate_enum_type(&e, &ctx);

    assert!(output.contains("export type Message ="));
    // External representation: { Text: string } | { Number: number }
    assert!(output.contains("Text: string"));
    assert!(output.contains("Number: number"));
}

#[test]
fn test_generate_complex_enum_with_struct() {
    let e = RustEnum {
        name: "UserRole".to_string(),
        generics: vec![],
        variants: vec![
            EnumVariant {
                has_explicit_rename: false,
                name: "Admin".to_string(),
                data: VariantData::Struct(vec![StructField {
                    has_explicit_rename: false,
                    use_optional: false,
                    is_flatten: false,
                    name: "permissions".to_string(),
                    ty: RustType::Vec(Box::new(RustType::Primitive("String".to_string()))),
                }]),
            },
            EnumVariant {
                has_explicit_rename: false,
                name: "User".to_string(),
                data: VariantData::Unit,
            },
        ],
        source_file: test_path(),
        representation: EnumRepresentation::Internal {
            tag: "type".to_string(),
        },
    };

    let ctx = default_ctx();
    let output = generate_enum_type(&e, &ctx);

    assert!(output.contains("type: \"Admin\""));
    assert!(output.contains("permissions: string[]"));
    assert!(output.contains("type: \"User\""));
}

#[test]
fn test_field_names_preserved_without_serde_attrs() {
    // Without serde attrs, field names should be preserved as-is (snake_case)
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "user_id".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            },
            StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "first_name".to_string(),
                ty: RustType::Primitive("String".to_string()),
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    // Without serde rename_all, fields keep their original snake_case names
    assert!(output.contains("user_id: number"));
    assert!(output.contains("first_name: string"));
}

#[test]
fn test_generate_empty_struct() {
    let s = RustStruct {
        name: "Empty".to_string(),
        generics: vec![],
        fields: vec![],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export interface Empty"));
    assert!(output.contains("{\n}\n"));
}

#[test]
fn test_generate_types_file_header() {
    let output = generate_types_file(&[], &[], &[], &default_ctx());

    assert!(output.contains("// This file was auto-generated by tauri-ts-generator"));
    assert!(output.contains("// Do not edit this file manually"));
}

#[test]
fn test_generate_type_alias() {
    let alias = RustTypeAlias {
        name: "UserAlias".to_string(),
        generics: vec![],
        target: RustType::custom("User"),
        source_file: test_path(),
    };

    let mut ctx = default_ctx();
    ctx.register_type("User");
    ctx.register_type("UserAlias");

    let output = generate_types_file(&[], &[], &[alias], &ctx);

    assert!(output.contains("export type UserAlias = User;"));
}

#[test]
fn test_generate_multiple_types() {
    let structs = vec![
        RustStruct {
            name: "User".to_string(),
            generics: vec![],
            fields: vec![StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "id".to_string(),
                ty: RustType::Primitive("i32".to_string()),
            }],
            shape: StructShape::Named,
            source_file: test_path(),
        },
        RustStruct {
            name: "Item".to_string(),
            generics: vec![],
            fields: vec![StructField {
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
                name: "name".to_string(),
                ty: RustType::Primitive("String".to_string()),
            }],
            shape: StructShape::Named,
            source_file: test_path(),
        },
    ];

    let enums = vec![RustEnum {
        name: "Status".to_string(),
        generics: vec![],
        variants: vec![EnumVariant {
            has_explicit_rename: false,
            name: "Active".to_string(),
            data: VariantData::Unit,
        }],
        source_file: test_path(),
        representation: EnumRepresentation::default(),
    }];

    let ctx = default_ctx();
    let output = generate_types_file(&structs, &enums, &[], &ctx);

    assert!(output.contains("export interface User"));
    assert!(output.contains("export interface Item"));
    assert!(output.contains("export type Status"));
}

#[test]
fn test_type_with_option_field() {
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![StructField {
            has_explicit_rename: false,
            use_optional: false,
            is_flatten: false,
            name: "email".to_string(),
            ty: RustType::Option(Box::new(RustType::Primitive("String".to_string()))),
        }],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("email: string | null"));
}

#[test]
fn test_type_with_vec_field() {
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![StructField {
            has_explicit_rename: false,
            use_optional: false,
            is_flatten: false,
            name: "tags".to_string(),
            ty: RustType::Vec(Box::new(RustType::Primitive("String".to_string()))),
        }],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("tags: string[]"));
}

#[test]
fn test_naming_prefix() {
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = GeneratorContext::new(NamingConfig {
        type_prefix: "I".to_string(),
        type_suffix: "".to_string(),
        function_prefix: "".to_string(),
        function_suffix: "".to_string(),
    });
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export interface IUser"));
}

#[test]
fn test_field_names_match_serde_serialization() {
    // Field names should match what serde will serialize:
    // - Without serde attrs: original name (snake_case)
    // - With explicit rename: the renamed value
    let s = RustStruct {
        name: "Config".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                name: "user_name".to_string(), // No serde attrs -> stays user_name
                ty: RustType::Primitive("String".to_string()),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
            },
            StructField {
                name: "API_KEY".to_string(), // serde(rename = "API_KEY") -> API_KEY
                ty: RustType::Primitive("String".to_string()),
                has_explicit_rename: true,
                use_optional: false,
                is_flatten: false,
            },
            StructField {
                name: "camelCaseField".to_string(), // serde(rename_all = "camelCase") -> camelCaseField
                ty: RustType::Primitive("bool".to_string()),
                has_explicit_rename: true,
                use_optional: false,
                is_flatten: false,
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    // Without serde attrs, field keeps original name
    assert!(output.contains("user_name: string"));
    assert!(!output.contains("userName: string"));
    // With explicit rename, uses the renamed value
    assert!(output.contains("API_KEY: string"));
    assert!(output.contains("camelCaseField: boolean"));
}

#[test]
fn test_enum_variant_explicit_rename_skips_camel_case() {
    // Enum with mixed renamed and normal variants
    let e = RustEnum {
        name: "Status".to_string(),
        generics: vec![],
        variants: vec![
            EnumVariant {
                name: "Active".to_string(),
                data: VariantData::Unit,
                has_explicit_rename: false,
            },
            EnumVariant {
                name: "INACTIVE_STATE".to_string(),
                data: VariantData::Unit,
                has_explicit_rename: true, // Explicitly renamed
            },
        ],
        source_file: test_path(),
        representation: EnumRepresentation::default(),
    };

    let ctx = default_ctx();
    let output = generate_enum_type(&e, &ctx);

    assert!(output.contains("\"Active\""));
    assert!(output.contains("\"INACTIVE_STATE\""));
}

#[test]
fn test_enum_struct_variant_field_names() {
    // Field names match serde behavior:
    // - Without explicit rename: original name preserved
    // - With explicit rename: uses the renamed value
    let e = RustEnum {
        name: "Event".to_string(),
        generics: vec![],
        variants: vec![EnumVariant {
            name: "Login".to_string(),
            data: VariantData::Struct(vec![
                StructField {
                    name: "user_id".to_string(),
                    ty: RustType::Primitive("i32".to_string()),
                    has_explicit_rename: false, // No serde rename -> keeps user_id
                    use_optional: false,
                    is_flatten: false,
                },
                StructField {
                    name: "TIMESTAMP".to_string(),
                    ty: RustType::Primitive("i64".to_string()),
                    has_explicit_rename: true, // serde(rename = "TIMESTAMP") -> TIMESTAMP
                    use_optional: false,
                    is_flatten: false,
                },
            ]),
            has_explicit_rename: false,
        }],
        source_file: test_path(),
        representation: EnumRepresentation::default(), // External tagging
    };

    let ctx = default_ctx();
    let output = generate_enum_type(&e, &ctx);

    assert!(output.contains("user_id: number")); // Preserved as-is
    assert!(output.contains("TIMESTAMP: number")); // Explicit rename preserved
}

#[test]
fn test_generate_ts_undefined_field() {
    let s = RustStruct {
        name: "Config".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                name: "volume".to_string(),
                ty: RustType::Option(Box::new(RustType::Primitive("f32".to_string()))),
                has_explicit_rename: false,
                use_optional: true,
                is_flatten: false,
            },
            StructField {
                name: "name".to_string(),
                ty: RustType::Option(Box::new(RustType::Primitive("String".to_string()))),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("volume?: number"));
    assert!(output.contains("name: string | null"));
}

#[test]
fn test_generate_interface_with_flatten() {
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                name: "name".to_string(),
                ty: RustType::Primitive("String".to_string()),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
            },
            StructField {
                name: "address".to_string(),
                ty: RustType::custom("Address"),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: true,
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let mut ctx = default_ctx();
    ctx.register_type("Address");
    let output = generate_interface(&s, &ctx);

    // Should generate type alias with intersection
    assert!(
        output.contains("export type User ="),
        "Should be a type alias, not interface"
    );
    assert!(output.contains("name: string"), "Should have normal field");
    assert!(
        output.contains("& Address"),
        "Should intersect with Address"
    );
    assert!(
        !output.contains("address:"),
        "Flatten field should not appear as property"
    );
}

#[test]
fn test_generate_interface_with_multiple_flatten() {
    let s = RustStruct {
        name: "User".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                name: "id".to_string(),
                ty: RustType::Primitive("i32".to_string()),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: false,
            },
            StructField {
                name: "address".to_string(),
                ty: RustType::custom("Address"),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: true,
            },
            StructField {
                name: "meta".to_string(),
                ty: RustType::custom("Metadata"),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: true,
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let mut ctx = default_ctx();
    ctx.register_type("Address");
    ctx.register_type("Metadata");
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export type User ="));
    assert!(output.contains("id: number"));
    assert!(output.contains("& Address"));
    assert!(output.contains("& Metadata"));
}

#[test]
fn test_generate_interface_only_flatten() {
    // Edge case: struct with only flatten fields
    let s = RustStruct {
        name: "Combined".to_string(),
        generics: vec![],
        fields: vec![
            StructField {
                name: "a".to_string(),
                ty: RustType::custom("TypeA"),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: true,
            },
            StructField {
                name: "b".to_string(),
                ty: RustType::custom("TypeB"),
                has_explicit_rename: false,
                use_optional: false,
                is_flatten: true,
            },
        ],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let mut ctx = default_ctx();
    ctx.register_type("TypeA");
    ctx.register_type("TypeB");
    let output = generate_interface(&s, &ctx);

    assert!(output.contains("export type Combined = TypeA & TypeB;"));
}

#[test]
fn test_generate_interface_without_flatten_remains_interface() {
    // Verify that structs without flatten still generate interface
    let s = RustStruct {
        name: "Simple".to_string(),
        generics: vec![],
        fields: vec![StructField {
            name: "id".to_string(),
            ty: RustType::Primitive("i32".to_string()),
            has_explicit_rename: false,
            use_optional: false,
            is_flatten: false,
        }],
        shape: StructShape::Named,
        source_file: test_path(),
    };

    let ctx = default_ctx();
    let output = generate_interface(&s, &ctx);

    assert!(
        output.contains("export interface Simple"),
        "Should be an interface"
    );
    assert!(
        !output.contains("export type Simple"),
        "Should not be a type alias"
    );
}
