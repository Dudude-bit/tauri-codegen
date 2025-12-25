use super::{CommandArg, RustType, TauriCommand};
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use syn::{FnArg, GenericArgument, ItemFn, PathArguments, ReturnType, Type};

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
    func.attrs.iter().any(|attr| {
        if let syn::Meta::Path(path) = &attr.meta {
            let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
            // Check for #[tauri::command] or #[command]
            (segments.len() == 2 && segments[0] == "tauri" && segments[1] == "command")
                || (segments.len() == 1 && segments[0] == "command")
        } else {
            false
        }
    })
}

/// Check if a method has the #[tauri::command] attribute
fn is_tauri_command_method(method: &syn::ImplItemFn) -> bool {
    method.attrs.iter().any(|attr| {
        if let syn::Meta::Path(path) = &attr.meta {
            let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
            (segments.len() == 2 && segments[0] == "tauri" && segments[1] == "command")
                || (segments.len() == 1 && segments[0] == "command")
        } else {
            false
        }
    })
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

    Some(TauriCommand {
        name,
        args,
        return_type,
        source_file: source_file.to_path_buf(),
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

    Some(TauriCommand {
        name,
        args,
        return_type,
        source_file: source_file.to_path_buf(),
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

            // Skip special Tauri types like State, Window, AppHandle
            if is_tauri_special_type(&pat_type.ty) {
                return None;
            }

            let ty = parse_type(&pat_type.ty);

            Some(CommandArg { name, ty })
        }
        FnArg::Receiver(_) => None, // Skip self arguments
    }
}

/// Check if a type is a special Tauri type that should be skipped
fn is_tauri_special_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let name = segment.ident.to_string();
            // These are injected by Tauri and not passed from frontend
            return matches!(
                name.as_str(),
                "State" | "Window" | "AppHandle" | "Webview" | "WebviewWindow"
            );
        }
    }
    false
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

/// Parse a Rust type into our RustType representation (without generic context)
pub fn parse_type(ty: &Type) -> RustType {
    parse_type_with_context(ty, &HashSet::new())
}

/// Parse a Rust type with known generic parameters from the parent struct/enum
pub fn parse_type_with_context(ty: &Type, generic_params: &HashSet<String>) -> RustType {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let name = segment.ident.to_string();

                // First check if it's a known generic parameter from the context
                if generic_params.contains(&name) {
                    return RustType::Generic(name);
                }

                match name.as_str() {
                    // Primitive types
                    "String" | "str" | "char" => RustType::Primitive("String".to_string()),
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => {
                        RustType::Primitive(name.clone())
                    }
                    "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        RustType::Primitive(name.clone())
                    }
                    "f32" | "f64" => RustType::Primitive(name.clone()),
                    "bool" => RustType::Primitive("bool".to_string()),

                    // Well-known external types (serialized as strings)
                    "DateTime" | "NaiveDateTime" | "NaiveDate" | "NaiveTime" // chrono
                    | "OffsetDateTime" | "PrimitiveDateTime" | "Date" | "Time" // time crate
                    | "Uuid" // uuid
                    | "Decimal" | "BigDecimal" // decimal
                    | "PathBuf" | "Path" // std::path
                    | "Url" // url
                    | "IpAddr" | "Ipv4Addr" | "Ipv6Addr" // std::net
                    => RustType::Primitive(name.clone()),
                    
                    // Duration (serialized as number in milliseconds/seconds)
                    "Duration" => RustType::Primitive("Duration".to_string()),
                    
                    // serde_json::Value (any JSON)
                    "Value" => RustType::Primitive("Value".to_string()),
                    
                    // Bytes
                    "Bytes" => RustType::Primitive("Bytes".to_string()),

                    // Generic types
                    "Vec" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Vec(Box::new(parse_type_with_context(&inner, generic_params)))
                        } else {
                            RustType::Unknown("Vec<?>".to_string())
                        }
                    }
                    "Option" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Option(Box::new(parse_type_with_context(&inner, generic_params)))
                        } else {
                            RustType::Unknown("Option<?>".to_string())
                        }
                    }
                    "Result" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Result(Box::new(parse_type_with_context(&inner, generic_params)))
                        } else {
                            RustType::Unknown("Result<?>".to_string())
                        }
                    }
                    "HashMap" | "BTreeMap" => {
                        if let Some((key, value)) = extract_two_generics(&segment.arguments) {
                            RustType::HashMap {
                                key: Box::new(parse_type_with_context(&key, generic_params)),
                                value: Box::new(parse_type_with_context(&value, generic_params)),
                            }
                        } else {
                            RustType::Unknown("HashMap<?, ?>".to_string())
                        }
                    }

                    // Custom types (not a known generic param)
                    _ => RustType::Custom(name),
                }
            } else {
                RustType::Unknown("unknown path".to_string())
            }
        }

        Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                RustType::Unit
            } else {
                let types = tuple.elems.iter().map(|t| parse_type_with_context(t, generic_params)).collect();
                RustType::Tuple(types)
            }
        }

        Type::Reference(reference) => {
            // For references, we parse the inner type
            parse_type_with_context(&reference.elem, generic_params)
        }

        Type::Slice(slice) => {
            // Treat slices like Vec
            RustType::Vec(Box::new(parse_type_with_context(&slice.elem, generic_params)))
        }

        _ => RustType::Unknown(format!("{:?}", ty)),
    }
}

/// Extract a single generic type argument (for Vec<T>, Option<T>)
fn extract_single_generic(args: &PathArguments) -> Option<Type> {
    if let PathArguments::AngleBracketed(angle) = args {
        if let Some(GenericArgument::Type(ty)) = angle.args.first() {
            return Some(ty.clone());
        }
    }
    None
}

/// Extract two generic type arguments (for HashMap<K, V>)
fn extract_two_generics(args: &PathArguments) -> Option<(Type, Type)> {
    if let PathArguments::AngleBracketed(angle) = args {
        let mut iter = angle.args.iter();
        if let (Some(GenericArgument::Type(first)), Some(GenericArgument::Type(second))) =
            (iter.next(), iter.next())
        {
            return Some((first.clone(), second.clone()));
        }
    }
    None
}
