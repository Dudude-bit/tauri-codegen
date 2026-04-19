//! Unit tests for the struct/enum parser. Extracted from the parent file
//! to keep implementation readable.

use super::*;
use crate::models::RustType;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    PathBuf::from("test.rs")
}

#[test]
fn test_parse_ts_optional_attribute() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Config {
            #[ts(optional)]
            pub volume: Option<f32>,
            pub name: Option<String>,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    let config = &structs[0];

    assert_eq!(config.fields[0].name, "volume");
    assert!(
        config.fields[0].use_optional,
        "Option field with #[ts(optional)] should have use_optional=true"
    );

    assert_eq!(config.fields[1].name, "name");
    assert!(
        !config.fields[1].use_optional,
        "Option field without attribute should have use_optional=false"
    );
}

#[test]
fn test_parse_ts_optional_ignored_on_non_option() {
    // This should print a warning but not fail
    let code = r#"
        #[derive(Serialize)]
        pub struct Config {
            #[ts(optional)]
            pub count: i32,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    let config = &structs[0];

    assert_eq!(config.fields[0].name, "count");
    assert!(
        !config.fields[0].use_optional,
        "Non-Option field should ignore ts(optional)"
    );
}

#[test]
fn test_parse_ts_optional_on_struct_variant() {
    let code = r#"
        #[derive(Serialize)]
        pub enum Settings {
            Network {
                #[ts(optional)]
                proxy: Option<String>,
                port: i32,
            }
        }
    "#;

    let ParsedTypes { enums, .. } = parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(enums.len(), 1);
    let settings = &enums[0];

    match &settings.variants[0].data {
        VariantData::Struct(fields) => {
            assert_eq!(fields[0].name, "proxy");
            assert!(fields[0].use_optional);
            assert_eq!(fields[1].name, "port");
            assert!(!fields[1].use_optional);
        }
        _ => panic!("Expected Struct variant"),
    }
}

#[test]
fn test_parse_simple_struct() {
    let code = r#"
        #[derive(Serialize)]
        pub struct User {
            pub id: i32,
            pub name: String,
        }
    "#;

    let ParsedTypes { structs, enums, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(enums.len(), 0);

    let user = &structs[0];
    assert_eq!(user.name, "User");
    assert_eq!(user.fields.len(), 2);
    assert_eq!(user.fields[0].name, "id");
    assert_eq!(user.fields[1].name, "name");
}

#[test]
fn test_parse_struct_with_generics() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Wrapper<T> {
            pub data: T,
            pub count: i32,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);

    let wrapper = &structs[0];
    assert_eq!(wrapper.name, "Wrapper");
    assert_eq!(wrapper.generics, vec!["T"]);
    assert_eq!(wrapper.fields.len(), 2);

    // data field should be Generic(T)
    match &wrapper.fields[0].ty {
        RustType::Generic(name) => assert_eq!(name, "T"),
        other => panic!("Expected Generic(T), got {:?}", other),
    }
}

#[test]
fn test_parse_struct_with_multiple_generics() {
    let code = r#"
        #[derive(Serialize, Deserialize)]
        pub struct Pair<K, V> {
            pub key: K,
            pub value: V,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);

    let pair = &structs[0];
    assert_eq!(pair.generics, vec!["K", "V"]);
}

#[test]
fn test_parse_tuple_struct() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Point(i32, i32);
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);

    let point = &structs[0];
    assert_eq!(point.name, "Point");
    assert_eq!(point.fields.len(), 2);
    // Tuple struct positions use bare indices (serde-compatible array keys),
    // and the shape is Tuple so the generator emits `[T1, T2]`.
    assert_eq!(point.fields[0].name, "0");
    assert_eq!(point.fields[1].name, "1");
    assert_eq!(point.shape, crate::models::StructShape::Tuple);
}

#[test]
fn test_parse_newtype_struct_has_newtype_shape() {
    let code = r#"
        #[derive(Serialize)]
        pub struct UserId(pub i32);
    "#;
    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs[0].shape, crate::models::StructShape::Newtype);
    assert_eq!(structs[0].fields.len(), 1);
}

#[test]
fn test_parse_unit_struct_has_unit_shape() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Marker;
    "#;
    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs[0].shape, crate::models::StructShape::Unit);
    assert!(structs[0].fields.is_empty());
}

#[test]
fn test_serde_transparent_forces_newtype_shape() {
    let code = r#"
        #[derive(Serialize)]
        #[serde(transparent)]
        pub struct Wrapped {
            pub inner: String,
        }
    "#;
    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs[0].shape, crate::models::StructShape::Newtype);
    assert_eq!(structs[0].fields.len(), 1);
}

#[test]
fn test_parse_simple_enum() {
    let code = r#"
        #[derive(Serialize)]
        pub enum Status {
            Active,
            Inactive,
            Pending,
        }
    "#;

    let ParsedTypes { structs, enums, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 0);
    assert_eq!(enums.len(), 1);

    let status = &enums[0];
    assert_eq!(status.name, "Status");
    assert_eq!(status.variants.len(), 3);
    assert_eq!(status.variants[0].name, "Active");
    assert_eq!(status.variants[1].name, "Inactive");
    assert_eq!(status.variants[2].name, "Pending");

    // All should be unit variants
    for variant in &status.variants {
        match &variant.data {
            VariantData::Unit => {}
            other => panic!("Expected Unit, got {:?}", other),
        }
    }
}

#[test]
fn test_parse_enum_with_tuple_data() {
    let code = r#"
        #[derive(Serialize)]
        pub enum Message {
            Text(String),
            Number(i32),
            Pair(String, i32),
        }
    "#;

    let ParsedTypes { enums, .. } = parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(enums.len(), 1);

    let message = &enums[0];
    assert_eq!(message.variants.len(), 3);

    match &message.variants[0].data {
        VariantData::Tuple(types) => {
            assert_eq!(types.len(), 1);
        }
        other => panic!("Expected Tuple, got {:?}", other),
    }

    match &message.variants[2].data {
        VariantData::Tuple(types) => {
            assert_eq!(types.len(), 2);
        }
        other => panic!("Expected Tuple with 2 elements, got {:?}", other),
    }
}

#[test]
fn test_parse_enum_with_struct_variant() {
    let code = r#"
        #[derive(Serialize)]
        pub enum UserRole {
            Admin { permissions: Vec<String> },
            User,
            Guest,
        }
    "#;

    let ParsedTypes { enums, .. } = parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(enums.len(), 1);

    let role = &enums[0];

    match &role.variants[0].data {
        VariantData::Struct(fields) => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "permissions");
        }
        other => panic!("Expected Struct variant, got {:?}", other),
    }

    match &role.variants[1].data {
        VariantData::Unit => {}
        other => panic!("Expected Unit, got {:?}", other),
    }
}

#[test]
fn test_serde_rename_field() {
    let code = r#"
        #[derive(Serialize)]
        pub struct User {
            #[serde(rename = "userId")]
            pub id: i32,
            pub name: String,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);

    let user = &structs[0];
    assert_eq!(user.fields[0].name, "userId");
    assert!(
        user.fields[0].has_explicit_rename,
        "Field with serde rename should have has_explicit_rename = true"
    );
    assert_eq!(user.fields[1].name, "name");
    assert!(
        !user.fields[1].has_explicit_rename,
        "Field without serde rename should have has_explicit_rename = false"
    );
}

#[test]
fn test_serde_rename_all_on_struct_fields() {
    let code = r#"
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct PortForwardRequest {
            pub local_port: u16,
            pub remote_port: u16,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);

    let request = &structs[0];
    assert_eq!(request.fields[0].name, "localPort");
    assert!(request.fields[0].has_explicit_rename);
    assert_eq!(request.fields[1].name, "remotePort");
    assert!(request.fields[1].has_explicit_rename);
}

#[test]
fn test_apply_rename_all_all_conventions_for_fields() {
    let field = "user_id";
    assert_eq!(
        apply_rename_all(field, &Some("lowercase".into())).unwrap(),
        "user_id"
    );
    assert_eq!(
        apply_rename_all(field, &Some("UPPERCASE".into())).unwrap(),
        "USER_ID"
    );
    assert_eq!(
        apply_rename_all(field, &Some("camelCase".into())).unwrap(),
        "userId"
    );
    assert_eq!(
        apply_rename_all(field, &Some("PascalCase".into())).unwrap(),
        "UserId"
    );
    assert_eq!(
        apply_rename_all(field, &Some("snake_case".into())).unwrap(),
        "user_id"
    );
    assert_eq!(
        apply_rename_all(field, &Some("SCREAMING_SNAKE_CASE".into())).unwrap(),
        "USER_ID"
    );
    assert_eq!(
        apply_rename_all(field, &Some("kebab-case".into())).unwrap(),
        "user-id"
    );
    assert_eq!(
        apply_rename_all(field, &Some("SCREAMING-KEBAB-CASE".into())).unwrap(),
        "USER-ID"
    );
}

#[test]
fn test_apply_rename_all_all_conventions_for_variants() {
    let variant = "GetUser";
    assert_eq!(
        apply_rename_all(variant, &Some("lowercase".into())).unwrap(),
        "getuser"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("UPPERCASE".into())).unwrap(),
        "GETUSER"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("camelCase".into())).unwrap(),
        "getUser"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("PascalCase".into())).unwrap(),
        "GetUser"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("snake_case".into())).unwrap(),
        "get_user"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("SCREAMING_SNAKE_CASE".into())).unwrap(),
        "GET_USER"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("kebab-case".into())).unwrap(),
        "get-user"
    );
    assert_eq!(
        apply_rename_all(variant, &Some("SCREAMING-KEBAB-CASE".into())).unwrap(),
        "GET-USER"
    );
}

#[test]
fn test_apply_rename_all_none_returns_none() {
    assert_eq!(apply_rename_all("user_id", &None), None);
}

#[test]
fn test_apply_rename_all_unknown_convention_falls_back_to_original() {
    assert_eq!(
        apply_rename_all("user_id", &Some("bogus".into())).unwrap(),
        "user_id"
    );
}

#[test]
fn test_serde_rename_all_pascal_case_on_struct_fields() {
    let code = r#"
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        pub struct User {
            pub user_id: i32,
            pub first_name: String,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);

    let user = &structs[0];
    assert_eq!(user.fields[0].name, "UserId");
    assert_eq!(user.fields[1].name, "FirstName");
}

#[test]
fn test_field_rename_overrides_container_rename_all() {
    let code = r#"
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Config {
            #[serde(rename = "API_KEY")]
            pub api_key: String,
            pub other_field: String,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    let cfg = &structs[0];
    assert_eq!(cfg.fields[0].name, "API_KEY");
    assert_eq!(cfg.fields[1].name, "otherField");
}

#[test]
fn test_serde_rename_variant() {
    let code = r#"
        #[derive(Serialize)]
        pub enum Status {
            #[serde(rename = "ACTIVE")]
            Active,
            #[serde(rename = "INACTIVE")]
            Inactive,
        }
    "#;

    let ParsedTypes { enums, .. } = parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(enums.len(), 1);

    let status = &enums[0];
    assert_eq!(status.variants[0].name, "ACTIVE");
    assert!(
        status.variants[0].has_explicit_rename,
        "Variant with serde rename should have has_explicit_rename = true"
    );
    assert_eq!(status.variants[1].name, "INACTIVE");
    assert!(
        status.variants[1].has_explicit_rename,
        "Variant with serde rename should have has_explicit_rename = true"
    );
}

#[test]
fn test_ignore_non_serializable() {
    let code = r#"
        pub struct NotExported {
            pub id: i32,
        }

        #[derive(Debug)]
        pub struct AlsoNotExported {
            pub name: String,
        }

        #[derive(Serialize)]
        pub struct Exported {
            pub data: String,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Exported");
}

#[test]
fn test_parse_types_in_mod() {
    let code = r#"
        mod types {
            #[derive(Serialize)]
            pub struct InnerType {
                pub value: i32,
            }

            #[derive(Deserialize)]
            pub enum InnerEnum {
                A,
                B,
            }
        }
    "#;

    let ParsedTypes { structs, enums, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(enums.len(), 1);
    assert_eq!(structs[0].name, "InnerType");
    assert_eq!(enums[0].name, "InnerEnum");
}

#[test]
fn test_deserialize_also_works() {
    let code = r#"
        #[derive(Deserialize)]
        pub struct Request {
            pub data: String,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Request");
}

#[test]
fn test_source_file_is_set() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Test {
            pub value: i32,
        }
    "#;

    let path = PathBuf::from("src/types.rs");
    let ParsedTypes { structs, .. } = parse_types(code, &path, ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].source_file, path);
}

#[test]
fn test_complex_field_types() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Complex {
            pub items: Vec<Item>,
            pub optional: Option<String>,
            pub map: HashMap<String, i32>,
            pub nested: Vec<Option<User>>,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].fields.len(), 4);

    match &structs[0].fields[0].ty {
        RustType::Vec(_) => {}
        other => panic!("Expected Vec, got {:?}", other),
    }

    match &structs[0].fields[1].ty {
        RustType::Option(_) => {}
        other => panic!("Expected Option, got {:?}", other),
    }

    match &structs[0].fields[2].ty {
        RustType::HashMap { .. } => {}
        other => panic!("Expected HashMap, got {:?}", other),
    }
}

#[test]
fn test_parse_expanded_code_with_serde_attrs() {
    let code = r#"
        pub mod types {
            pub struct AuthResponse {
                #[serde(rename = "accessToken")]
                pub access_token: ::std::string::String,
                #[serde(rename = "refreshToken")]
                pub refresh_token: ::std::string::String,
            }
        }
    "#;

    let ParsedTypes { structs, .. } =
        super::parse_types(code, &test_path(), ParseOptions::EXPANDED).unwrap();
    assert_eq!(structs.len(), 1, "Should find AuthResponse struct");
    assert_eq!(structs[0].name, "AuthResponse");
}

#[test]
fn test_parse_expanded_without_derive_but_with_serde_field_attrs() {
    // This simulates cargo expand output where derive is already expanded
    let code = r#"
        pub struct User {
            #[serde(rename = "userId")]
            pub user_id: i32,
            pub name: String,
        }
    "#;

    let ParsedTypes { structs, .. } =
        super::parse_types(code, &test_path(), ParseOptions::EXPANDED).unwrap();
    assert_eq!(
        structs.len(),
        1,
        "Should find User struct via serde field attrs"
    );
    assert_eq!(structs[0].name, "User");
}

#[test]
fn test_parse_types_regular_ignores_without_derive() {
    // Regular parse_types should NOT find structs without derive
    let code = r#"
        pub struct User {
            #[serde(rename = "userId")]
            pub user_id: i32,
        }
    "#;

    let ParsedTypes { structs, .. } =
        super::parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(
        structs.len(),
        0,
        "Regular parse should not find struct without derive"
    );
}

#[test]
fn test_parse_type_alias_with_generics() {
    let code = r#"
        pub type Wrapper<T> = Vec<T>;
    "#;

    let parsed = super::parse_types(code, &test_path(), ParseOptions::SOURCE_ALL).unwrap();
    assert_eq!(parsed.aliases.len(), 1);
    assert_eq!(parsed.aliases[0].name, "Wrapper");
    assert_eq!(parsed.aliases[0].generics, vec!["T"]);
}

#[test]
fn test_serde_skip_fields_are_excluded() {
    let code = r#"
        #[derive(Serialize)]
        pub struct User {
            pub id: i32,
            pub name: String,
            #[serde(skip)]
            pub internal_cache: Vec<u8>,
            #[serde(skip_serializing)]
            pub password_hash: String,
            #[serde(skip_deserializing)]
            pub computed_field: i32,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 1);
    // Only #[serde(skip)] should be excluded
    // skip_serializing and skip_deserializing are directional and should be kept
    assert_eq!(structs[0].fields.len(), 4);
    assert_eq!(structs[0].fields[0].name, "id");
    assert_eq!(structs[0].fields[1].name, "name");
    assert_eq!(structs[0].fields[2].name, "password_hash");
    assert_eq!(structs[0].fields[3].name, "computed_field");
}

#[test]
fn test_serde_skip_in_enum_variant_struct() {
    let code = r#"
        #[derive(Serialize)]
        pub enum Event {
            Login {
                user_id: i32,
                #[serde(skip)]
                internal_token: String,
            },
        }
    "#;

    let ParsedTypes { enums, .. } = parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(enums.len(), 1);

    match &enums[0].variants[0].data {
        crate::models::VariantData::Struct(fields) => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "user_id");
        }
        other => panic!("Expected Struct variant, got {:?}", other),
    }
}

#[test]
fn test_parse_serde_flatten() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Address {
            pub city: String,
            pub country: String,
        }

        #[derive(Serialize)]
        pub struct User {
            pub name: String,
            #[serde(flatten)]
            pub address: Address,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    assert_eq!(structs.len(), 2);

    let user = structs.iter().find(|s| s.name == "User").unwrap();
    assert_eq!(user.fields.len(), 2);

    assert_eq!(user.fields[0].name, "name");
    assert!(
        !user.fields[0].is_flatten,
        "Regular field should not be flattened"
    );

    assert_eq!(user.fields[1].name, "address");
    assert!(
        user.fields[1].is_flatten,
        "Field with #[serde(flatten)] should be flattened"
    );
}

#[test]
fn test_parse_multiple_serde_flatten() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Metadata {
            pub created_at: String,
        }

        #[derive(Serialize)]
        pub struct Address {
            pub city: String,
        }

        #[derive(Serialize)]
        pub struct User {
            pub name: String,
            #[serde(flatten)]
            pub address: Address,
            #[serde(flatten)]
            pub meta: Metadata,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    let user = structs.iter().find(|s| s.name == "User").unwrap();

    assert_eq!(user.fields.len(), 3);
    assert!(!user.fields[0].is_flatten); // name
    assert!(user.fields[1].is_flatten); // address
    assert!(user.fields[2].is_flatten); // meta
}

#[test]
fn test_parse_serde_flatten_with_other_attrs() {
    let code = r#"
        #[derive(Serialize)]
        pub struct Inner {
            pub value: i32,
        }

        #[derive(Serialize)]
        pub struct Outer {
            #[serde(rename = "customName")]
            pub name: String,
            #[serde(flatten)]
            pub inner: Inner,
        }
    "#;

    let ParsedTypes { structs, .. } =
        parse_types(code, &test_path(), ParseOptions::SOURCE).unwrap();
    let outer = structs.iter().find(|s| s.name == "Outer").unwrap();

    assert_eq!(outer.fields[0].name, "customName");
    assert!(outer.fields[0].has_explicit_rename);
    assert!(!outer.fields[0].is_flatten);

    assert!(outer.fields[1].is_flatten);
}
