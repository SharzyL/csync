mod common;

use common::{wait_for_file, wait_for_file_content, CsyncRunner, TestEnv};
use std::fs;
use std::time::Duration;

#[test]
fn test_basic_initial_sync() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create test files
    env.create_source_file("file1.txt", "content1");
    env.create_source_file("file2.txt", "content2");
    env.create_source_file("subdir/file3.txt", "content3");

    // Run initial sync
    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success(), "csync failed");

    // Verify files are synced
    assert_eq!(env.read_target_file("file1.txt"), Some("content1".to_string()));
    assert_eq!(env.read_target_file("file2.txt"), Some("content2".to_string()));
    assert_eq!(
        env.read_target_file("subdir/file3.txt"),
        Some("content3".to_string())
    );
}

#[test]
fn test_parallel_sync_many_files() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

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

    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success());

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
    let runner = CsyncRunner::new();

    // Create and sync files first
    env.create_source_file("file1.txt", "content");
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Initial sync failed");

    // Run fast sync again (should skip unchanged files)
    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &["--fast-initial-sync"])
        .expect("Fast sync failed");

    assert!(output.status.success());
    assert!(env.target_file_exists("file1.txt"));
}

#[test]
fn test_runtime_file_creation() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Start with empty source
    env.create_source_file("initial.txt", "initial");

    // Run initial sync
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Initial sync failed");

    // Start watching
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &[])
        .expect("Failed to start watch");

    std::thread::sleep(Duration::from_millis(500));

    // Create new file
    let new_file_path = env.create_source_file("new_file.txt", "new content");

    // Wait for sync
    let target_path = env.target_path().join("new_file.txt");
    assert!(
        wait_for_file_content(&target_path, "new content", Duration::from_secs(3)),
        "New file not synced"
    );

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_runtime_file_modification() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create initial file
    env.create_source_file("modify.txt", "original");

    // Initial sync
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Initial sync failed");

    // Start watching
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &[])
        .expect("Failed to start watch");

    std::thread::sleep(Duration::from_millis(500));

    // Modify file
    let file_path = env.source_path().join("modify.txt");
    fs::write(&file_path, "modified").expect("Failed to modify file");

    // Wait for sync
    let target_path = env.target_path().join("modify.txt");
    assert!(
        wait_for_file_content(&target_path, "modified", Duration::from_secs(3)),
        "Modified file not synced"
    );

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_runtime_file_deletion() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create file
    env.create_source_file("delete_me.txt", "content");

    // Initial sync
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Initial sync failed");

    assert!(env.target_file_exists("delete_me.txt"));

    // Start watching
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &[])
        .expect("Failed to start watch");

    std::thread::sleep(Duration::from_millis(500));

    // Delete file
    let file_path = env.source_path().join("delete_me.txt");
    fs::remove_file(&file_path).expect("Failed to delete file");

    // Wait for deletion to sync
    std::thread::sleep(Duration::from_millis(1500));

    assert!(
        !env.target_file_exists("delete_me.txt"),
        "Deleted file still exists in target"
    );

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_no_delete_flag() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create and sync file
    env.create_source_file("keep_me.txt", "content");
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Initial sync failed");

    // Start watching with --no-delete
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &["--no-delete"])
        .expect("Failed to start watch");

    std::thread::sleep(Duration::from_millis(500));

    // Delete file from source
    fs::remove_file(env.source_path().join("keep_me.txt")).expect("Failed to delete");

    // Wait a bit
    std::thread::sleep(Duration::from_millis(1500));

    // File should still exist in target
    assert!(
        env.target_file_exists("keep_me.txt"),
        "File was deleted despite --no-delete flag"
    );

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_file_rename() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create initial file
    env.create_source_file("old_name.txt", "content");
    runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Initial sync failed");

    // Start watching
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &[])
        .expect("Failed to start watch");

    std::thread::sleep(Duration::from_millis(500));

    // Rename file
    let old_path = env.source_path().join("old_name.txt");
    let new_path = env.source_path().join("new_name.txt");
    fs::rename(&old_path, &new_path).expect("Failed to rename file");

    // Wait for sync
    std::thread::sleep(Duration::from_millis(1500));

    // New file should exist, old should not
    assert!(env.target_file_exists("new_name.txt"), "Renamed file not found");
    assert!(
        !env.target_file_exists("old_name.txt"),
        "Old file still exists"
    );

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_skip_initial_sync() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create file but don't run initial sync
    env.create_source_file("file.txt", "content");

    // Start watching with --skip-initial-sync
    let mut child = runner
        .watch(env.source_path(), env.target_path(), &["--skip-initial-sync"])
        .expect("Failed to start watch");

    std::thread::sleep(Duration::from_millis(500));

    // File should NOT be synced yet
    assert!(!env.target_file_exists("file.txt"));

    // Create new file after starting watch
    env.create_source_file("new.txt", "new");

    // New file should be synced
    let target_path = env.target_path().join("new.txt");
    assert!(
        wait_for_file(&target_path, Duration::from_secs(3)),
        "New file not synced"
    );

    child.kill().expect("Failed to kill child");
    let _ = child.wait();
}

#[test]
fn test_empty_directories() {
    let env = TestEnv::new();
    let runner = CsyncRunner::new();

    // Create empty directories
    fs::create_dir_all(env.source_path().join("empty1")).expect("Failed to create dir");
    fs::create_dir_all(env.source_path().join("empty2/nested")).expect("Failed to create dir");

    // Create file in another directory
    env.create_source_file("nonempty/file.txt", "content");

    let output = runner
        .initial_sync(env.source_path(), env.target_path(), &[])
        .expect("Failed to run csync");

    assert!(output.status.success());

    // Directories should be created
    assert!(env.target_path().join("empty1").is_dir());
    assert!(env.target_path().join("empty2/nested").is_dir());
    assert!(env.target_file_exists("nonempty/file.txt"));
}
