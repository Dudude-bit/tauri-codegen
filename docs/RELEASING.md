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

## Cutting a release (with `cargo-release`)

One-time: `cargo install cargo-release` on your machine.

### Main crate only (the common case)

```sh
cargo release -p tauri-ts-generator minor --execute
git push --follow-tags
```

`minor` picks the next minor version from whatever's in `Cargo.toml`; use `patch`, `major`, or an explicit version like `1.9.0` if you want control. `--execute` actually writes changes — without it the command is a dry run, which is worth doing first.

The tool does exactly the four things you'd do by hand:

1. Bumps `version` in `Cargo.toml`.
2. Updates `Cargo.lock`.
3. Creates a commit titled `chore: Release tauri-ts-generator version X.Y.Z`.
4. Creates an annotated tag `vX.Y.Z`.

It does *not* push (see `push = false` in `release.toml`) — that's your last-chance review step. CI takes over once the tag lands on GitHub.

### When the derive crate also changed

Cut derive first, then main:

```sh
cargo release -p tauri-ts-generator-derive patch --execute
cargo release -p tauri-ts-generator minor --execute
git push --follow-tags
```

The first command bumps `tauri-ts-generator-derive`'s version *and* the dependency line that names it in the main `Cargo.toml` (via `dependent-version = "upgrade"`), then commits. No tag on the derive commit — only the main crate's tag drives CI. The second command does the usual main bump on top.

### Doing it by hand (without cargo-release)

If you can't install `cargo-release`, the manual sequence is:

```sh
# edit Cargo.toml (main) + tauri-ts-generator-derive/Cargo.toml if needed
cargo update -p tauri-ts-generator -p tauri-ts-generator-derive
git add Cargo.toml tauri-ts-generator-derive/Cargo.toml Cargo.lock
git commit -m "release: v1.9.0"
git tag v1.9.0
git push origin main --follow-tags
```

### After the tag lands

The `Release` workflow appears under "Actions" on GitHub as soon as the tag pushes. It:
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
