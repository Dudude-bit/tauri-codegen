//! Shared helpers for e2e tests: build a temp project, run the CLI, and
//! assert the generated output matches expected strings.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Path to the compiled `tauri-ts-generator` binary. Cargo exposes this
/// via the `CARGO_BIN_EXE_<name>` env var for integration tests.
pub fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_tauri-ts-generator")
}

/// Layout of a test project. `dir` is kept alive so the tempdir survives
/// for the duration of the test; the other fields are paths tests assert
/// against.
pub struct Project {
    #[allow(dead_code)] // held for Drop; tests don't read it directly
    pub dir: TempDir,
    pub types_out: PathBuf,
    pub commands_out: PathBuf,
}

impl Project {
    /// Build a minimal Tauri-like project with `rust_source` written to
    /// `src-tauri/src/lib.rs` and a config pointing at it.
    pub fn with_source(rust_source: &str) -> Self {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = dir.path();

        let src_dir = root.join("src-tauri").join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), rust_source).unwrap();

        let generated_dir = root.join("src").join("generated");
        fs::create_dir_all(&generated_dir).unwrap();
        let types_out = generated_dir.join("types.ts");
        let commands_out = generated_dir.join("commands.ts");

        fs::write(
            root.join("tauri-codegen.toml"),
            concat!(
                "[input]\n",
                "source_dir = \"src-tauri/src\"\n",
                "[output]\n",
                "types_file = \"src/generated/types.ts\"\n",
                "commands_file = \"src/generated/commands.ts\"\n",
            ),
        )
        .unwrap();

        Self {
            dir,
            types_out,
            commands_out,
        }
    }

    /// Write an extra source file under `src-tauri/src/<relative>`.
    pub fn add_source(&self, relative: &str, content: &str) {
        let path = self.dir.path().join("src-tauri").join("src").join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }
}

/// Run `tauri-ts-generator generate` inside `project` and return the captured
/// output. The caller decides whether to assert success or failure.
pub fn run_generate(project: &Project) -> Output {
    Command::new(binary_path())
        .current_dir(project.root())
        .arg("generate")
        .output()
        .expect("spawn tauri-ts-generator")
}

/// Run and assert success.
pub fn run_generate_ok(project: &Project) -> Output {
    let output = run_generate(project);
    assert!(
        output.status.success(),
        "expected success; exit={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Run and assert failure.
pub fn run_generate_err(project: &Project) -> Output {
    let output = run_generate(project);
    assert!(
        !output.status.success(),
        "expected failure, but succeeded.\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Assert that the file at `path` contains exactly `expected` (byte-equal,
/// ignoring leading/trailing whitespace so test strings can be readable).
pub fn assert_file_eq(path: &Path, expected: &str) {
    let actual =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        // In UPDATE mode, just rewrite — developer will inspect the diff.
        fs::write(path, expected).unwrap();
        return;
    }

    let actual_trim = actual.trim();
    let expected_trim = expected.trim();
    if actual_trim != expected_trim {
        panic!(
            "{} does not match expected:\n--- expected ---\n{}\n--- actual ---\n{}\n--- diff hint ---\nexpected {} bytes, got {}",
            path.display(),
            expected_trim,
            actual_trim,
            expected_trim.len(),
            actual_trim.len(),
        );
    }
}

/// Assert that `needle` appears somewhere in `haystack` (useful for stderr
/// assertions where we don't want to lock down the full format).
pub fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected output to contain `{}`, got:\n{}",
        needle,
        haystack
    );
}
