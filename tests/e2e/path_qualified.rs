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
fn same_type_reached_via_different_paths_is_not_a_conflict() {
    // Two commands reach the same `User` struct — one through a fully
    // qualified `crate::a::User`, the other through a plain `a::User`.
    // Both resolve to the same file. Pipeline should *not* raise a
    // conflict on the second pass (exercises the `register_resolution`
    // same-name-same-source return path).
    let project = Project::with_source(
        r#"
        pub mod a {
            use serde::{Deserialize, Serialize};
            #[derive(Serialize, Deserialize)]
            pub struct User { pub id: i32 }
        }

        #[tauri::command]
        fn first() -> Result<a::User, String> { todo!() }

        #[tauri::command]
        fn second() -> Result<crate::a::User, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let types = std::fs::read_to_string(&project.types_out).unwrap();
    assert_eq!(
        types.matches("export interface User").count(),
        1,
        "User must be declared exactly once:\n{types}"
    );

    let commands = std::fs::read_to_string(&project.commands_out).unwrap();
    assert!(commands.contains("Promise<User>"));
    // And the file itself mentions both commands referencing it.
    assert!(commands.contains("function first()"));
    assert!(commands.contains("function second()"));
}

#[test]
fn crate_aliased_path_still_resolves() {
    // `use crate as my_alias;` followed by `my_alias::types::User`.
    // Historically easy to miss in resolvers; our simple-name rendering
    // keeps it working as long as `User` is registered anywhere.
    let project = Project::with_source(
        r#"
        use crate as my_alias;
        pub mod types {
            use serde::{Deserialize, Serialize};
            #[derive(Serialize, Deserialize)]
            pub struct User { pub id: i32 }
        }

        #[tauri::command]
        fn fetch(u: my_alias::types::User) -> Result<my_alias::types::User, String> { todo!() }
        "#,
    );
    run_generate_ok(&project);
    let commands = std::fs::read_to_string(&project.commands_out).unwrap();
    assert!(commands.contains("u: User"), "arg type:\n{commands}");
    assert!(
        commands.contains("Promise<User>"),
        "return type:\n{commands}"
    );
    assert!(
        commands.contains("import type { User }"),
        "import:\n{commands}"
    );
    assert!(!commands.contains("my_alias"), "no alias leak:\n{commands}");
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
