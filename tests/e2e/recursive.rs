//! Self-referential and mutually-recursive types used to loop forever in
//! the fixpoint walker without dedup. These tests lock the guard in.

use crate::helpers::{run_generate_ok, Project};

#[test]
fn self_referential_linked_list() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Node {
            pub value: i32,
            pub next: Option<Box<Node>>,
        }

        #[tauri::command]
        fn head() -> Result<Node, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    // Node must appear exactly once.
    assert_eq!(
        types.matches("export interface Node").count(),
        1,
        "Node should be emitted exactly once:\n{types}"
    );
    assert!(types.contains("next: Node | null"));
}

#[test]
fn mutually_recursive_structs() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct A { pub b: Option<Box<B>> }

        #[derive(Serialize, Deserialize)]
        pub struct B { pub a: Option<Box<A>> }

        #[tauri::command]
        fn root() -> Result<A, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert_eq!(types.matches("export interface A").count(), 1);
    assert_eq!(types.matches("export interface B").count(), 1);
}
