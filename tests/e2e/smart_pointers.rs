//! Smart-pointer wrappers (`Box`, `Arc`, `Rc`, `Cow`) must serialize
//! transparently — the generator should unwrap them to the inner type.

use crate::helpers::{run_generate_ok, Project};

#[test]
fn box_arc_rc_unwrap_to_inner() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};
        use std::sync::Arc;
        use std::rc::Rc;

        #[derive(Serialize, Deserialize)]
        pub struct Wrappers {
            pub boxed: Box<String>,
            pub arced: Arc<i32>,
            pub rc: Rc<bool>,
        }

        #[tauri::command]
        fn x() -> Result<Wrappers, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(types.contains("boxed: string"), "Box<String>:\n{types}");
    assert!(types.contains("arced: number"), "Arc<i32>:\n{types}");
    assert!(types.contains("rc: boolean"), "Rc<bool>:\n{types}");
}

#[test]
fn cow_skips_lifetime_argument() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};
        use std::borrow::Cow;

        #[derive(Serialize, Deserialize)]
        pub struct Leaf<'a> {
            pub label: Cow<'a, str>,
        }

        #[tauri::command]
        fn x() -> Result<Leaf<'static>, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(types.contains("label: string"), "Cow<'_, str>:\n{types}");
}

#[test]
fn vec_of_box_custom_type_is_preserved() {
    let project = Project::with_source(
        r#"
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        pub struct Item { pub n: i32 }

        #[derive(Serialize, Deserialize)]
        pub struct Bag { pub items: Vec<Box<Item>> }

        #[tauri::command]
        fn x() -> Result<Bag, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert!(
        types.contains("items: Item[]"),
        "Vec<Box<T>> should flatten to T[]:\n{types}"
    );
    assert!(types.contains("export interface Item"));
}
