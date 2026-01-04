use crate::models::{CommandArg, RustType, TauriCommand};
use anyhow::Result;
use std::path::Path;
use syn::{FnArg, ItemFn, ReturnType};

use super::type_extractor::parse_type;

/// Parse a Rust source file and extract Tauri commands
pub fn parse_commands(content: &str, source_file: &Path) -> Result<Vec<TauriCommand>> {
    let syntax = syn::parse_file(content)?;
    let mut commands = Vec::new();

    for item in syntax.items {
        match item {
            syn::Item::Fn(ref func) => {
                if is_tauri_command(func) {
                    if let Some(cmd) = parse_command_fn(func, source_file) {
                        commands.push(cmd);
                    }
                }
            }
            syn::Item::Impl(ref impl_block) => {
                // Also check for functions inside impl blocks
                for impl_item in &impl_block.items {
                    if let syn::ImplItem::Fn(method) = impl_item {
                        if is_tauri_command_method(method) {
                            if let Some(cmd) = parse_command_method(method, source_file) {
                                commands.push(cmd);
                            }
                        }
                    }
                }
            }
            syn::Item::Mod(ref module) => {
                // Check for functions inside mod blocks
                if let Some((_, ref items)) = module.content {
                    for mod_item in items {
                        if let syn::Item::Fn(func) = mod_item {
                            if is_tauri_command(func) {
                                if let Some(cmd) = parse_command_fn(func, source_file) {
                                    commands.push(cmd);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(commands)
}

/// Check if a function has the #[tauri::command] attribute
fn is_tauri_command(func: &ItemFn) -> bool {
    func.attrs.iter().any(is_tauri_command_attr)
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

/// Check if a method has the #[tauri::command] attribute
fn is_tauri_command_method(method: &syn::ImplItemFn) -> bool {
    method.attrs.iter().any(is_tauri_command_attr)
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

/// Parse a function into a TauriCommand
fn parse_command_fn(func: &ItemFn, source_file: &Path) -> Option<TauriCommand> {
    let name = func.sig.ident.to_string();

    let args = func
        .sig
        .inputs
        .iter()
        .filter_map(parse_fn_arg)
        .collect();

    let return_type = parse_return_type(&func.sig.output);
    let rename_all = extract_rename_all(&func.attrs);

    Some(TauriCommand {
        name,
        args,
        return_type,
        source_file: source_file.to_path_buf(),
        rename_all,
    })
}

/// Parse a method into a TauriCommand
fn parse_command_method(method: &syn::ImplItemFn, source_file: &Path) -> Option<TauriCommand> {
    let name = method.sig.ident.to_string();

    let args = method
        .sig
        .inputs
        .iter()
        .filter_map(parse_fn_arg)
        .collect();

    let return_type = parse_return_type(&method.sig.output);
    let rename_all = extract_rename_all(&method.attrs);

    Some(TauriCommand {
        name,
        args,
        return_type,
        source_file: source_file.to_path_buf(),
        rename_all,
    })
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
                RustType::Custom(name) => assert_eq!(name, "User"),
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
}

