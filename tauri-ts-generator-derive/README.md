# tauri-ts-generator-derive

Proc-macro companion to [`tauri-ts-generator`](https://crates.io/crates/tauri-ts-generator).
Registers the `#[ts(...)]` attribute namespace so structs annotated with
`#[derive(TS)]` can carry codegen hints like:

```rust
use tauri_ts_generator::TS;

#[derive(serde::Serialize, TS)]
pub struct Config {
    #[ts(optional)]
    pub volume: Option<f32>,
}
```

You don't depend on this crate directly — add `tauri-ts-generator` and
re-export `TS` from there. See the main crate's README for the full usage
guide and configuration reference.
