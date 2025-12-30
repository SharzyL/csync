mod common;

use common::{CsyncRunner, TestEnv};
use std::time::Duration;

#[test]
fn test_root_gitignore_initial_sync() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create files
    env.create_source_file("file1.txt", "content1");
    env.create_source_file("file2.log", "log content");
    env.create_source_file("temp_file.txt", "temp");

    // Create root .gitignore
    env.create_gitignore(".", &["*.log", "temp_*"]);

    // Run initial sync
    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success(), "csync failed: {:?}", output);

    // Check results
    assert!(env.target_file_exists("file1.txt"), "file1.txt should be synced");
    assert!(!env.target_file_exists("file2.log"), "file2.log should be ignored");
    assert!(!env.target_file_exists("temp_file.txt"), "temp_file.txt should be ignored");
}

#[test]
fn test_subdirectory_gitignore_initial_sync() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create directory structure
    env.create_source_file("root.txt", "root content");
    env.create_source_file("subdir/file.txt", "file content");
    env.create_source_file("subdir/build.o", "object file");
    env.create_source_file("subdir/nested/data.txt", "data");
    env.create_source_file("subdir/nested/secret.key", "secret");

    // Create .gitignore files at different levels
    env.create_gitignore(".", &["*.log"]);
    env.create_gitignore("subdir", &["*.o", "*.a"]);
    env.create_gitignore("subdir/nested", &["*.key", "*.pem"]);

    // Run initial sync
    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success(), "csync failed");

    // Verify synced files
    assert!(env.target_file_exists("root.txt"));
    assert!(env.target_file_exists("subdir/file.txt"));
    assert!(env.target_file_exists("subdir/nested/data.txt"));

    // Verify ignored files
    assert!(!env.target_file_exists("subdir/build.o"));
    assert!(!env.target_file_exists("subdir/nested/secret.key"));
}

#[test]
fn test_nested_gitignore_patterns() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create deep directory structure
    env.create_source_file("level1/file1.txt", "l1");
    env.create_source_file("level1/level2/file2.txt", "l2");
    env.create_source_file("level1/level2/build.o", "build");
    env.create_source_file("level1/level2/level3/file3.txt", "l3");
    env.create_source_file("level1/level2/level3/temp.bin", "temp");

    // .gitignore at level2
    env.create_gitignore("level1/level2", &["*.o", "*.bin"]);

    // Run initial sync
    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success());

    // Files should be synced
    assert!(env.target_file_exists("level1/file1.txt"));
    assert!(env.target_file_exists("level1/level2/file2.txt"));
    assert!(env.target_file_exists("level1/level2/level3/file3.txt"));

    // Build artifacts should be ignored
    assert!(!env.target_file_exists("level1/level2/build.o"));
    assert!(!env.target_file_exists("level1/level2/level3/temp.bin"));
}

#[test]
fn test_gitignore_runtime_sync() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Setup initial files and gitignore
    env.create_source_file("existing.txt", "existing");
    env.create_gitignore(".", &["*.log", "*.tmp"]);
    env.create_gitignore("subdir", &["*.o"]);

    // Run initial sync first
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed initial sync");

    // Start watching
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &["--debug"])
        .expect("Failed to start watch");

    // Give csync time to start watching
    std::thread::sleep(Duration::from_millis(500));

    // Create new files that should be synced
    env.create_source_file("new_file.txt", "new content");
    env.create_source_file("subdir/code.rs", "rust code");

    // Create new files that should be ignored
    env.create_source_file("debug.log", "log");
    env.create_source_file("temp.tmp", "temp");
    env.create_source_file("subdir/build.o", "object");

    // Wait for syncs
    std::thread::sleep(Duration::from_millis(1500));

    // Kill the watcher
    child.kill().expect("Failed to kill child process");
    let _ = child.wait();

    // Verify synced files
    assert!(env.target_file_exists("new_file.txt"));
    assert!(env.target_file_exists("subdir/code.rs"));

    // Verify ignored files
    assert!(!env.target_file_exists("debug.log"));
    assert!(!env.target_file_exists("temp.tmp"));
    assert!(!env.target_file_exists("subdir/build.o"));
}

#[test]
fn test_gitignore_directory_patterns() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create directory structure
    env.create_source_file("src/main.rs", "main");
    env.create_source_file("target/debug/app", "binary");
    env.create_source_file("target/release/app", "binary");
    env.create_source_file("cache/data.db", "cache");

    // Gitignore directories
    env.create_gitignore(".", &["target/", "cache/"]);

    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success());

    // Source files should sync
    assert!(env.target_file_exists("src/main.rs"));

    // Ignored directories shouldn't sync
    assert!(!env.target_file_exists("target/debug/app"));
    assert!(!env.target_file_exists("target/release/app"));
    assert!(!env.target_file_exists("cache/data.db"));
}

#[test]
fn test_no_gitignore_flag() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create files and gitignore
    env.create_source_file("file.txt", "content");
    env.create_source_file("debug.log", "log");
    env.create_gitignore(".", &["*.log"]);

    // Run with --no-git-ignore flag
    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &["--no-git-ignore"])
        .expect("Failed to run csync");

    assert!(output.status.success());

    // Both files should be synced when gitignore is disabled
    assert!(env.target_file_exists("file.txt"), "file.txt should be synced");
    assert!(
        env.target_file_exists("debug.log"),
        "debug.log should be synced with --no-git-ignore (gitignore *.log pattern should be ignored)"
    );
}

#[test]
fn test_custom_ignore_patterns() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create files
    env.create_source_file("file.txt", "content");
    env.create_source_file("test.rs", "test");
    env.create_source_file("backup.bak", "backup");

    // Use custom ignore patterns (no .gitignore file)
    let output = runner
        .initial_sync(
            env.source_path(),
            env.target_path(),
            &["-i", "*.bak", "-i", "test.*"],
        )
        .expect("Failed to run csync");

    assert!(output.status.success());

    // Only file.txt should be synced
    assert!(env.target_file_exists("file.txt"));
    assert!(!env.target_file_exists("test.rs"));
    assert!(!env.target_file_exists("backup.bak"));
}
