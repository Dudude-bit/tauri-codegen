//! Snapshot-based e2e tests. Unlike `basic::simple_struct_and_command`
//! which locks down one byte-exact golden, these capture the full
//! generated output for a variety of projects and diff against stored
//! snapshots. Refresh with `cargo insta review` after intentional changes.

use crate::helpers::{run_generate_ok, Project};

/// Grab both output files from a project that was already run.
fn outputs(project: &Project) -> (String, String) {
    (
        std::fs::read_to_string(&project.types_out).unwrap(),
        std::fs::read_to_string(&project.commands_out).unwrap(),
    )
}

#[test]
fn snapshot_user_crud() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct User { pub id: i32, pub name: String, pub email: Option<String> }

        #[derive(Serialize, Deserialize)]
        pub struct CreateUserRequest { pub name: String, pub email: Option<String> }

        #[derive(Serialize, Deserialize)]
        pub enum Status { Active, Inactive, Pending }

        #[tauri::command]
        fn greet(name: String) -> String { todo!() }

        #[tauri::command]
        fn get_user(id: i32) -> Result<User, String> { todo!() }

        #[tauri::command]
        fn create_user(request: CreateUserRequest) -> Result<User, String> { todo!() }

        #[tauri::command]
        async fn get_all_users() -> Result<Vec<User>, String> { todo!() }

        #[tauri::command]
        fn delete_user(id: i32) -> Result<(), String> { todo!() }

        #[tauri::command]
        fn get_status(id: i32) -> Result<Status, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let (types, commands) = outputs(&project);
    insta::assert_snapshot!("user_crud__types", types);
    insta::assert_snapshot!("user_crud__commands", commands);
}

#[test]
fn snapshot_rename_all_matrix() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct PlainThing { pub user_id: i32, pub first_name: String }

        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct CamelThing { pub user_id: i32, pub first_name: String }

        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        pub struct PascalThing { pub user_id: i32, pub first_name: String }

        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum Loud { Active, InProgress, NotFound }

        #[tauri::command]
        fn dispatch() -> Result<(PlainThing, CamelThing, PascalThing, Loud), String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let (types, _) = outputs(&project);
    insta::assert_snapshot!("rename_all_matrix__types", types);
}

#[test]
fn snapshot_serde_flatten_and_skip() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Address { pub city: String, pub country: String }

        #[derive(Serialize, Deserialize)]
        pub struct User {
            pub name: String,
            #[serde(flatten)]
            pub address: Address,
            #[serde(skip)]
            pub internal: String,
        }

        #[tauri::command]
        fn get_user() -> Result<User, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let (types, _) = outputs(&project);
    insta::assert_snapshot!("flatten_and_skip__types", types);
}

#[test]
fn snapshot_tagged_enum_variants() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        #[serde(tag = "kind", content = "data")]
        pub enum Event {
            Click { x: i32, y: i32 },
            KeyPress(String),
            Idle,
        }

        #[derive(Serialize, Deserialize)]
        pub struct Msg { pub body: String }

        #[derive(Serialize, Deserialize)]
        #[serde(untagged)]
        pub enum Envelope {
            Text(String),
            Wrapped(Msg),
        }

        #[tauri::command]
        fn handle() -> Result<(Event, Envelope), String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let (types, _) = outputs(&project);
    insta::assert_snapshot!("tagged_enum_variants__types", types);
}

#[test]
fn snapshot_smart_pointers_and_recursion() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};
        use std::sync::Arc;

        #[derive(Serialize, Deserialize)]
        pub struct Node {
            pub value: i32,
            pub next: Option<Box<Node>>,
            pub shared: Arc<String>,
        }

        #[tauri::command]
        fn head() -> Result<Node, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let (types, commands) = outputs(&project);
    insta::assert_snapshot!("smart_pointers_and_recursion__types", types);
    insta::assert_snapshot!("smart_pointers_and_recursion__commands", commands);
}
