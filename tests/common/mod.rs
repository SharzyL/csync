use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
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
    pub fn read_target_file(&self, rel_path: &str) -> Option<String> {
        let path = self.target.path().join(rel_path);
        fs::read_to_string(path).ok()
    }

    /// Count files in target directory (excluding directories and .gitignore)
    pub fn count_target_files(&self) -> usize {
        walkdir::WalkDir::new(self.target.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.file_name() != ".gitignore")
            .count()
    }
}

/// Helper to run csync command
pub struct CsyncRunner {
    binary_path: PathBuf,
}

impl CsyncRunner {
    pub fn new() -> Self {
        // Find the binary in the cargo target directory
        let binary = if cfg!(debug_assertions) {
            "target/debug/csync"
        } else {
            "target/release/csync"
        };

        Self {
            binary_path: PathBuf::from(binary),
        }
    }

    /// Run initial sync only (non-blocking)
    pub fn initial_sync(
        &self,
        source: &Path,
        target: &Path,
        extra_args: &[&str],
    ) -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg(source)
            .arg(target)
            .arg("--initial-sync-only")
            .args(extra_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.output()
    }

    /// Run csync in watch mode (returns child process that needs to be killed)
    pub fn watch(
        &self,
        source: &Path,
        target: &Path,
        extra_args: &[&str],
    ) -> std::io::Result<std::process::Child> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg(source)
            .arg(target)
            .args(extra_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.spawn()
    }
}

/// Wait for file to appear with timeout
pub fn wait_for_file(path: &Path, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Wait for file to be synced with content check
pub fn wait_for_file_content(path: &Path, expected: &str, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(content) = fs::read_to_string(path) {
            if content == expected {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}
