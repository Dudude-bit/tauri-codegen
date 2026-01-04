//! Derive macro for tauri-ts-generator
//!
//! This crate provides the `#[derive(TS)]` macro which registers the `ts` attribute
//! namespace, allowing `#[ts(optional)]` annotations on struct fields.
//!
//! The derive macro itself is a no-op - it doesn't generate any runtime code.
//! Its sole purpose is to make the Rust compiler accept `#[ts(...)]` attributes
//! which are then parsed at code generation time by `tauri-ts-generator`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro that enables `#[ts(...)]` attributes on struct/enum fields.
///
/// This macro is a no-op at compile time. It simply registers the `ts` attribute
/// namespace so that the Rust compiler doesn't error on `#[ts(optional)]` attributes.
///
/// # Example
///
/// ```rust
/// use tauri_ts_generator_derive::TS;
///
/// #[derive(TS)]
/// pub struct Config {
///     // This field will be typed as `prop?: number` in TypeScript
///     #[ts(optional)]
///     pub volume: Option<f32>,
///     
///     // This field will be typed as `string | null` (default behavior)
///     pub name: Option<String>,
/// }
/// ```
#[proc_macro_derive(TS, attributes(ts))]
pub fn derive_ts(input: TokenStream) -> TokenStream {
    // Parse the input to validate syntax, but don't generate any code
    let _ = parse_macro_input!(input as DeriveInput);
    
    // Return empty token stream - this is a no-op derive
    TokenStream::from(quote! {})
}
