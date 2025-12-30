use csync::Csync;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Test fixture that creates temporary source and target directories
pub struct TestEnv {
    pub source: TempDir,
    pub target: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        Self {
            source: TempDir::new().expect("Failed to create source temp dir"),
            target: TempDir::new().expect("Failed to create target temp dir"),
        }
    }

    pub fn source_path(&self) -> &Path {
        self.source.path()
    }

    pub fn target_path(&self) -> &Path {
        self.target.path()
    }

    /// Create a file in the source directory
    pub fn create_source_file(&self, rel_path: &str, content: &str) -> PathBuf {
        let path = self.source.path().join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directory");
        }
        fs::write(&path, content).expect("Failed to write file");
        path
    }

    /// Create a .gitignore file in the source directory
    #[allow(dead_code)]
    pub fn create_gitignore(&self, rel_path: &str, patterns: &[&str]) -> PathBuf {
        let gitignore_path = self.source.path().join(rel_path).join(".gitignore");
        if let Some(parent) = gitignore_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directory");
        }
        let content = patterns.join("\n");
        fs::write(&gitignore_path, content).expect("Failed to write .gitignore");
        gitignore_path
    }

    /// Check if a file exists in the target directory
    pub fn target_file_exists(&self, rel_path: &str) -> bool {
        self.target.path().join(rel_path).exists()
    }

    /// Read a file from the target directory
    #[allow(dead_code)]
    pub fn read_target_file(&self, rel_path: &str) -> Option<String> {
        let path = self.target.path().join(rel_path);
        fs::read_to_string(path).ok()
    }

    /// Count files in target directory (excluding directories and .gitignore)
    #[allow(dead_code)]
    pub fn count_target_files(&self) -> usize {
        walkdir::WalkDir::new(self.target.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.file_name() != ".gitignore")
            .count()
    }

    /// Create a Csync instance for this test environment
    pub fn create_csync(
        &self,
        ignore_patterns: &[String],
        use_gitignore: bool,
        no_delete: bool,
    ) -> Csync {
        let patterns_vec = ignore_patterns.to_vec();
        Csync::new(
            self.source_path(),
            self.target_path(),
            &patterns_vec,
            use_gitignore,
            no_delete,
        )
        .expect("Failed to create Csync")
    }
}
