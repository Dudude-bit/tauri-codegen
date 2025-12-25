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
        path.extension()
            .map(|ext| ext == "rs")
            .unwrap_or(false)
    }

    /// Check if a path should be excluded
    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.exclude_patterns {
            // Check if any component of the path matches the exclude pattern
            if path_str.contains(pattern) {
                return true;
            }

            // Also check against the file/directory name
            if let Some(name) = path.file_name() {
                if name.to_string_lossy() == *pattern {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_rust_file() {
        let scanner = Scanner::new(PathBuf::from("."), vec![]);

        assert!(scanner.is_rust_file(Path::new("main.rs")));
        assert!(scanner.is_rust_file(Path::new("src/lib.rs")));
        assert!(!scanner.is_rust_file(Path::new("file.txt")));
        assert!(!scanner.is_rust_file(Path::new("file.ts")));
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
}

