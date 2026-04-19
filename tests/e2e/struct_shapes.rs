//! Tests for non-named struct shapes. Previously every struct rendered as
//! `interface Foo { ... }`, which silently disagreed with serde for:
//!
//!  * unit structs (`struct Foo;`)         → serde emits `null`
//!  * newtype  (`struct Foo(T)`)           → serde emits `T` transparently
//!  * tuple    (`struct Foo(T1, T2)`)      → serde emits `[T1, T2]`
//!  * `#[serde(transparent)]` on one-field → serde emits inner value

use crate::helpers::{run_generate_ok, Project};

fn types_for(source: &str) -> String {
    let project = Project::with_source(source);
    run_generate_ok(&project);
    std::fs::read_to_string(&project.types_out).unwrap()
}

#[test]
fn unit_struct_renders_as_null_type() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct Ping;
        #[tauri::command]
        fn x() -> Result<Ping, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("export type Ping = null;"),
        "unit struct must be typed as `null`, got:\n{types}"
    );
    assert!(
        !types.contains("export interface Ping"),
        "must NOT emit empty interface:\n{types}"
    );
}

#[test]
fn newtype_struct_is_transparent() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct UserId(pub i32);
        #[tauri::command]
        fn x() -> Result<UserId, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("export type UserId = number;"),
        "newtype must emit inner type, got:\n{types}"
    );
    assert!(!types.contains("field0:"), "no numbered wrapper:\n{types}");
}

#[test]
fn multi_field_tuple_struct_renders_as_array() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct Pair(pub i32, pub String);
        #[tauri::command]
        fn x() -> Result<Pair, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("export type Pair = [number, string];"),
        "tuple struct must emit JSON array type, got:\n{types}"
    );
}

#[test]
fn serde_transparent_named_struct_emits_inner() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct Name { pub inner: String }
        #[tauri::command]
        fn x() -> Result<Name, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("export type Name = string;"),
        "#[serde(transparent)] must emit inner type, got:\n{types}"
    );
    assert!(
        !types.contains("interface Name"),
        "no interface wrapper:\n{types}"
    );
}

#[test]
fn option_of_option_collapses_to_single_null() {
    // Serde collapses `Option<Option<T>>` on the wire (both Nones become
    // `null`), so the TypeScript must not stack `| null | null`.
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct Thing { pub maybe: Option<Option<i32>> }
        #[tauri::command]
        fn x() -> Result<Thing, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("maybe: number | null;"),
        "should be single null, got:\n{types}"
    );
    assert!(
        !types.contains("| null | null"),
        "redundant nulls:\n{types}"
    );
}

#[test]
fn serde_default_makes_field_optional() {
    // `#[serde(default)]` (or `default = "fn"`) allows the field to be
    // omitted from the JSON; the TS binding must reflect that with `?`.
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        fn default_retries() -> i32 { 3 }
        #[derive(Serialize, Deserialize)]
        pub struct Config {
            pub name: String,
            #[serde(default)]
            pub theme: Option<String>,
            #[serde(default = "default_retries")]
            pub retries: Option<i32>,
        }
        #[tauri::command]
        fn x() -> Result<Config, String> { todo!() }
        "#,
    );
    assert!(types.contains("name: string;"), "{types}");
    assert!(
        types.contains("theme?: string;"),
        "serde(default) must make theme optional, got:\n{types}"
    );
    assert!(
        types.contains("retries?: number;"),
        "serde(default = fn) must make retries optional, got:\n{types}"
    );
}

#[test]
fn skip_serializing_if_none_makes_option_optional() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};
        #[derive(Serialize, Deserialize)]
        pub struct Req {
            pub id: i32,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub note: Option<String>,
        }
        #[tauri::command]
        fn x() -> Result<Req, String> { todo!() }
        "#,
    );
    assert!(
        types.contains("note?: string;"),
        "skip_serializing_if = Option::is_none must emit optional:\n{types}"
    );
}

#[test]
fn newtype_wrapping_custom_type() {
    let types = types_for(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Inner { pub v: i32 }

        #[derive(Serialize, Deserialize)]
        pub struct Wrap(pub Inner);

        #[tauri::command]
        fn x() -> Result<Wrap, String> { todo!() }
        "#,
    );
    assert!(types.contains("export interface Inner"));
    assert!(
        types.contains("export type Wrap = Inner;"),
        "newtype over custom must unwrap to it, got:\n{types}"
    );
}
