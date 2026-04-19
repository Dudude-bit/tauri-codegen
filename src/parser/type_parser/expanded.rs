//! Discover which types have `impl Serialize`/`Deserialize` blocks in
//! cargo-expand output. Derive macros are already expanded there, so the
//! presence of a `#[derive(Serialize)]` attribute on the struct itself is no
//! longer a reliable signal — we must find the generated impl blocks instead.

use std::collections::HashSet;
use syn::Item;

/// Collect names of all types that have `impl Serialize` or `impl Deserialize`
/// anywhere in the expanded item tree.
pub(super) fn collect_serializable_types(items: &[Item]) -> HashSet<String> {
    let mut result = HashSet::new();
    collect_recursive(items, &mut result);
    result
}

fn collect_recursive(items: &[Item], result: &mut HashSet<String>) {
    for item in items {
        match item {
            Item::Impl(item_impl) => {
                check_serde_impl(item_impl, result);
            }
            Item::Mod(module) => {
                if let Some((_, mod_items)) = &module.content {
                    collect_recursive(mod_items, result);
                }
            }
            Item::Const(item_const) => {
                // serde puts impl Serialize/Deserialize inside `const _: () = { impl ... }`.
                if let syn::Expr::Block(expr_block) = &*item_const.expr {
                    for stmt in &expr_block.block.stmts {
                        if let syn::Stmt::Item(Item::Impl(item_impl)) = stmt {
                            check_serde_impl(item_impl, result);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// If `item_impl` is an `impl Serialize for X` / `impl Deserialize for X`
/// block, record `X` in `result`.
fn check_serde_impl(item_impl: &syn::ItemImpl, result: &mut HashSet<String>) {
    if let Some((_, trait_path, _)) = &item_impl.trait_ {
        let trait_name = trait_path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();

        if trait_name == "Serialize" || trait_name == "Deserialize" {
            if let syn::Type::Path(type_path) = &*item_impl.self_ty {
                if let Some(segment) = type_path.path.segments.last() {
                    result.insert(segment.ident.to_string());
                }
            }
        }
    }
}
