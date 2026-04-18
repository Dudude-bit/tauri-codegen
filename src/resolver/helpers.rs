//! Free helpers shared by the resolver implementation: path-comparison and
//! type-name extraction. Kept separate so `resolver.rs` stays focused on the
//! `ModuleResolver` impl.

pub(super) fn are_siblings(path_a: &[String], path_b: &[String]) -> bool {
    // Empty paths or single-element paths can't be siblings
    if path_a.len() < 2 || path_b.len() < 2 {
        return false;
    }
    // They must have the same length and share the same prefix except for the last segment
    path_a.len() == path_b.len() && path_a[..path_a.len() - 1] == path_b[..path_b.len() - 1]
}

pub(super) fn extract_base_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(type_path) => {
            // Extract the type name (first segment usually: State, MyState, etc.)
            type_path.path.segments.last().map(|s| s.ident.to_string())
        }
        syn::Type::Reference(type_ref) => {
            // Handle &T or &mut T
            extract_base_type_name(&type_ref.elem)
        }
        _ => None,
    }
}
