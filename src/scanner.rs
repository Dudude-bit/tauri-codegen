use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Scanner for finding Rust source files in a directory
pub struct Scanner {
    /// Root directory to scan
    source_dir: PathBuf,
    /// Patterns to exclude
    exclude_patterns: Vec<String>,
}

impl Scanner {
    /// Create a new scanner
    pub fn new(source_dir: PathBuf, exclude_patterns: Vec<String>) -> Self {
        Scanner {
            source_dir,
            exclude_patterns,
        }
    }

    /// Scan for all Rust source files
    pub fn scan(&self) -> Result<Vec<PathBuf>> {
        let mut rust_files = Vec::new();

        for entry in WalkDir::new(&self.source_dir)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !self.is_excluded(e.path()))
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && self.is_rust_file(path) {
                rust_files.push(path.to_path_buf());
            }
        }

        Ok(rust_files)
    }

    /// Check if a path is a Rust source file
    fn is_rust_file(&self, path: &Path) -> bool {
        path.extension().map(|ext| ext == "rs").unwrap_or(false)
    }

    /// Check if a path should be excluded
    /// Uses component-based matching for more precise exclusion
    fn is_excluded(&self, path: &Path) -> bool {
        for pattern in &self.exclude_patterns {
            // Check each path component for an exact match
            for component in path.components() {
                if let std::path::Component::Normal(name) = component {
                    if name.to_string_lossy() == *pattern {
                        return true;
                    }
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_is_rust_file() {
        let scanner = Scanner::new(PathBuf::from("."), vec![]);

        assert!(scanner.is_rust_file(Path::new("main.rs")));
        assert!(scanner.is_rust_file(Path::new("src/lib.rs")));
        assert!(!scanner.is_rust_file(Path::new("file.txt")));
        assert!(!scanner.is_rust_file(Path::new("file.ts")));
    }

    #[test]
    fn test_is_rust_file_edge_cases() {
        let scanner = Scanner::new(PathBuf::from("."), vec![]);

        assert!(!scanner.is_rust_file(Path::new("file"))); // No extension
        assert!(!scanner.is_rust_file(Path::new(".rs"))); // Hidden file
        assert!(!scanner.is_rust_file(Path::new("file.RS"))); // Uppercase
    }

    #[test]
    fn test_is_excluded() {
        let scanner = Scanner::new(
            PathBuf::from("."),
            vec!["target".to_string(), "tests".to_string()],
        );

        assert!(scanner.is_excluded(Path::new("target/debug/main.rs")));
        assert!(scanner.is_excluded(Path::new("src/tests/test.rs")));
        assert!(!scanner.is_excluded(Path::new("src/main.rs")));
    }

    #[test]
    fn test_is_excluded_exact_match() {
        let scanner = Scanner::new(PathBuf::from("."), vec!["tests".to_string()]);

        // Exact directory match
        assert!(scanner.is_excluded(Path::new("tests")));
        // Directory in path
        assert!(scanner.is_excluded(Path::new("src/tests/file.rs")));
        // Similar name but not exact match - should NOT be excluded with component-based matching
        assert!(!scanner.is_excluded(Path::new("my_tests/file.rs")));
        // Exact component within path
        assert!(scanner.is_excluded(Path::new("foo/tests/bar.rs")));
    }

    #[test]
    fn test_is_excluded_empty_patterns() {
        let scanner = Scanner::new(PathBuf::from("."), vec![]);

        assert!(!scanner.is_excluded(Path::new("target/debug/main.rs")));
        assert!(!scanner.is_excluded(Path::new("anything/at/all.rs")));
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let scanner = Scanner::new(dir.path().to_path_buf(), vec![]);

        let files = scanner.scan().unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_scan_with_rust_files() {
        let dir = tempdir().unwrap();

        // Create some Rust files
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn hello() {}").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), vec![]);
        let files = scanner.scan().unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|p| p.file_name().unwrap() == "main.rs"));
        assert!(files.iter().any(|p| p.file_name().unwrap() == "lib.rs"));
    }

    #[test]
    fn test_scan_nested_directories() {
        let dir = tempdir().unwrap();

        // Create nested structure
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(src.join("lib.rs"), "pub mod utils;").unwrap();

        let utils = src.join("utils");
        fs::create_dir_all(&utils).unwrap();
        fs::write(utils.join("mod.rs"), "pub fn util() {}").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), vec![]);
        let files = scanner.scan().unwrap();

        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_scan_excludes_directories() {
        let dir = tempdir().unwrap();

        // Create files in root
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        // Create files in excluded directory
        let target = dir.path().join("target");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("generated.rs"), "// generated").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), vec!["target".to_string()]);
        let files = scanner.scan().unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap() == "main.rs");
    }

    #[test]
    fn test_scan_mixed_file_types() {
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
        fs::write(dir.path().join("config.toml"), "[config]").unwrap();
        fs::write(dir.path().join("style.css"), "body {}").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), vec![]);
        let files = scanner.scan().unwrap();

        // Only .rs file should be found
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap() == "main.rs");
    }

    #[test]
    fn test_scan_multiple_exclude_patterns() {
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let target = dir.path().join("target");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("build.rs"), "// build").unwrap();

        let tests = dir.path().join("tests");
        fs::create_dir_all(&tests).unwrap();
        fs::write(tests.join("test.rs"), "// test").unwrap();

        let scanner = Scanner::new(
            dir.path().to_path_buf(),
            vec!["target".to_string(), "tests".to_string()],
        );
        let files = scanner.scan().unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap() == "main.rs");
    }

    #[test]
    fn test_scanner_new() {
        let scanner = Scanner::new(
            PathBuf::from("/some/path"),
            vec!["exclude1".to_string(), "exclude2".to_string()],
        );

        assert_eq!(scanner.source_dir, PathBuf::from("/some/path"));
        assert_eq!(scanner.exclude_patterns.len(), 2);
    }
}
