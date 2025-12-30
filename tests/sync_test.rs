mod common;

use common::TestEnv;

#[test]
fn test_parallel_sync_many_files() {
    let env = TestEnv::new();

    // Create many files to test parallel sync
    for i in 0..100 {
        env.create_source_file(&format!("file_{}.txt", i), &format!("content {}", i));
    }

    // Create nested directories
    for i in 0..10 {
        env.create_source_file(
            &format!("dir{}/subdir/file.txt", i),
            &format!("nested {}", i),
        );
    }

    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Verify all files are synced
    for i in 0..100 {
        assert!(
            env.target_file_exists(&format!("file_{}.txt", i)),
            "file_{}.txt missing",
            i
        );
    }

    for i in 0..10 {
        assert!(
            env.target_file_exists(&format!("dir{}/subdir/file.txt", i)),
            "dir{}/subdir/file.txt missing",
            i
        );
    }

    // Should have 110 files total
    assert_eq!(env.count_target_files(), 110);
}

#[test]
fn test_fast_initial_sync() {
    let env = TestEnv::new();

    // Create and sync files first
    env.create_source_file("file1.txt", "content");
    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Run fast sync again (should skip unchanged files)
    csync.initial_sync(true).expect("Fast sync failed");

    assert!(env.target_file_exists("file1.txt"));
}

#[test]
fn test_no_delete_flag() {
    let env = TestEnv::new();

    // Create and sync file
    env.create_source_file("keep_me.txt", "content");

    // First sync with no_delete = false
    let csync1 = env.create_csync(&[], true, false);
    csync1.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("keep_me.txt"));

    // Delete file from source
    std::fs::remove_file(env.source_path().join("keep_me.txt")).expect("Failed to delete");

    // Sync with no_delete = true
    let csync2 = env.create_csync(&[], true, true);
    csync2.initial_sync(false).expect("Sync failed");

    // File should still exist in target because no_delete is true
    assert!(
        env.target_file_exists("keep_me.txt"),
        "File was deleted despite no_delete flag"
    );
}

#[test]
fn test_empty_directories() {
    let env = TestEnv::new();

    // Create empty directories
    std::fs::create_dir_all(env.source_path().join("empty1")).expect("Failed to create dir");
    std::fs::create_dir_all(env.source_path().join("empty2/nested")).expect("Failed to create dir");

    // Create file in another directory
    env.create_source_file("nonempty/file.txt", "content");

    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Directories should be created
    assert!(env.target_path().join("empty1").is_dir());
    assert!(env.target_path().join("empty2/nested").is_dir());
    assert!(env.target_file_exists("nonempty/file.txt"));
}

#[test]
fn test_large_files() {
    let env = TestEnv::new();

    // Create a larger file (1MB)
    let large_content = "x".repeat(1024 * 1024);
    env.create_source_file("large.bin", &large_content);

    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert_eq!(
        env.read_target_file("large.bin"),
        Some(large_content),
        "Large file content mismatch"
    );
}

#[test]
fn test_special_characters_in_filenames() {
    let env = TestEnv::new();

    // Create files with special characters
    env.create_source_file("file with spaces.txt", "content");
    env.create_source_file("file-with-dashes.txt", "content");
    env.create_source_file("file_with_underscores.txt", "content");

    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("file with spaces.txt"));
    assert!(env.target_file_exists("file-with-dashes.txt"));
    assert!(env.target_file_exists("file_with_underscores.txt"));
}

#[test]
fn test_deeply_nested_directories() {
    let env = TestEnv::new();

    // Create a deeply nested structure
    let deep_path = "a/b/c/d/e/f/g/h/i/j/file.txt";
    env.create_source_file(deep_path, "deep content");

    let csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists(deep_path));
    assert_eq!(
        env.read_target_file(deep_path),
        Some("deep content".to_string())
    );
}
