//! Every `#[serde(rename_all = "...")]` variant applied to a struct, plus
//! `#[serde(rename)]` on individual fields.

use crate::helpers::{run_generate_ok, Project};

fn types_for(source: &str) -> String {
    let project = Project::with_source(source);
    run_generate_ok(&project);
    std::fs::read_to_string(&project.types_out).unwrap()
}

#[test]
fn no_attrs_preserves_original_field_names() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct Thing { pub user_id: i32, pub first_name: String }
        #[tauri::command]
        fn x() -> Result<Thing, String> { todo!() }
        "#,
    );
    // Plain snake_case must stay snake_case — it matches what serde emits.
    assert!(types.contains("user_id: number"), "{types}");
    assert!(types.contains("first_name: string"), "{types}");
    assert!(!types.contains("userId:"), "unexpected camelCase:\n{types}");
}

#[test]
fn camel_case() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Thing { pub user_id: i32, pub first_name: String }
        #[tauri::command]
        fn x() -> Result<Thing, String> { todo!() }
        "#,
    );
    assert!(types.contains("userId: number"), "{types}");
    assert!(types.contains("firstName: string"), "{types}");
}

#[test]
fn pascal_case() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        pub struct Thing { pub user_id: i32, pub first_name: String }
        #[tauri::command]
        fn x() -> Result<Thing, String> { todo!() }
        "#,
    );
    assert!(types.contains("UserId: number"), "{types}");
    assert!(types.contains("FirstName: string"), "{types}");
}

#[test]
fn snake_case_transform_on_camel_input() {
    // Starting from a camelCase field, snake_case should rewrite it.
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub struct Thing { pub userId: i32 }
        #[tauri::command]
        fn x() -> Result<Thing, String> { todo!() }
        "#,
    );
    assert!(types.contains("user_id: number"), "{types}");
}

#[test]
fn screaming_snake_and_kebab() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub struct Loud { pub user_id: i32 }

        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub struct Kebab { pub user_id: i32 }

        #[tauri::command]
        fn x() -> Result<(Loud, Kebab), String> { todo!() }
        "#,
    );
    assert!(
        types.contains("USER_ID: number"),
        "SCREAMING_SNAKE:\n{types}"
    );
    assert!(
        types.contains(r#""user-id": number"#) || types.contains("user-id: number"),
        "kebab:\n{types}"
    );
}

#[test]
fn field_rename_overrides_container_rename_all() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Config {
            #[serde(rename = "API_KEY")]
            pub api_key: String,
            pub other_field: String,
        }
        #[tauri::command]
        fn x() -> Result<Config, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("API_KEY: string"),
        "explicit rename:\n{types}"
    );
    assert!(
        types.contains("otherField: string"),
        "inherits camelCase:\n{types}"
    );
    assert!(
        !types.contains("apiKey:"),
        "rename must not fall through:\n{types}"
    );
}

#[test]
fn snake_case_handles_acronyms_correctly() {
    // Regression: `HTTPServer` must become `http_server`, not
    // `h_t_t_p_server`. Affects every variant name with an acronym.
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum Msg { HTTPServer, URLParser, XMLHttp, ParseJSON }
        #[tauri::command]
        fn x() -> Result<Msg, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(types.contains(r#""http_server""#), "HTTPServer:\n{types}");
    assert!(types.contains(r#""url_parser""#), "URLParser:\n{types}");
    assert!(types.contains(r#""xml_http""#), "XMLHttp:\n{types}");
    assert!(types.contains(r#""parse_json""#), "ParseJSON:\n{types}");
}

#[test]
fn enum_with_rename_all_transforms_variants() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum Status { Active, InProgress, NotFound }
        #[tauri::command]
        fn x() -> Result<Status, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(types.contains(r#""ACTIVE""#), "{types}");
    assert!(types.contains(r#""IN_PROGRESS""#), "{types}");
    assert!(types.contains(r#""NOT_FOUND""#), "{types}");
}
