//! Rust allows a command signature to reference a type through a fully
//! qualified path (`crate::types::User`, `super::models::Order`) or a
//! partial one (`types::User`). Before this fix the generator leaked the
//! path characters straight into TypeScript — `crate::types::User` is not
//! a valid TS type — and the `import type { … }` block came up empty
//! because the context is keyed by simple names.

use crate::helpers::{run_generate_ok, Project};

#[test]
fn crate_qualified_path_in_command_signature() {
    let project = Project::with_source(
        r#"
        pub mod types {
            use serde::{Deserialize, Serialize};
            #[derive(Serialize, Deserialize)]
            pub struct User { pub id: i32 }
            #[derive(Serialize, Deserialize)]
            pub struct Order { pub n: i32 }
        }

        #[tauri::command]
        fn by_full_path(u: crate::types::User) -> Result<crate::types::Order, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();

    assert!(
        commands.contains("u: User"),
        "`crate::types::` must not leak into arg type:\n{commands}"
    );
    assert!(
        commands.contains("Promise<Order>"),
        "return type must be simple name:\n{commands}"
    );
    assert!(
        commands.contains("invoke<Order>(\"by_full_path\""),
        "invoke<> must use simple name:\n{commands}"
    );
    assert!(
        commands.contains("import type { Order, User } from \"./types\""),
        "types must be imported — registry is keyed by simple name:\n{commands}"
    );
    assert!(
        !commands.contains("crate::") && !commands.contains("types::"),
        "no Rust path separators allowed in TS:\n{commands}"
    );
}

#[test]
fn partial_module_path_in_command_signature() {
    let project = Project::with_source(
        r#"
        pub mod types {
            use serde::{Deserialize, Serialize};
            #[derive(Serialize, Deserialize)]
            pub struct User { pub id: i32 }
        }

        #[tauri::command]
        fn by_partial(u: types::User) -> Result<types::User, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();
    assert!(commands.contains("u: User"), "{commands}");
    assert!(commands.contains("Promise<User>"), "{commands}");
    assert!(
        commands.contains("import type { User }"),
        "User must be imported:\n{commands}"
    );
}

#[test]
fn qualified_path_with_generic_arguments() {
    // Combines path qualification with a generic type — the stripping must
    // happen on *both* the outer and the inner name.
    let project = Project::with_source(
        r#"
        pub mod types {
            use serde::{Deserialize, Serialize};
            #[derive(Serialize, Deserialize)]
            pub struct Page<T> { pub items: Vec<T> }
            #[derive(Serialize, Deserialize)]
            pub struct User { pub id: i32 }
        }

        #[tauri::command]
        fn list() -> Result<crate::types::Page<crate::types::User>, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();
    assert!(
        commands.contains("Promise<Page<User>>"),
        "nested generic with qualified paths must resolve:\n{commands}"
    );
    assert!(
        commands.contains("import type { Page, User }"),
        "both base and arg types must be imported:\n{commands}"
    );
}
