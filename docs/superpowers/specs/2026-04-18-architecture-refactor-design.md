# Architecture refactor — design spec

**Status:** draft, awaiting review
**Context:** This spec is Track 4 of the January 2026 audit. Tracks 1–3 (naming fix, CI, smart pointers + recursion tests) are already merged. Track 4 is deferred to its own session because it is the highest-risk work and benefits most from an explicit plan and review cycle.

## Goal

Reduce the maintenance cost of the four largest source files and the duplicated type-walking logic, without changing public behavior. Keep the full test suite green at every step so CI catches regressions the moment they appear.

## What's in scope

1. **Decompose `src/parser/type_parser.rs` (1319 lines).** Extract three concerns into sibling files:
   - `serde_attrs.rs` — `SerdeContainerAttrs`, `parse_serde_container_attrs`, `get_serde_rename`, `apply_rename_all`, `is_serializable`, `has_serde_field_attrs`, `has_serde_variant_attrs`, `has_ts_optional`.
   - `expanded.rs` — `collect_serializable_types`, `check_serde_impl`, the `expanded`/`include_all` branches of `parse_items`.
   - `items.rs` — `parse_struct`, `parse_enum`, `parse_alias`, `parse_items`.
   Leave `type_parser.rs` as a thin module front matter that re-exports the public API.

2. **Decompose `src/resolver.rs` (1247 lines).** Split by lifecycle:
   - `imports.rs` — `use`-path collection (`parse_file`, helper walkers).
   - `resolution.rs` — `resolve_type`, `resolve_alias_target`, `ResolutionResult`.
   - `registry.rs` — `register_expanded_type_if_missing`, `type_definitions` map and its invariants.
   Keep `ModuleResolver` itself in `resolver.rs`.

3. **Decompose `src/pipeline.rs` (985 lines).** Move the `collect_reachable_types` machinery and its four helper closures into a new `src/pipeline/collect.rs`. Keep `Pipeline::run` and stage methods in `pipeline.rs`. Introduce a `CollectState` struct to thread the six `&mut` collections through one handle instead of eight arguments (retires the `#[allow(clippy::too_many_arguments)]`).

4. **Decompose `src/generator/types_gen.rs` (951 lines).** Split by output concern:
   - `types_gen/interface.rs` — struct emission (`generate_interface`, `render_field`, flatten handling).
   - `types_gen/enum_.rs` — enum emission (all four `EnumRepresentation` branches).
   - `types_gen/alias.rs` — type alias emission.

5. **Unify the type walk.** `pipeline.rs` currently defines `collect_custom_types_recursive` and two open-coded walks (over struct fields and enum variant data). Extract one public `walk_custom_types(ty, visitor)` in `src/models/rust_type.rs` (or a new `src/models/walk.rs`) and call it from all three sites.

6. **Normalize diagnostics.** Introduce a `Diagnostics` sink (or a minimal logger trait) that respects `Pipeline::verbose`. Replace the ad-hoc `eprintln!` sites that currently ignore the flag:
   - `pipeline.rs` cargo-expand info/warning lines
   - `pipeline.rs` per-file "Failed to parse …" warnings
   - `parser/type_parser.rs` unknown-`rename_all` warning
   Leave hard failures (`anyhow::bail!`) alone — they already halt the pipeline.

## What's explicitly out of scope

- No public-API changes. `lib.rs` re-exports stay identical.
- No behavior changes to the generated TypeScript. The golden `tests/fixtures/expected/` outputs must byte-match before and after.
- No new features. Smart pointers, naming, etc. are handled by earlier tracks.
- No dependency changes.

## Strategy

Each of the six items above becomes a separate PR. Order them from lowest blast radius to highest:

1. **Unify the type walk** (item 5) — tiny, local, covered by existing tests.
2. **Normalize diagnostics** (item 6) — additive; existing `eprintln!` call sites just route through a new sink.
3. **Pipeline collect extraction** (item 3) — contained to `pipeline.rs` internals.
4. **Resolver decomposition** (item 2) — touches many call sites but each move is mechanical.
5. **Type parser decomposition** (item 1) — largest file; do last among parsers.
6. **Types-gen decomposition** (item 4) — largest file overall; do it alone.

After each PR: run `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. If any fails, fix before moving on. Do **not** stack these PRs — land each on `main` before starting the next.

## Risks

- **`syn` re-export churn.** Splitting `type_parser.rs` can cascade import changes across tests. Mitigate by keeping a re-export shim in the old file for one PR, then pruning.
- **Circular module deps.** `resolver` and `parser` both touch type items; watch for accidental cycles when moving `ParsedTypes`.
- **Invisible behavior drift.** The diagnostics rewrite is the easiest place to accidentally silence a previously-visible warning. Add a regression test that captures a warning path (invalid `rename_all`) and asserts the message appears.

## Acceptance criteria

- All four files drop below 500 lines. New files stay under 500 lines.
- `collect_custom_types_recursive` no longer exists; every walk goes through the unified visitor.
- No `eprintln!` calls outside the new diagnostics sink (or a documented exception for `main.rs`).
- `#[allow(clippy::too_many_arguments)]` on `resolve_and_enqueue` is removed.
- `cargo test` count is at least what it is today (265); byte-for-byte comparison of `tests/fixtures/expected/*.ts` outputs unchanged.

## Open questions

- Should `Diagnostics` be a trait object (`Box<dyn Diagnostics>`) to allow test capture, or a simple concrete struct with a verbosity flag? Leaning concrete struct; the only consumer today is the CLI.
- For `types_gen` split, should enum rendering live next to `EnumRepresentation` in `models/`, or stay in `generator/`? Leaning `generator/` to keep the `models` layer free of output logic.

Review these before starting implementation.
