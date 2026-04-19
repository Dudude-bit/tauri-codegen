use crate::known_types::{
    is_external_number_type, is_external_string_type, is_primitive_type, BYTES_TYPE,
    JSON_VALUE_TYPE,
};
use crate::models::RustType;
use std::collections::HashSet;
use syn::{GenericArgument, PathArguments, Type};

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

                // Check if it's a known primitive type
                if is_primitive_type(&name) {
                    // Special handling for str which maps to String
                    let normalized = if name == "str" || name == "char" {
                        "String".to_string()
                    } else {
                        name.clone()
                    };
                    return RustType::Primitive(normalized);
                }

                // Check if it's a known external type that serializes to string
                if is_external_string_type(&name) {
                    return RustType::Primitive(name);
                }

                // Check if it's a known external type that serializes to number
                if is_external_number_type(&name) {
                    return RustType::Primitive(name);
                }

                // Special types
                if name == JSON_VALUE_TYPE {
                    return RustType::Primitive(name);
                }
                if name == BYTES_TYPE {
                    return RustType::Primitive(name);
                }

                // Generic container types
                match name.as_str() {
                    "Vec" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Vec(Box::new(parse_type_with_context(&inner, generic_params)))
                        } else {
                            RustType::Unknown("Vec<?>".to_string())
                        }
                    }
                    "Option" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Option(Box::new(parse_type_with_context(
                                &inner,
                                generic_params,
                            )))
                        } else {
                            RustType::Unknown("Option<?>".to_string())
                        }
                    }
                    "Result" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Result(Box::new(parse_type_with_context(
                                &inner,
                                generic_params,
                            )))
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

                    // Transparent smart-pointer / wrapper containers: serde (and Tauri's JSON
                    // bridge) serialize the inner type unchanged, so the TypeScript output
                    // should reflect the inner type too. Cow<'a, T> is handled by skipping
                    // the leading lifetime argument when extracting the type parameter.
                    "Box" | "Rc" | "Arc" | "Cow" => {
                        if let Some(inner) = extract_first_type_arg(&segment.arguments) {
                            parse_type_with_context(&inner, generic_params)
                        } else {
                            RustType::Unknown(format!("{}<?>", name))
                        }
                    }

                    // Custom types (not a known generic param). Generic
                    // arguments on the final path segment (e.g. `Page<User>`)
                    // propagate into `Custom.args` so command signatures
                    // carry the concrete instantiation through to TS.
                    _ => {
                        let full_name = type_path
                            .path
                            .segments
                            .iter()
                            .map(|s| s.ident.to_string())
                            .collect::<Vec<_>>()
                            .join("::");

                        let args = match &segment.arguments {
                            PathArguments::AngleBracketed(angle) => angle
                                .args
                                .iter()
                                .filter_map(|arg| match arg {
                                    GenericArgument::Type(ty) => {
                                        Some(parse_type_with_context(ty, generic_params))
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>(),
                            _ => Vec::new(),
                        };

                        RustType::Custom {
                            name: full_name,
                            args,
                        }
                    }
                }
            } else {
                RustType::Unknown("unknown path".to_string())
            }
        }

        Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                RustType::Unit
            } else {
                let types = tuple
                    .elems
                    .iter()
                    .map(|t| parse_type_with_context(t, generic_params))
                    .collect();
                RustType::Tuple(types)
            }
        }

        Type::Reference(reference) => {
            // For references, we parse the inner type
            parse_type_with_context(&reference.elem, generic_params)
        }

        Type::Slice(slice) => {
            // Treat slices like Vec
            RustType::Vec(Box::new(parse_type_with_context(
                &slice.elem,
                generic_params,
            )))
        }

        // Unsupported shapes get a dedicated category so the caller can
        // surface a specific diagnostic instead of "Unknown(<opaque Debug>)".
        Type::TraitObject(_) => RustType::Unknown("dyn Trait".to_string()),
        Type::ImplTrait(_) => RustType::Unknown("impl Trait".to_string()),
        Type::BareFn(_) => RustType::Unknown("fn pointer".to_string()),
        Type::Ptr(_) => RustType::Unknown("raw pointer".to_string()),
        Type::Array(_) => RustType::Unknown("fixed-size array".to_string()),

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

/// Extract the first type argument, skipping lifetimes and const generics.
/// Needed for wrappers like Cow<'a, T> where the first arg is a lifetime.
fn extract_first_type_arg(args: &PathArguments) -> Option<Type> {
    if let PathArguments::AngleBracketed(angle) = args {
        for arg in &angle.args {
            if let GenericArgument::Type(ty) = arg {
                return Some(ty.clone());
            }
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

#[cfg(test)]
mod tests;
