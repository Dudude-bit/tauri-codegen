//! Integration tests for ModuleResolver. Extracted from resolver.rs to
//! keep that file focused on implementation.

use super::*;
fn base_path() -> PathBuf {
    PathBuf::from("src")
}

#[test]
fn test_resolve_local_type() {
    let mut resolver = ModuleResolver::new();
    let code = "struct User;";
    let path = PathBuf::from("src/types.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    match resolver.resolve_type("User", &path) {
        ResolutionResult::Found(p) => assert_eq!(p, path),
        _ => panic!("Failed to resolve"),
    }
}

#[test]
fn test_resolve_super() {
    let mut resolver = ModuleResolver::new();

    // Parent
    let parent_code = "struct User;";
    let parent_path = PathBuf::from("src/mod.rs");
    resolver
        .parse_file(&parent_path, parent_code, &base_path())
        .unwrap();

    // Child
    let child_code = "";
    let child_path = PathBuf::from("src/sub/mod.rs");
    resolver
        .parse_file(&child_path, child_code, &base_path())
        .unwrap();

    match resolver.resolve_type("super::User", &child_path) {
        ResolutionResult::Found(p) => assert_eq!(p, parent_path),
        res => panic!("Failed to resolve super::User: {:?}", res),
    }
}

#[test]
fn test_resolve_path_via_import() {
    let mut resolver = ModuleResolver::new();

    // Define type in a module: src/types.rs -> User
    let types_code = "struct User;";
    let types_path = PathBuf::from("src/types.rs");
    resolver
        .parse_file(&types_path, types_code, &base_path())
        .unwrap();

    // Usage file: imports module, uses qualified path
    // use crate::types;
    // ... types::User
    let cmd_code = "use crate::types;";
    let cmd_path = PathBuf::from("src/cmd.rs");
    resolver
        .parse_file(&cmd_path, cmd_code, &base_path())
        .unwrap();

    match resolver.resolve_type("types::User", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        res => panic!("Failed to resolve types::User via import: {:?}", res),
    }
}

#[test]
fn test_resolve_ambiguous() {
    let mut resolver = ModuleResolver::new();

    let path_a = PathBuf::from("src/a.rs");
    resolver
        .parse_file(&path_a, "struct User;", &base_path())
        .unwrap();

    let path_b = PathBuf::from("src/b.rs");
    resolver
        .parse_file(&path_b, "struct User;", &base_path())
        .unwrap();

    let path_cmd = PathBuf::from("src/cmd.rs");
    resolver.parse_file(&path_cmd, "", &base_path()).unwrap();

    match resolver.resolve_type("User", &path_cmd) {
        ResolutionResult::Ambiguous(paths) => {
            assert_eq!(paths.len(), 2);
            assert!(paths.contains(&path_a));
            assert!(paths.contains(&path_b));
        }
        res => panic!("Expected Ambiguous, got {:?}", res),
    }
}

#[test]
fn test_resolve_path_via_renamed_import() {
    let mut resolver = ModuleResolver::new();

    let types_path = PathBuf::from("src/long_name/types.rs");
    resolver
        .parse_file(&types_path, "struct User;", &base_path())
        .unwrap();

    // use crate::long_name::types as t;
    // t::User
    let cmd_code = "use crate::long_name::types as t;";
    let cmd_path = PathBuf::from("src/cmd.rs");
    resolver
        .parse_file(&cmd_path, cmd_code, &base_path())
        .unwrap();

    match resolver.resolve_type("t::User", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        res => panic!("Failed to resolve t::User via renamed import: {:?}", res),
    }
}

#[test]
fn test_resolve_deeply_nested_path() {
    let mut resolver = ModuleResolver::new();

    let target_path = PathBuf::from("src/a/b/c/target.rs");
    resolver
        .parse_file(&target_path, "struct Deep;", &base_path())
        .unwrap();

    let cmd_path = PathBuf::from("src/main.rs");
    resolver.parse_file(&cmd_path, "", &base_path()).unwrap();

    match resolver.resolve_type("crate::a::b::c::target::Deep", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, target_path),
        res => panic!("Failed to resolve deep path: {:?}", res),
    }
}

#[test]
fn test_resolve_super_chain() {
    let mut resolver = ModuleResolver::new();

    let root_path = PathBuf::from("src/types.rs");
    resolver
        .parse_file(&root_path, "struct Top;", &base_path())
        .unwrap();

    let deep_path = PathBuf::from("src/a/b/c/deep.rs");
    resolver.parse_file(&deep_path, "", &base_path()).unwrap();

    // deep.rs is at crate::a::b::c::deep
    // super -> c
    // super -> b
    // super -> a
    // super -> crate
    // super::super::super::super::types::Top
    match resolver.resolve_type("super::super::super::super::types::Top", &deep_path) {
        ResolutionResult::Found(p) => assert_eq!(p, root_path),
        res => panic!("Failed to resolve super chain: {:?}", res),
    }
}

#[test]
fn test_resolve_sibling_via_super() {
    let mut resolver = ModuleResolver::new();

    // src/sibling.rs -> crate::sibling
    let sibling_path = PathBuf::from("src/sibling.rs");
    resolver
        .parse_file(&sibling_path, "struct SiblingType;", &base_path())
        .unwrap();

    // src/current.rs -> crate::current
    let current_path = PathBuf::from("src/current.rs");
    resolver
        .parse_file(&current_path, "", &base_path())
        .unwrap();

    // siblings must be accessed via parent (super) if not imported
    match resolver.resolve_type("super::sibling::SiblingType", &current_path) {
        ResolutionResult::Found(p) => assert_eq!(p, sibling_path),
        res => panic!("Failed to resolve sibling path via super: {:?}", res),
    }
}
#[test]
fn test_resolve_reexport() {
    let mut resolver = ModuleResolver::new();

    // src/types.rs -> struct User
    let types_path = PathBuf::from("src/types.rs");
    resolver
        .parse_file(&types_path, "pub struct User;", &base_path())
        .unwrap();

    // src/lib.rs -> pub mod types; pub use types::User;
    let lib_path = PathBuf::from("src/lib.rs");
    let lib_code = "pub mod types; pub use types::User;";
    resolver
        .parse_file(&lib_path, lib_code, &base_path())
        .unwrap();

    // Verify lib_path was parsed correctly
    assert!(resolver.files.contains_key(&lib_path));

    // src/cmd.rs -> use crate::User;
    let main_path = PathBuf::from("src/cmd.rs");
    let main_code = "use crate::User;";
    resolver
        .parse_file(&main_path, main_code, &base_path())
        .unwrap();

    // Should resolve crate::User to src/types.rs
    match resolver.resolve_type("crate::User", &main_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        res => panic!("Failed to resolve re-export: {:?}", res),
    }
}

#[test]
fn test_resolve_type_via_wildcard_reexport() {
    let mut resolver = ModuleResolver::new();

    // src/resources/types.rs -> struct PodInfo
    let types_path = PathBuf::from("src/resources/types.rs");
    resolver
        .parse_file(&types_path, "pub struct PodInfo;", &base_path())
        .unwrap();

    // src/resources/mod.rs -> pub use types::*;
    let mod_path = PathBuf::from("src/resources/mod.rs");
    let mod_code = "pub use types::*;";
    resolver
        .parse_file(&mod_path, mod_code, &base_path())
        .unwrap();

    // src/commands.rs -> use crate::resources::PodInfo;
    let cmd_path = PathBuf::from("src/commands.rs");
    let cmd_code = "use crate::resources::PodInfo;";
    resolver
        .parse_file(&cmd_path, cmd_code, &base_path())
        .unwrap();

    // Should resolve crate::resources::PodInfo via wildcard re-export to src/resources/types.rs
    match resolver.resolve_type("crate::resources::PodInfo", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        res => panic!("Failed to resolve via wildcard re-export: {:?}", res),
    }
}

#[test]
fn test_resolve_simple_name_via_wildcard() {
    let mut resolver = ModuleResolver::new();

    // src/types.rs -> struct User
    let types_path = PathBuf::from("src/types.rs");
    resolver
        .parse_file(&types_path, "pub struct User;", &base_path())
        .unwrap();

    // src/main.rs -> use types::*;
    let main_path = PathBuf::from("src/main.rs");
    let main_code = "use types::*;";
    resolver
        .parse_file(&main_path, main_code, &base_path())
        .unwrap();

    // Should resolve User via wildcard import
    match resolver.resolve_type("User", &main_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        res => panic!("Failed to resolve simple name via wildcard: {:?}", res),
    }
}

#[test]
fn test_normalize_relative_path_simple() {
    let resolver = ModuleResolver::new();

    // ["types"] with context ["crate", "resources"] -> ["crate", "resources", "types"]
    let relative = vec!["types".to_string()];
    let from_module = vec!["crate".to_string(), "resources".to_string()];
    let result = resolver.normalize_relative_path(&relative, &from_module);

    assert_eq!(result, vec!["crate", "resources", "types"]);
}

#[test]
fn test_normalize_relative_path_with_super() {
    let resolver = ModuleResolver::new();

    // ["super", "other"] with context ["crate", "foo", "bar"] -> ["crate", "foo", "other"]
    let relative = vec!["super".to_string(), "other".to_string()];
    let from_module = vec!["crate".to_string(), "foo".to_string(), "bar".to_string()];
    let result = resolver.normalize_relative_path(&relative, &from_module);

    assert_eq!(result, vec!["crate", "foo", "other"]);
}

#[test]
fn test_normalize_absolute_path() {
    let resolver = ModuleResolver::new();

    // ["crate", "types"] is already absolute, should not change
    let absolute = vec!["crate".to_string(), "types".to_string()];
    let from_module = vec!["crate".to_string(), "other".to_string()];
    let result = resolver.normalize_relative_path(&absolute, &from_module);

    assert_eq!(result, vec!["crate", "types"]);
}

#[test]
fn test_resolve_through_multiple_wildcards() {
    let mut resolver = ModuleResolver::new();

    // src/inner/types.rs -> struct DeepType
    let deep_types_path = PathBuf::from("src/inner/types.rs");
    resolver
        .parse_file(&deep_types_path, "pub struct DeepType;", &base_path())
        .unwrap();

    // src/inner/mod.rs -> pub use types::*;
    let inner_mod_path = PathBuf::from("src/inner/mod.rs");
    resolver
        .parse_file(&inner_mod_path, "pub use types::*;", &base_path())
        .unwrap();

    // src/lib.rs -> pub use inner::*;
    let lib_path = PathBuf::from("src/lib.rs");
    resolver
        .parse_file(&lib_path, "pub use inner::*;", &base_path())
        .unwrap();

    // Should resolve crate::inner::DeepType via wildcard in inner/mod.rs
    let cmd_path = PathBuf::from("src/cmd.rs");
    resolver.parse_file(&cmd_path, "", &base_path()).unwrap();

    match resolver.resolve_type("crate::inner::DeepType", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, deep_types_path),
        res => panic!("Failed to resolve through nested wildcard: {:?}", res),
    }
}

#[test]
fn test_resolve_mixed_explicit_and_wildcard() {
    let mut resolver = ModuleResolver::new();

    // src/a.rs -> struct TypeA
    let a_path = PathBuf::from("src/a.rs");
    resolver
        .parse_file(&a_path, "pub struct TypeA;", &base_path())
        .unwrap();

    // src/b.rs -> struct TypeB
    let b_path = PathBuf::from("src/b.rs");
    resolver
        .parse_file(&b_path, "pub struct TypeB;", &base_path())
        .unwrap();

    // src/lib.rs -> pub use a::TypeA; pub use b::*;
    let lib_path = PathBuf::from("src/lib.rs");
    let lib_code = "pub use a::TypeA; pub use b::*;";
    resolver
        .parse_file(&lib_path, lib_code, &base_path())
        .unwrap();

    let cmd_path = PathBuf::from("src/cmd.rs");
    resolver.parse_file(&cmd_path, "", &base_path()).unwrap();

    // Resolve explicit re-export
    match resolver.resolve_type("crate::TypeA", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, a_path),
        res => panic!("Failed to resolve explicit re-export: {:?}", res),
    }

    // Resolve wildcard re-export
    match resolver.resolve_type("crate::TypeB", &cmd_path) {
        ResolutionResult::Found(p) => assert_eq!(p, b_path),
        res => panic!("Failed to resolve wildcard re-export: {:?}", res),
    }
}

#[test]
fn test_are_siblings_empty_path() {
    // Empty paths should not panic and return false
    assert!(!are_siblings(&[], &[]));
    assert!(!are_siblings(&["crate".to_string()], &[]));
    assert!(!are_siblings(&[], &["crate".to_string()]));
}

#[test]
fn test_are_siblings_single_element() {
    // Single element paths can't have siblings (no parent)
    assert!(!are_siblings(
        &["crate".to_string()],
        &["crate".to_string()]
    ));
}

#[test]
fn test_are_siblings_valid() {
    assert!(are_siblings(
        &["crate".to_string(), "foo".to_string()],
        &["crate".to_string(), "bar".to_string()]
    ));
    assert!(!are_siblings(
        &["crate".to_string(), "foo".to_string()],
        &["crate".to_string(), "other".to_string(), "bar".to_string()]
    ));
}

#[test]
fn test_parse_nested_module() {
    let mut resolver = ModuleResolver::new();

    let code = r#"
        mod inner {
            pub struct NestedType;
        }
        pub struct OuterType;
    "#;
    let path = PathBuf::from("src/lib.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // Both types should be registered
    assert!(resolver.type_definitions.contains_key("NestedType"));
    assert!(resolver.type_definitions.contains_key("OuterType"));
}

#[test]
fn test_parse_type_alias() {
    let mut resolver = ModuleResolver::new();

    let code = r#"
        pub struct RealType;
        pub type AliasType = RealType;
    "#;
    let path = PathBuf::from("src/types.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // Both should be registered
    assert!(resolver.type_definitions.contains_key("RealType"));
    assert!(resolver.type_definitions.contains_key("AliasType"));

    // Resolve alias type
    match resolver.resolve_type("AliasType", &path) {
        ResolutionResult::Found(p) => assert_eq!(p, path),
        res => panic!("Failed to resolve type alias: {:?}", res),
    }
}

#[test]
fn test_no_duplicate_registration() {
    let mut resolver = ModuleResolver::new();

    let code = "pub struct User;";
    let path = PathBuf::from("src/types.rs");

    // Parse the same file twice
    resolver.parse_file(&path, code, &base_path()).unwrap();
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // Should only have one entry for the path
    let locations = resolver.type_definitions.get("User").unwrap();
    assert_eq!(locations.len(), 1);
}

#[test]
fn test_resolve_alias_target_simple() {
    let mut resolver = ModuleResolver::new();

    let code = r#"
        pub struct RealType;
        pub type AliasType = RealType;
    "#;
    let path = PathBuf::from("src/types.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // resolve_alias_target should return the base type name
    let target = resolver.resolve_alias_target("AliasType", &path);
    assert_eq!(target, Some("RealType".to_string()));

    // Non-alias types should return None
    let no_target = resolver.resolve_alias_target("RealType", &path);
    assert_eq!(no_target, None);
}

#[test]
fn test_resolve_alias_target_tauri_state() {
    let mut resolver = ModuleResolver::new();

    // This simulates: type AppStateMutexed<'a> = State<'a, Mutex<AppState>>;
    let code = r#"
        pub type AppStateMutexed<'a> = State<'a, Mutex<AppState>>;
    "#;
    let path = PathBuf::from("src/commands.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // resolve_alias_target should return "State"
    let target = resolver.resolve_alias_target("AppStateMutexed", &path);
    assert_eq!(target, Some("State".to_string()));
}

#[test]
fn test_resolve_alias_target_generic_type() {
    let mut resolver = ModuleResolver::new();

    let code = r#"
        pub type MyWindow = Window;
        pub type MyHandle<T> = AppHandle<T>;
    "#;
    let path = PathBuf::from("src/types.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // Both should resolve to their base types
    assert_eq!(
        resolver.resolve_alias_target("MyWindow", &path),
        Some("Window".to_string())
    );
    assert_eq!(
        resolver.resolve_alias_target("MyHandle", &path),
        Some("AppHandle".to_string())
    );
}

#[test]
fn test_type_alias_stored_in_scope() {
    let mut resolver = ModuleResolver::new();

    let code = r#"
        pub type CustomState<'a> = State<'a, MyAppState>;
    "#;
    let path = PathBuf::from("src/lib.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // Check that the alias is stored in the scope
    let scope = resolver.files.get(&path).unwrap();
    assert_eq!(
        scope.type_aliases.get("CustomState"),
        Some(&"State".to_string())
    );
}

#[test]
fn test_resolve_alias_chain() {
    let mut resolver = ModuleResolver::new();

    // Create a chain: AliasedState -> MyState -> State
    let code = r#"
        pub type MyState<'a> = State<'a, AppState>;
        pub type AliasedState<'a> = MyState<'a>;
    "#;
    let path = PathBuf::from("src/types.rs");
    resolver.parse_file(&path, code, &base_path()).unwrap();

    // resolve_alias_target should follow the chain to "State"
    let target = resolver.resolve_alias_target("AliasedState", &path);
    assert_eq!(target, Some("State".to_string()));

    // MyState should also resolve to State
    let target2 = resolver.resolve_alias_target("MyState", &path);
    assert_eq!(target2, Some("State".to_string()));
}

#[test]
fn test_cross_file_type_not_ambiguous() {
    // This test verifies the fix for the bug where types defined in one file
    // and used in another would incorrectly trigger "ambiguous type" warnings
    // when cargo-expand was also registering the same types.
    //
    // The fix ensures that types are only registered from their actual source
    // files, not from cargo-expand output.
    let mut resolver = ModuleResolver::new();

    // types.rs defines DeploymentContainerInfo
    let types_code = r#"
        pub struct DeploymentContainerInfo {
            pub name: String,
        }
    "#;
    let types_path = PathBuf::from("src/resources/types.rs");
    resolver
        .parse_file(&types_path, types_code, &base_path())
        .unwrap();

    // workloads.rs uses DeploymentContainerInfo via import
    let workloads_code = r#"
        use super::types::DeploymentContainerInfo;

        pub struct StatefulSetDetailInfo {
            pub containers: Vec<DeploymentContainerInfo>,
        }
    "#;
    let workloads_path = PathBuf::from("src/resources/workloads.rs");
    resolver
        .parse_file(&workloads_path, workloads_code, &base_path())
        .unwrap();

    // DeploymentContainerInfo should only be registered once (from types.rs)
    let locations = resolver
        .type_definitions
        .get("DeploymentContainerInfo")
        .unwrap();
    assert_eq!(
        locations.len(),
        1,
        "Type should only be registered once, not duplicated"
    );
    assert_eq!(locations[0], types_path);

    // Resolving the type from workloads.rs should find it (not be ambiguous)
    match resolver.resolve_type("DeploymentContainerInfo", &workloads_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        ResolutionResult::Ambiguous(paths) => {
            panic!("Type should NOT be ambiguous! Found in: {:?}", paths);
        }
        res => panic!("Expected Found, got {:?}", res),
    }
}

#[test]
fn test_type_defined_in_multiple_files_is_ambiguous() {
    // This test verifies that types ACTUALLY defined in multiple files
    // are correctly detected as ambiguous (this is the expected behavior).
    let mut resolver = ModuleResolver::new();

    // Two different files both define a type with the same name
    let file_a = PathBuf::from("src/a.rs");
    resolver
        .parse_file(&file_a, "pub struct SharedName;", &base_path())
        .unwrap();

    let file_b = PathBuf::from("src/b.rs");
    resolver
        .parse_file(&file_b, "pub struct SharedName;", &base_path())
        .unwrap();

    // This SHOULD be ambiguous because the type is defined in two places
    let locations = resolver.type_definitions.get("SharedName").unwrap();
    assert_eq!(
        locations.len(),
        2,
        "Type defined in 2 files should have 2 locations"
    );

    let query_path = PathBuf::from("src/main.rs");
    resolver.parse_file(&query_path, "", &base_path()).unwrap();

    match resolver.resolve_type("SharedName", &query_path) {
        ResolutionResult::Ambiguous(paths) => {
            assert_eq!(paths.len(), 2);
            assert!(paths.contains(&file_a));
            assert!(paths.contains(&file_b));
        }
        res => panic!("Expected Ambiguous, got {:?}", res),
    }
}

#[test]
fn test_cargo_expand_types_not_registered_separately() {
    // This test verifies the fix for the cargo-expand ambiguity bug.
    //
    // BEFORE THE FIX:
    // When cargo-expand was used, types were registered unconditionally,
    // even if they already existed in source files. This caused duplicates.
    //
    // AFTER THE FIX:
    // register_expanded_type_if_missing() only registers types that DON'T
    // already exist in source files. Types from source files take priority.

    let mut resolver = ModuleResolver::new();

    // Parse the actual source file FIRST - this registers the type
    let types_path = PathBuf::from("src/resources/types.rs");
    let types_code = r#"
        pub struct DeploymentContainerInfo {
            pub name: String,
        }
    "#;
    resolver
        .parse_file(&types_path, types_code, &base_path())
        .unwrap();

    // Verify type is registered exactly once from the source file
    let locations = resolver
        .type_definitions
        .get("DeploymentContainerInfo")
        .unwrap();
    assert_eq!(locations.len(), 1, "Type should be registered only once");
    assert_eq!(
        locations[0], types_path,
        "Type should be registered from source file"
    );

    // Now try to register the same type from cargo-expand - should be ignored
    let expanded_path = PathBuf::from("<cargo-expand>");
    resolver.register_expanded_type_if_missing("DeploymentContainerInfo", &expanded_path);

    // Type should STILL be registered only once (cargo-expand was ignored)
    let locations = resolver
        .type_definitions
        .get("DeploymentContainerInfo")
        .unwrap();
    assert_eq!(
        locations.len(),
        1,
        "Cargo-expand should not duplicate source file types"
    );
    assert_eq!(
        locations[0], types_path,
        "Source file registration should be preserved"
    );

    // Simulate another file that USES (not defines) the type
    let workloads_path = PathBuf::from("src/resources/workloads.rs");
    let workloads_code = r#"
        use super::types::DeploymentContainerInfo;

        pub struct StatefulSetDetailInfo {
            pub containers: Vec<DeploymentContainerInfo>,
        }
    "#;
    resolver
        .parse_file(&workloads_path, workloads_code, &base_path())
        .unwrap();

    // Type should STILL be registered only once (imports don't register types)
    let locations = resolver
        .type_definitions
        .get("DeploymentContainerInfo")
        .unwrap();
    assert_eq!(
        locations.len(),
        1,
        "Importing a type should not register it again"
    );

    // Resolution should find the type without ambiguity
    match resolver.resolve_type("DeploymentContainerInfo", &workloads_path) {
        ResolutionResult::Found(p) => assert_eq!(p, types_path),
        ResolutionResult::Ambiguous(paths) => {
            panic!(
                "BUG: Type should NOT be ambiguous! This was the original bug. Found in: {:?}",
                paths
            );
        }
        res => panic!("Expected Found, got {:?}", res),
    }
}

#[test]
fn test_macro_generated_types_can_be_resolved() {
    // This test verifies that types that ONLY exist in cargo-expand output
    // (macro-generated types not present in source files) can still be resolved.
    //
    // Example: progenitor generates API client structs from OpenAPI specs.
    // These structs don't exist in source code, only in expanded output.

    let mut resolver = ModuleResolver::new();

    // Parse a source file that does NOT contain MacroGeneratedType
    let source_path = PathBuf::from("src/commands.rs");
    let source_code = r#"
        pub struct NormalType {
            pub value: i32,
        }
    "#;
    resolver
        .parse_file(&source_path, source_code, &base_path())
        .unwrap();

    // MacroGeneratedType doesn't exist in source - verify it's not registered
    assert!(!resolver.type_definitions.contains_key("MacroGeneratedType"));

    // Register a macro-generated type from cargo-expand
    // This simulates what pipeline.rs does after parsing source files
    let expanded_path = PathBuf::from("<cargo-expand>");
    resolver.register_expanded_type_if_missing("MacroGeneratedType", &expanded_path);

    // Now the macro-generated type should be registered
    let locations = resolver.type_definitions.get("MacroGeneratedType").unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0], expanded_path);

    // Resolution should find the macro-generated type
    match resolver.resolve_type("MacroGeneratedType", &source_path) {
        ResolutionResult::Found(p) => {
            assert_eq!(p, expanded_path, "Should resolve to <cargo-expand>");
        }
        res => panic!("Expected Found for macro-generated type, got {:?}", res),
    }
}
