use crate::models::{CommandArg, RustType, TauriCommand};
use anyhow::Result;
use std::path::Path;
use syn::{FnArg, ReturnType};

use super::type_extractor::parse_type;

/// Parse a Rust source file and extract Tauri commands.
///
/// Looks for `#[tauri::command]` (or `#[command]`) on free fns, impl
/// methods, and items inside inline `mod` blocks at any depth.
pub fn parse_commands(content: &str, source_file: &Path) -> Result<Vec<TauriCommand>> {
    let syntax = syn::parse_file(content)?;
    let mut commands = Vec::new();
    walk_for_commands(
        &syntax.items,
        &|_sig, attrs| attrs.iter().any(is_tauri_command_attr),
        source_file,
        &mut commands,
    );
    Ok(commands)
}

/// Parse Tauri commands from `cargo expand` output.
///
/// `#[tauri::command]` is a procedural macro, so by the time the source
/// has been through `cargo expand` the attribute is gone — the original
/// `fn name(...)` remains, and the macro has emitted a sibling
/// `pub use __cmd__name;` re-export. The re-export is what Tauri's
/// `generate_handler!` macro keys off, so it is the most reliable
/// post-expansion marker we have.
///
/// Strategy: walk the expanded module tree, collect every `__cmd__X`
/// name that appears in a `use` item, then walk again and accept any
/// `fn X(...)` whose name matches. This catches commands emitted by a
/// `macro_rules!` that stamps out N `#[tauri::command]` items —
/// [`parse_commands`] cannot see them because the attribute the macro
/// generated has already been consumed by expansion.
///
/// # Caveats
///
/// * **`__cmd__X` is a Tauri-internal naming convention.** A user who
///   manually defines `pub use __cmd__foo;` for unrelated reasons would
///   produce a phantom command. In practice this is a non-issue —
///   intended for the output of `cargo expand`, where the prefix only
///   appears as a Tauri-emitted marker.
/// * **`rename_all` is unrecoverable on this path.** Expansion
///   consumes the `#[tauri::command(rename_all = "...")]` attribute on
///   the `lib` side and leaves only a `pub use __cmd__X;` re-export;
///   the body that bakes the renaming in is generated *on the consumer
///   side* by `tauri::generate_handler!`, which lives in a different
///   crate (`main.rs`) and is not part of the expanded output we're
///   parsing. There is no recovery path short of cross-crate macro
///   expansion. Macro-generated `#[tauri::command]` items that depend
///   on `rename_all` will therefore be emitted with default casing;
///   write the command directly in source (so `parse_commands` picks
///   up the intact attribute) if that matters.
pub fn parse_expanded_commands(content: &str, source_file: &Path) -> Result<Vec<TauriCommand>> {
    let syntax = syn::parse_file(content)?;
    let mut command_names = std::collections::HashSet::new();
    collect_expanded_command_names(&syntax.items, &mut command_names);

    let mut commands = Vec::new();
    walk_for_commands(
        &syntax.items,
        &|sig, _attrs| command_names.contains(&sig.ident.to_string()),
        source_file,
        &mut commands,
    );
    Ok(commands)
}

/// Single walker shared by [`parse_commands`] and
/// [`parse_expanded_commands`]. Recursively descends into inline
/// modules and impl blocks at any depth, applying `is_command` to each
/// function signature + attribute list to decide whether to materialise
/// a `TauriCommand`.
///
/// The two call sites differ only in the predicate they supply
/// (attribute check vs name-set check), so the walker stays generic
/// over `Fn(&Signature, &[Attribute]) -> bool`.
fn walk_for_commands<F>(
    items: &[syn::Item],
    is_command: &F,
    source_file: &Path,
    out: &mut Vec<TauriCommand>,
) where
    F: Fn(&syn::Signature, &[syn::Attribute]) -> bool,
{
    for item in items {
        match item {
            syn::Item::Fn(func) if is_command(&func.sig, &func.attrs) => {
                out.push(parse_command_from_signature(
                    &func.sig,
                    &func.attrs,
                    source_file,
                ));
            }
            syn::Item::Impl(impl_block) => {
                for impl_item in &impl_block.items {
                    if let syn::ImplItem::Fn(method) = impl_item {
                        if is_command(&method.sig, &method.attrs) {
                            out.push(parse_command_from_signature(
                                &method.sig,
                                &method.attrs,
                                source_file,
                            ));
                        }
                    }
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    walk_for_commands(inner, is_command, source_file, out);
                }
            }
            _ => {}
        }
    }
}

/// Recursively gather every `X` from `pub use __cmd__X;` (and bare
/// `use __cmd__X;`) anywhere in the tree, descending into inline
/// modules. Tauri's expansion of `#[tauri::command]` emits exactly one
/// such re-export per command, so the set of names this returns is the
/// authoritative list of commands defined under `items`.
fn collect_expanded_command_names(
    items: &[syn::Item],
    names: &mut std::collections::HashSet<String>,
) {
    for item in items {
        match item {
            syn::Item::Use(use_item) => {
                walk_use_tree(&use_item.tree, names);
            }
            syn::Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    collect_expanded_command_names(inner, names);
                }
            }
            _ => {}
        }
    }
}

fn walk_use_tree(tree: &syn::UseTree, names: &mut std::collections::HashSet<String>) {
    match tree {
        syn::UseTree::Path(path) => walk_use_tree(&path.tree, names),
        syn::UseTree::Group(group) => {
            for inner in &group.items {
                walk_use_tree(inner, names);
            }
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if let Some(stripped) = ident.strip_prefix("__cmd__") {
                names.insert(stripped.to_string());
            }
        }
        syn::UseTree::Rename(rename) => {
            let ident = rename.ident.to_string();
            if let Some(stripped) = ident.strip_prefix("__cmd__") {
                names.insert(stripped.to_string());
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

/// Check if an attribute is #[tauri::command] or #[command] (with or without arguments)
fn is_tauri_command_attr(attr: &syn::Attribute) -> bool {
    let path = match &attr.meta {
        syn::Meta::Path(path) => path,
        syn::Meta::List(list) => &list.path,
        _ => return false,
    };

    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    // Check for #[tauri::command] or #[command]
    (segments.len() == 2 && segments[0] == "tauri" && segments[1] == "command")
        || (segments.len() == 1 && segments[0] == "command")
}

/// Extract rename_all value from #[tauri::command(rename_all = "...")]
fn extract_rename_all(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !is_tauri_command_attr(attr) {
            continue;
        }

        if let syn::Meta::List(list) = &attr.meta {
            // Parse the tokens inside the parentheses
            let tokens = list.tokens.clone();
            if let Ok(nested) = syn::parse2::<syn::ExprAssign>(tokens.clone()) {
                // Check for rename_all = "value"
                if let syn::Expr::Path(path) = nested.left.as_ref() {
                    if path.path.is_ident("rename_all") {
                        if let syn::Expr::Lit(lit) = nested.right.as_ref() {
                            if let syn::Lit::Str(lit_str) = &lit.lit {
                                return Some(lit_str.value());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Parse a function signature into a TauriCommand
fn parse_command_from_signature(
    sig: &syn::Signature,
    attrs: &[syn::Attribute],
    source_file: &Path,
) -> TauriCommand {
    let name = sig.ident.to_string();
    let args = sig.inputs.iter().filter_map(parse_fn_arg).collect();
    let return_type = parse_return_type(&sig.output);
    let rename_all = extract_rename_all(attrs);

    TauriCommand {
        name,
        args,
        return_type,
        source_file: source_file.to_path_buf(),
        rename_all,
    }
}

/// Parse a function argument
fn parse_fn_arg(arg: &FnArg) -> Option<CommandArg> {
    match arg {
        FnArg::Typed(pat_type) => {
            // Extract argument name from pattern
            let name = match pat_type.pat.as_ref() {
                syn::Pat::Ident(ident) => ident.ident.to_string(),
                _ => return None,
            };

            let ty = parse_type(&pat_type.ty);

            Some(CommandArg { name, ty })
        }
        FnArg::Receiver(_) => None, // Skip self arguments
    }
}

/// Parse the return type of a function
fn parse_return_type(return_type: &ReturnType) -> Option<RustType> {
    match return_type {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            let rust_type = parse_type(ty);
            match rust_type {
                RustType::Unit => None,
                _ => Some(rust_type),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.rs")
    }

    #[test]
    fn test_parse_simple_command() {
        let code = r#"
            #[tauri::command]
            fn greet() {
                println!("Hello!");
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "greet");
        assert!(commands[0].args.is_empty());
        assert!(commands[0].return_type.is_none());
    }

    #[test]
    fn test_parse_command_with_short_attribute() {
        let code = r#"
            #[command]
            fn greet() {}
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "greet");
    }

    #[test]
    fn test_parse_command_with_args() {
        let code = r#"
            #[tauri::command]
            fn get_user(id: i32, name: String) -> User {
                unimplemented!()
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "get_user");
        assert_eq!(commands[0].args.len(), 2);
        assert_eq!(commands[0].args[0].name, "id");
        assert_eq!(commands[0].args[1].name, "name");
    }

    #[test]
    fn test_parse_command_with_return_type() {
        let code = r#"
            #[tauri::command]
            fn get_user(id: i32) -> Result<User, String> {
                unimplemented!()
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert!(commands[0].return_type.is_some());

        match &commands[0].return_type {
            Some(RustType::Result(inner)) => match inner.as_ref() {
                RustType::Custom { name, .. } => assert_eq!(name, "User"),
                other => panic!("Expected Custom, got {:?}", other),
            },
            other => panic!("Expected Result, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_async_command() {
        let code = r#"
            #[tauri::command]
            async fn fetch_data() -> Result<Vec<Item>, String> {
                unimplemented!()
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "fetch_data");
        assert!(commands[0].return_type.is_some());
    }

    #[test]
    fn test_parse_command_in_mod() {
        let code = r#"
            mod commands {
                #[tauri::command]
                fn inner_command(id: i32) -> String {
                    unimplemented!()
                }
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "inner_command");
    }

    #[test]
    fn test_parse_multiple_commands() {
        let code = r#"
            #[tauri::command]
            fn command_one() {}

            #[tauri::command]
            fn command_two(id: i32) -> String {
                unimplemented!()
            }

            #[tauri::command]
            async fn command_three() -> Result<(), String> {
                Ok(())
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].name, "command_one");
        assert_eq!(commands[1].name, "command_two");
        assert_eq!(commands[2].name, "command_three");
    }

    #[test]
    fn test_ignore_non_command_functions() {
        let code = r#"
            fn helper_function() {}

            pub fn another_helper(x: i32) -> i32 {
                x * 2
            }

            #[tauri::command]
            fn actual_command() {}
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "actual_command");
    }

    #[test]
    fn test_command_with_complex_types() {
        let code = r#"
            #[tauri::command]
            fn complex(
                items: Vec<Item>,
                optional: Option<String>,
                map: HashMap<String, i32>
            ) -> Result<Vec<Response>, String> {
                unimplemented!()
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].args.len(), 3);

        // Check items arg
        match &commands[0].args[0].ty {
            RustType::Vec(_) => {}
            other => panic!("Expected Vec, got {:?}", other),
        }

        // Check optional arg
        match &commands[0].args[1].ty {
            RustType::Option(_) => {}
            other => panic!("Expected Option, got {:?}", other),
        }

        // Check map arg
        match &commands[0].args[2].ty {
            RustType::HashMap { .. } => {}
            other => panic!("Expected HashMap, got {:?}", other),
        }
    }

    #[test]
    fn test_void_return_is_none() {
        let code = r#"
            #[tauri::command]
            fn void_command() {
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert!(commands[0].return_type.is_none());
    }

    #[test]
    fn test_unit_return_is_none() {
        let code = r#"
            #[tauri::command]
            fn unit_command() -> () {
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert!(commands[0].return_type.is_none());
    }

    #[test]
    fn test_source_file_is_set() {
        let code = r#"
            #[tauri::command]
            fn my_command() {}
        "#;

        let path = PathBuf::from("src/commands.rs");
        let commands = parse_commands(code, &path).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].source_file, path);
    }

    #[test]
    fn test_parse_rename_all_snake_case() {
        let code = r#"
            #[tauri::command(rename_all = "snake_case")]
            fn my_command(user_id: i32) {}
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].rename_all, Some("snake_case".to_string()));
    }

    #[test]
    fn test_parse_rename_all_camel_case() {
        let code = r#"
            #[tauri::command(rename_all = "camelCase")]
            fn my_command(user_id: i32) {}
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].rename_all, Some("camelCase".to_string()));
    }

    #[test]
    fn test_parse_no_rename_all() {
        let code = r#"
            #[tauri::command]
            fn my_command(user_id: i32) {}
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].rename_all, None);
    }

    #[test]
    fn test_parse_command_short_form_with_rename_all() {
        let code = r#"
            #[command(rename_all = "snake_case")]
            fn my_command(user_id: i32) {}
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].rename_all, Some("snake_case".to_string()));
    }

    // ---- Recursion / depth coverage for `parse_commands` -----------------
    //
    // Pre-2.0.3 the walker descended only one level into `Item::Mod` and
    // looked at `Item::Fn` only inside it — `Item::Impl` blocks and any
    // deeper mod nesting were silently dropped. These tests pin the
    // post-refactor contract: discovery is recursive in both directions.

    #[test]
    fn parse_commands_finds_command_in_nested_mod() {
        let code = r#"
            mod outer {
                mod inner {
                    #[tauri::command]
                    fn deep_command(id: i32) -> String {
                        unimplemented!()
                    }
                }
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1, "nested-mod command must be found");
        assert_eq!(commands[0].name, "deep_command");
    }

    #[test]
    fn parse_commands_finds_command_in_impl_inside_mod() {
        let code = r#"
            mod commands {
                impl Service {
                    #[tauri::command]
                    fn from_impl(id: i32) -> i32 { id }
                }
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1, "impl-inside-mod command must be found");
        assert_eq!(commands[0].name, "from_impl");
    }

    #[test]
    fn parse_commands_finds_command_in_nested_impl_inside_mod() {
        // Combination case — both wrappings at once.
        let code = r#"
            mod outer {
                mod inner {
                    impl Service {
                        #[tauri::command]
                        fn deeply_buried(x: bool) -> bool { x }
                    }
                }
            }
        "#;

        let commands = parse_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "deeply_buried");
    }

    // ---- `parse_expanded_commands` --------------------------------------
    //
    // Post-cargo-expand the `#[tauri::command]` attribute is gone. The
    // only surviving marker is the proc-macro's `pub use __cmd__name;`
    // re-export. These tests pin that the expanded-path parser keys off
    // the marker and not the attribute, and that the walker shares the
    // same recursion contract as `parse_commands`.

    #[test]
    fn parse_expanded_commands_picks_up_bare_fn_with_cmd_marker() {
        let code = r#"
            pub async fn subscribe_pod_watch(namespace: String) -> Result<String, String> {
                Ok(namespace)
            }
            #[allow(unused_imports)]
            pub use __cmd__subscribe_pod_watch;
        "#;

        let commands = parse_expanded_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "subscribe_pod_watch");
        assert_eq!(commands[0].args.len(), 1);
        assert_eq!(commands[0].args[0].name, "namespace");
    }

    #[test]
    fn parse_expanded_commands_handles_grouped_reexport() {
        // Real cargo-expand output groups multiple re-exports:
        //   pub use {__cmd__foo, __tauri_command_name_foo};
        let code = r#"
            pub async fn camel_case_cmd(my_arg: i32) -> i32 { my_arg }
            #[allow(unused_imports)]
            pub use {__cmd__camel_case_cmd, __tauri_command_name_camel_case_cmd};
        "#;

        let commands = parse_expanded_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "camel_case_cmd");
    }

    #[test]
    fn parse_expanded_commands_ignores_unrelated_uses() {
        // Plain `pub use` statements without the `__cmd__` prefix must
        // not be mistaken for commands. The function below has a
        // `pub use other_name;` sibling but no `__cmd__` marker — it is
        // not a Tauri command.
        let code = r#"
            pub fn not_a_command(x: i32) -> i32 { x }
            pub use other_name;
        "#;

        let commands = parse_expanded_commands(code, &test_path()).unwrap();
        assert!(commands.is_empty(), "got {:?}", commands);
    }

    #[test]
    fn parse_expanded_commands_descends_into_nested_mods() {
        // Marker can be at any depth; the walker must find the fn at
        // the same depth as the marker.
        let code = r#"
            mod commands {
                mod watch {
                    pub fn subscribe_node_watch() -> Result<String, String> {
                        unimplemented!()
                    }
                    #[allow(unused_imports)]
                    pub use __cmd__subscribe_node_watch;
                }
            }
        "#;

        let commands = parse_expanded_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "subscribe_node_watch");
    }

    #[test]
    fn parse_expanded_commands_finds_method_in_impl() {
        let code = r#"
            impl Service {
                pub fn list_pods(namespace: String) -> Result<String, String> {
                    Ok(namespace)
                }
            }
            #[allow(unused_imports)]
            pub use __cmd__list_pods;
        "#;

        let commands = parse_expanded_commands(code, &test_path()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "list_pods");
    }
}
