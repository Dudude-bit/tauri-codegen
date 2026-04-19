# Releasing

This project ships two crates to [crates.io](https://crates.io/):

- **`tauri-ts-generator`** — the main CLI + library.
- **`tauri-ts-generator-derive`** — a proc-macro sibling. Main depends on it, so it publishes first.

Releases are automated: tag a commit and GitHub Actions handles the rest (see `.github/workflows/release.yml`).

## One-time setup

1. **Create a crates.io API token.** https://crates.io/settings/tokens → "New Token" → restrict scope to `publish-new` + `publish-update` for the two crate names if you like.
2. **Add it to the repo.** GitHub → Settings → Secrets and variables → Actions → "New repository secret":
   - Name: `CARGO_REGISTRY_TOKEN`
   - Value: the token from step 1
3. Make sure the crate owner on crates.io matches the token owner (first publish does an `cargo owner --add` implicitly).

## Cutting a release

1. **Bump versions** in the Cargo.toml files you actually changed:
   - `Cargo.toml` (main crate) — always bump for a release.
   - `tauri-ts-generator-derive/Cargo.toml` — bump only if the derive crate itself changed.
   - If derive's version changed, **also update the `tauri-ts-generator-derive` dependency line** inside the main `Cargo.toml` to the new version.

2. **Update `Cargo.lock`** so CI doesn't complain about a mismatch:

   ```sh
   cargo update -p tauri-ts-generator -p tauri-ts-generator-derive
   ```

3. **Commit and tag.** The tag must match the main crate's new version exactly:

   ```sh
   git add Cargo.toml tauri-ts-generator-derive/Cargo.toml Cargo.lock
   git commit -m "release: v1.9.0"
   git tag v1.9.0
   git push origin main --follow-tags
   ```

4. **Watch the workflow.** The `Release` workflow appears under "Actions" on GitHub as soon as the tag pushes. It:
   1. Re-runs `fmt` / `clippy -D warnings` / full test suite / doc tests against the tagged ref.
   2. Confirms the tag name matches `Cargo.toml`'s `version` (guards against a stale local tag).
   3. Publishes the derive crate (skipped automatically if its version already exists on crates.io).
   4. Waits 30 seconds for the sparse index, then publishes the main crate.
   5. Drafts a GitHub release with auto-generated notes from the commit range.

If any step fails, fix forward and retag (`git tag -d vX.Y.Z`, bump to `vX.Y.Z+1`, push).

## Troubleshooting

- **`error: crate version X is already uploaded`** (main crate): the tag matches a version you've previously published. Bump the version and retag.
- **`error: failed to get registry index`** during publish-main: crates.io didn't finish indexing derive in 30s. Bump the `sleep` in `release.yml` or re-run the job.
- **Test failures in the release workflow but main branch was green**: the tag points at a different commit than you think. Check `git show $TAG`.
- **`cargo search` returns nothing**: first-ever release of the derive crate. The workflow treats that as "must publish" and proceeds — expected on v1.0.
- **Rejected by `deny_unknown_fields` in a user config**: that's them, not you — the release itself will still publish.

## Skipping publish

Push a tag that doesn't start with `v` (e.g. `2026-release-candidate`). The workflow is gated on `v*` and will stay silent.
