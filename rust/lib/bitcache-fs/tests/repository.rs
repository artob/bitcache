// This is free and unencumbered software released into the public domain.

#![cfg(unix)]

use bitcache_core::{Bytes, Id, Repository};
use bitcache_fs::FsRepository;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bitcache-fs-test-{}-{}",
            std::process::id(),
            sequence
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn blob_path(repository_path: &Path, id: &Id) -> PathBuf {
    repository_path.join(id.to_hex().as_str())
}

fn nanos_since_epoch(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn ctime_nanos(metadata: &fs::Metadata) -> i128 {
    i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec())
}

fn assert_same_file_except_ctime(before: &fs::Metadata, after: &fs::Metadata) {
    assert_eq!(after.dev(), before.dev());
    assert_eq!(after.ino(), before.ino());
    assert_eq!(after.len(), before.len());
    assert_eq!(after.permissions().mode(), before.permissions().mode());
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    assert_eq!(after.accessed().unwrap(), before.accessed().unwrap());
    assert_eq!(after.created().ok(), before.created().ok());
    assert!(ctime_nanos(after) > ctime_nanos(before));
}

#[tokio::test]
async fn test_fs_repository_metadata_permissions_and_duplicate_storage() {
    let temp_dir = TestDir::new();
    let repository_path = temp_dir.path().join("repository");
    let mut repository = FsRepository::create(repository_path.to_str().unwrap()).unwrap();

    // Store in-memory data and verify its filesystem-derived metadata.
    let memory_data = b"stored from memory";
    let memory_id = repository
        .put(Bytes::from_static(memory_data))
        .await
        .unwrap();
    let memory_path = blob_path(&repository_path, &memory_id);
    let memory_metadata = fs::metadata(&memory_path).unwrap();
    assert_eq!(memory_metadata.permissions().mode() & 0o777, 0o444);

    let blob = repository.get(&memory_id).await.unwrap().unwrap();
    let metadata_after_get = fs::metadata(&memory_path).unwrap();
    assert_eq!(blob.metadata().len(), memory_metadata.len());
    assert_eq!(
        blob.metadata().created_nanos(),
        Some(nanos_since_epoch(
            metadata_after_get
                .created()
                .or_else(|_| metadata_after_get.modified())
                .unwrap(),
        ))
    );
    assert_eq!(
        blob.metadata().accessed_nanos(),
        Some(nanos_since_epoch(metadata_after_get.accessed().unwrap()))
    );

    // Re-store the same blob through put_file. Its inode and all file metadata
    // except ctime must remain unchanged.
    let memory_input = temp_dir.path().join("memory-input");
    fs::write(&memory_input, memory_data).unwrap();
    let before_memory_duplicate = fs::metadata(&memory_path).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(repository.put_file(&memory_input).await.unwrap(), memory_id);
    let after_memory_duplicate = fs::metadata(&memory_path).unwrap();
    assert_same_file_except_ctime(&before_memory_duplicate, &after_memory_duplicate);
    assert_eq!(fs::read(&memory_path).unwrap(), memory_data);

    // Store a file and verify that the inverse duplicate path has the same
    // no-replacement behavior.
    let file_data = b"stored from a file";
    let file_input = temp_dir.path().join("file-input");
    fs::write(&file_input, file_data).unwrap();
    let file_id = repository.put_file(&file_input).await.unwrap();
    let file_path = blob_path(&repository_path, &file_id);
    let before_file_duplicate = fs::metadata(&file_path).unwrap();
    assert_eq!(before_file_duplicate.permissions().mode() & 0o777, 0o444);

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        repository.put(Bytes::from_static(file_data)).await.unwrap(),
        file_id
    );
    let after_file_duplicate = fs::metadata(&file_path).unwrap();
    assert_same_file_except_ctime(&before_file_duplicate, &after_file_duplicate);
    assert_eq!(fs::read(&file_path).unwrap(), file_data);

    assert!(fs::read_dir(&repository_path).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".put-")
    }));
}
