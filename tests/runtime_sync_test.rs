mod common;

use common::TestEnv;
use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode};
use notify::{Event, EventKind};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

/// Helper to create a basic Event
fn create_event(kind: EventKind, paths: Vec<PathBuf>) -> Event {
    Event {
        kind,
        paths,
        attrs: Default::default(),
    }
}

#[test]
fn test_runtime_file_creation() {
    let env = TestEnv::new();

    // Initial sync with one file
    env.create_source_file("existing.txt", "existing");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("existing.txt"));

    // Simulate runtime file creation
    let new_file = env.create_source_file("new_file.txt", "new content");
    let event = create_event(EventKind::Create(CreateKind::File), vec![new_file]);
    csync.handle_event(&event);

    // Verify new file was synced
    assert!(env.target_file_exists("new_file.txt"));
    assert_eq!(
        env.read_target_file("new_file.txt"),
        Some("new content".to_string())
    );
}

#[test]
fn test_runtime_file_modification() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("file.txt", "original content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert_eq!(
        env.read_target_file("file.txt"),
        Some("original content".to_string())
    );

    // Modify the file
    let modified_file = env.source_path().join("file.txt");
    fs::write(&modified_file, "modified content").expect("Failed to modify file");

    // Simulate modify event
    let event = create_event(
        EventKind::Modify(ModifyKind::Data(DataChange::Any)),
        vec![modified_file],
    );
    csync.handle_event(&event);

    // Verify file was updated
    assert_eq!(
        env.read_target_file("file.txt"),
        Some("modified content".to_string())
    );
}

#[test]
fn test_runtime_file_deletion() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("to_delete.txt", "content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("to_delete.txt"));

    // Delete the file
    let deleted_file = env.source_path().join("to_delete.txt");
    fs::remove_file(&deleted_file).expect("Failed to delete file");

    // Simulate remove event
    let event = create_event(EventKind::Remove(RemoveKind::File), vec![deleted_file]);
    csync.handle_event(&event);

    // Verify file was deleted from target
    assert!(!env.target_file_exists("to_delete.txt"));
}

#[test]
fn test_runtime_file_deletion_with_no_delete_flag() {
    let env = TestEnv::new();

    // Initial sync with no_delete = true
    env.create_source_file("keep_me.txt", "content");
    let mut csync = env.create_csync(&[], true, true);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("keep_me.txt"));

    // Delete the file from source
    let deleted_file = env.source_path().join("keep_me.txt");
    fs::remove_file(&deleted_file).expect("Failed to delete file");

    // Simulate remove event
    let event = create_event(EventKind::Remove(RemoveKind::File), vec![deleted_file]);
    csync.handle_event(&event);

    // Verify file still exists in target (no_delete flag)
    assert!(
        env.target_file_exists("keep_me.txt"),
        "File should not be deleted when no_delete flag is set"
    );
}

#[test]
fn test_runtime_directory_creation() {
    let env = TestEnv::new();

    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Create directory and file in it
    let new_dir = env.source_path().join("new_dir");
    fs::create_dir(&new_dir).expect("Failed to create directory");

    let event = create_event(EventKind::Create(CreateKind::Folder), vec![new_dir]);
    csync.handle_event(&event);

    // Create file in the new directory
    let new_file = env.create_source_file("new_dir/file.txt", "content");
    let event = create_event(EventKind::Create(CreateKind::File), vec![new_file]);
    csync.handle_event(&event);

    // Verify directory and file exist
    assert!(env.target_path().join("new_dir").is_dir());
    assert!(env.target_file_exists("new_dir/file.txt"));
}

#[test]
fn test_runtime_directory_deletion() {
    let env = TestEnv::new();

    // Initial sync with directory
    env.create_source_file("dir_to_delete/file.txt", "content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("dir_to_delete/file.txt"));

    // Delete file first
    let file_path = env.source_path().join("dir_to_delete/file.txt");
    fs::remove_file(&file_path).expect("Failed to delete file");
    let event = create_event(EventKind::Remove(RemoveKind::File), vec![file_path]);
    csync.handle_event(&event);

    // Delete directory
    let dir_path = env.source_path().join("dir_to_delete");
    fs::remove_dir(&dir_path).expect("Failed to delete directory");
    let event = create_event(EventKind::Remove(RemoveKind::Folder), vec![dir_path]);
    csync.handle_event(&event);

    // Verify directory and file are deleted from target
    assert!(!env.target_file_exists("dir_to_delete/file.txt"));
    assert!(!env.target_path().join("dir_to_delete").exists());
}

#[test]
fn test_runtime_file_rename_atomic() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("old_name.txt", "content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("old_name.txt"));

    // Rename file (atomic operation with both paths)
    let old_path = env.source_path().join("old_name.txt");
    let new_path = env.source_path().join("new_name.txt");
    fs::rename(&old_path, &new_path).expect("Failed to rename file");

    // Simulate rename event with both paths
    let event = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        vec![old_path, new_path],
    );
    csync.handle_event(&event);

    // Verify old file is gone and new file exists
    assert!(!env.target_file_exists("old_name.txt"));
    assert!(env.target_file_exists("new_name.txt"));
    assert_eq!(
        env.read_target_file("new_name.txt"),
        Some("content".to_string())
    );
}

#[test]
fn test_runtime_file_rename_with_tracker() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("file.txt", "content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("file.txt"));

    // Simulate rename with separate MOVED_FROM and MOVED_TO events (with tracker)
    let old_path = env.source_path().join("file.txt");
    let new_path = env.source_path().join("renamed.txt");

    // MOVED_FROM event with tracker cookie 123
    let mut event_from = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::From)),
        vec![old_path.clone()],
    );
    event_from.attrs.set_tracker(123);
    csync.handle_event(&event_from);

    // Actually rename the file
    fs::rename(&old_path, &new_path).expect("Failed to rename file");

    // MOVED_TO event with same tracker cookie
    let mut event_to = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::To)),
        vec![new_path],
    );
    event_to.attrs.set_tracker(123);
    csync.handle_event(&event_to);

    // Verify rename happened
    assert!(!env.target_file_exists("file.txt"));
    assert!(env.target_file_exists("renamed.txt"));
    assert_eq!(
        env.read_target_file("renamed.txt"),
        Some("content".to_string())
    );
}

#[test]
fn test_runtime_editor_save_sequence() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("file.txt", "original");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert_eq!(
        env.read_target_file("file.txt"),
        Some("original".to_string())
    );

    // Simulate editor save sequence:
    // 1. MOVE file.txt to file.txt~
    let file_path = env.source_path().join("file.txt");
    let backup_path = env.source_path().join("file.txt~");

    let mut event_from = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::From)),
        vec![file_path.clone()],
    );
    event_from.attrs.set_tracker(456);
    csync.handle_event(&event_from);

    fs::rename(&file_path, &backup_path).expect("Failed to create backup");

    let mut event_to = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::To)),
        vec![backup_path.clone()],
    );
    event_to.attrs.set_tracker(456);
    csync.handle_event(&event_to);

    // 2. CREATE file.txt
    fs::write(&file_path, "modified").expect("Failed to create new file");
    let event = create_event(EventKind::Create(CreateKind::File), vec![file_path.clone()]);
    csync.handle_event(&event);

    // 3. MODIFY file.txt (should be ignored due to ephemeral cache)
    let event = create_event(
        EventKind::Modify(ModifyKind::Data(DataChange::Any)),
        vec![file_path.clone()],
    );
    csync.handle_event(&event);

    // At this point, file should not be synced yet (in ephemeral cache)
    // The target should still have original content or be unchanged
    // Let's verify by checking the actual state

    // 4. ATTRIB event (metadata change) - this should trigger sync
    let event = create_event(
        EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::Any)),
        vec![file_path.clone()],
    );
    csync.handle_event(&event);

    // Now file should be synced with new content
    assert_eq!(
        env.read_target_file("file.txt"),
        Some("modified".to_string())
    );

    // 5. DELETE file.txt~
    fs::remove_file(&backup_path).expect("Failed to delete backup");
    let event = create_event(EventKind::Remove(RemoveKind::File), vec![backup_path]);
    csync.handle_event(&event);

    // Final state: file.txt should have modified content, backup should be gone
    assert_eq!(
        env.read_target_file("file.txt"),
        Some("modified".to_string())
    );
    assert!(!env.target_file_exists("file.txt~"));
}

#[test]
fn test_runtime_cache_expiry_moved_from() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("file.txt", "content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("file.txt"));

    // Simulate MOVED_FROM without matching MOVED_TO
    let file_path = env.source_path().join("file.txt");
    let mut event = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::From)),
        vec![file_path.clone()],
    );
    event.attrs.set_tracker(789);
    csync.handle_event(&event);

    // Delete the file from source (simulate incomplete move)
    fs::remove_file(&file_path).expect("Failed to delete file");

    // File should still exist in target (waiting in cache)
    assert!(env.target_file_exists("file.txt"));

    // Wait for cache to expire and check
    thread::sleep(Duration::from_millis(150));
    csync.check_cache().expect("Failed to check cache");

    // Now file should be deleted from target
    assert!(!env.target_file_exists("file.txt"));
}

#[test]
fn test_runtime_ignored_file_creation() {
    let env = TestEnv::new();

    // Create .gitignore
    env.create_gitignore(".", &["*.log", "temp_*"]);

    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Try to create ignored file
    let ignored_file = env.create_source_file("debug.log", "log content");
    let event = create_event(EventKind::Create(CreateKind::File), vec![ignored_file]);
    csync.handle_event(&event);

    // Verify ignored file was NOT synced
    assert!(!env.target_file_exists("debug.log"));

    // Create non-ignored file
    let normal_file = env.create_source_file("normal.txt", "content");
    let event = create_event(EventKind::Create(CreateKind::File), vec![normal_file]);
    csync.handle_event(&event);

    // Verify normal file WAS synced
    assert!(env.target_file_exists("normal.txt"));
}

#[test]
fn test_runtime_subdirectory_gitignore() {
    let env = TestEnv::new();

    // Initial sync with subdirectory .gitignore
    env.create_gitignore("subdir", &["*.o", "*.tmp"]);

    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Create ignored file in subdirectory
    let ignored_file = env.create_source_file("subdir/build.o", "object file");
    let event = create_event(EventKind::Create(CreateKind::File), vec![ignored_file]);
    csync.handle_event(&event);

    // Verify ignored file was NOT synced
    assert!(!env.target_file_exists("subdir/build.o"));

    // Create non-ignored file in subdirectory
    let normal_file = env.create_source_file("subdir/source.c", "source code");
    let event = create_event(EventKind::Create(CreateKind::File), vec![normal_file]);
    csync.handle_event(&event);

    // Verify normal file WAS synced
    assert!(env.target_file_exists("subdir/source.c"));
}

#[test]
fn test_runtime_multiple_rapid_modifications() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("file.txt", "v0");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    let file_path = env.source_path().join("file.txt");

    // Simulate rapid modifications
    for i in 1..=5 {
        let content = format!("v{}", i);
        fs::write(&file_path, &content).expect("Failed to write file");

        let event = create_event(
            EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            vec![file_path.clone()],
        );
        csync.handle_event(&event);

        // Each modification should be synced
        assert_eq!(env.read_target_file("file.txt"), Some(content));
    }
}

#[test]
fn test_runtime_nested_directory_creation() {
    let env = TestEnv::new();

    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Create nested directory structure
    let dirs = ["a", "a/b", "a/b/c", "a/b/c/d"];
    for dir in &dirs {
        let dir_path = env.source_path().join(dir);
        fs::create_dir_all(&dir_path).expect("Failed to create directory");
        let event = create_event(EventKind::Create(CreateKind::Folder), vec![dir_path]);
        csync.handle_event(&event);
    }

    // Create file in deepest directory
    let deep_file = env.create_source_file("a/b/c/d/file.txt", "deep content");
    let event = create_event(EventKind::Create(CreateKind::File), vec![deep_file]);
    csync.handle_event(&event);

    // Verify entire structure exists
    assert!(env.target_path().join("a").is_dir());
    assert!(env.target_path().join("a/b").is_dir());
    assert!(env.target_path().join("a/b/c").is_dir());
    assert!(env.target_path().join("a/b/c/d").is_dir());
    assert!(env.target_file_exists("a/b/c/d/file.txt"));
}

#[test]
fn test_runtime_batch_file_operations() {
    let env = TestEnv::new();

    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Create multiple files
    for i in 0..10 {
        let file = env.create_source_file(&format!("file{}.txt", i), &format!("content{}", i));
        let event = create_event(EventKind::Create(CreateKind::File), vec![file]);
        csync.handle_event(&event);
    }

    // Verify all files were synced
    for i in 0..10 {
        assert!(env.target_file_exists(&format!("file{}.txt", i)));
        assert_eq!(
            env.read_target_file(&format!("file{}.txt", i)),
            Some(format!("content{}", i))
        );
    }

    // Delete half of them
    for i in 0..5 {
        let file_path = env.source_path().join(format!("file{}.txt", i));
        fs::remove_file(&file_path).expect("Failed to delete file");
        let event = create_event(EventKind::Remove(RemoveKind::File), vec![file_path]);
        csync.handle_event(&event);
    }

    // Verify deletions
    for i in 0..5 {
        assert!(!env.target_file_exists(&format!("file{}.txt", i)));
    }
    for i in 5..10 {
        assert!(env.target_file_exists(&format!("file{}.txt", i)));
    }
}

#[test]
fn test_runtime_directory_rename() {
    let env = TestEnv::new();

    // Initial sync with directory
    env.create_source_file("old_dir/file1.txt", "content1");
    env.create_source_file("old_dir/file2.txt", "content2");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    assert!(env.target_file_exists("old_dir/file1.txt"));
    assert!(env.target_file_exists("old_dir/file2.txt"));

    // Rename directory
    let old_dir = env.source_path().join("old_dir");
    let new_dir = env.source_path().join("new_dir");
    fs::rename(&old_dir, &new_dir).expect("Failed to rename directory");

    // Simulate directory rename event
    let dir_event = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        vec![old_dir.clone(), new_dir.clone()],
    );
    csync.handle_event(&dir_event);

    // In real usage, the file watcher would also send events for files being moved.
    // Simulate those file move events:
    let file1_old = old_dir.join("file1.txt");
    let file1_new = new_dir.join("file1.txt");
    let file1_event = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        vec![file1_old, file1_new],
    );
    csync.handle_event(&file1_event);

    let file2_old = old_dir.join("file2.txt");
    let file2_new = new_dir.join("file2.txt");
    let file2_event = create_event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        vec![file2_old, file2_new],
    );
    csync.handle_event(&file2_event);

    // Verify old directory is gone and new directory exists with files
    assert!(!env.target_path().join("old_dir").exists());
    assert!(env.target_file_exists("new_dir/file1.txt"));
    assert!(env.target_file_exists("new_dir/file2.txt"));
    assert_eq!(
        env.read_target_file("new_dir/file1.txt"),
        Some("content1".to_string())
    );
}

#[test]
fn test_runtime_custom_ignore_patterns() {
    let env = TestEnv::new();

    // Use custom ignore patterns
    let mut csync = env.create_csync(&["*.bak".to_string(), "temp_*".to_string()], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Try to create files matching custom patterns
    let ignored1 = env.create_source_file("backup.bak", "backup");
    let event = create_event(EventKind::Create(CreateKind::File), vec![ignored1]);
    csync.handle_event(&event);

    let ignored2 = env.create_source_file("temp_work.txt", "temp");
    let event = create_event(EventKind::Create(CreateKind::File), vec![ignored2]);
    csync.handle_event(&event);

    // Create normal file
    let normal = env.create_source_file("normal.txt", "content");
    let event = create_event(EventKind::Create(CreateKind::File), vec![normal]);
    csync.handle_event(&event);

    // Verify only normal file was synced
    assert!(!env.target_file_exists("backup.bak"));
    assert!(!env.target_file_exists("temp_work.txt"));
    assert!(env.target_file_exists("normal.txt"));
}

#[test]
fn test_runtime_metadata_only_modification() {
    let env = TestEnv::new();

    // Initial sync
    env.create_source_file("file.txt", "content");
    let mut csync = env.create_csync(&[], true, false);
    csync.initial_sync(false).expect("Initial sync failed");

    // Change only metadata (e.g., permissions)
    let file_path = env.source_path().join("file.txt");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&file_path)
            .expect("Failed to get metadata")
            .permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file_path, perms).expect("Failed to set permissions");
    }

    // Simulate metadata change event
    let event = create_event(
        EventKind::Modify(ModifyKind::Metadata(
            notify::event::MetadataKind::Permissions,
        )),
        vec![file_path],
    );
    csync.handle_event(&event);

    // File should still exist and have same content
    assert!(env.target_file_exists("file.txt"));
    assert_eq!(
        env.read_target_file("file.txt"),
        Some("content".to_string())
    );

    // On Unix, verify permissions were synced
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let target_perms = fs::metadata(env.target_path().join("file.txt"))
            .expect("Failed to get target metadata")
            .permissions();
        let source_perms = fs::metadata(env.source_path().join("file.txt"))
            .expect("Failed to get source metadata")
            .permissions();
        assert_eq!(target_perms.mode() & 0o777, source_perms.mode() & 0o777);
    }
}
