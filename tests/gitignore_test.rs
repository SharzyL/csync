mod common;

use common::TestEnv;

#[test]
fn test_root_gitignore_initial_sync() {
    let env = TestEnv::new();

    // Create files
    env.create_source_file("file1.txt", "content1");
    env.create_source_file("file2.log", "log content");
    env.create_source_file("temp_file.txt", "temp");

    // Create root .gitignore
    env.create_gitignore(".", &["*.log", "temp_*"]);

    // Create csync and run initial sync
    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Check results
    assert!(
        env.target_file_exists("file1.txt"),
        "file1.txt should be synced"
    );
    assert!(
        !env.target_file_exists("file2.log"),
        "file2.log should be ignored"
    );
    assert!(
        !env.target_file_exists("temp_file.txt"),
        "temp_file.txt should be ignored"
    );
}

#[test]
fn test_subdirectory_gitignore_initial_sync() {
    let env = TestEnv::new();

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
    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

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

    // Create deep directory structure
    env.create_source_file("level1/file1.txt", "l1");
    env.create_source_file("level1/level2/file2.txt", "l2");
    env.create_source_file("level1/level2/build.o", "build");
    env.create_source_file("level1/level2/level3/file3.txt", "l3");
    env.create_source_file("level1/level2/level3/temp.bin", "temp");

    // .gitignore at level2
    env.create_gitignore("level1/level2", &["*.o", "*.bin"]);

    // Run initial sync
    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Files should be synced
    assert!(env.target_file_exists("level1/file1.txt"));
    assert!(env.target_file_exists("level1/level2/file2.txt"));
    assert!(env.target_file_exists("level1/level2/level3/file3.txt"));

    // Build artifacts should be ignored
    assert!(!env.target_file_exists("level1/level2/build.o"));
    assert!(!env.target_file_exists("level1/level2/level3/temp.bin"));
}

#[test]
fn test_gitignore_directory_patterns() {
    let env = TestEnv::new();

    // Create directory structure
    env.create_source_file("src/main.rs", "main");
    env.create_source_file("target/debug/app", "binary");
    env.create_source_file("target/release/app", "binary");
    env.create_source_file("cache/data.db", "cache");

    // Gitignore directories
    env.create_gitignore(".", &["target/", "cache/"]);

    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

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

    // Create files and gitignore
    env.create_source_file("file.txt", "content");
    env.create_source_file("debug.log", "log");
    env.create_gitignore(".", &["*.log"]);

    // Create csync with use_gitignore = false
    let csync = env.create_csync(&[], false, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Both files should be synced when gitignore is disabled
    assert!(
        env.target_file_exists("file.txt"),
        "file.txt should be synced"
    );
    assert!(
        env.target_file_exists("debug.log"),
        "debug.log should be synced with --no-git-ignore (gitignore *.log pattern should be ignored)"
    );
}

#[test]
fn test_custom_ignore_patterns() {
    let env = TestEnv::new();

    // Create files
    env.create_source_file("file.txt", "content");
    env.create_source_file("test.rs", "test");
    env.create_source_file("backup.bak", "backup");

    // Use custom ignore patterns (no .gitignore file)
    let csync = env.create_csync(&["*.bak".to_string(), "test.*".to_string()], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Only file.txt should be synced
    assert!(env.target_file_exists("file.txt"));
    assert!(!env.target_file_exists("test.rs"));
    assert!(!env.target_file_exists("backup.bak"));
}
