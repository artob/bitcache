// This is free and unencumbered software released into the public domain.

#![cfg(unix)]

use bitcache_core::{
    Bytes, Id, ListOptions, PutOptions, Repository, RepositoryError, futures_util::StreamExt,
};
use bitcache_fs::FsRepository;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};
use tokio::io::AsyncReadExt;

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
    repository_path.join(format!("{}.xz", id.to_hex().as_str()))
}

fn uncompressed_blob_path(repository_path: &Path, id: &Id) -> PathBuf {
    repository_path.join(id.to_hex().as_str())
}

async fn read_blob_file(repository: &FsRepository, id: &Id) -> Vec<u8> {
    let mut file = repository.get_file(id).await.unwrap().unwrap();
    let mut data = Vec::new();
    file.read_to_end(&mut data).await.unwrap();
    data
}

fn nanos_since_epoch(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn now_nanos() -> u64 {
    nanos_since_epoch(SystemTime::now())
}

fn ctime_nanos(metadata: &fs::Metadata) -> i128 {
    i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec())
}

fn xattr_timestamp(path: &Path, name: &str) -> u64 {
    let value = xattr::get(path, name).unwrap().unwrap();
    u64::from_be_bytes(value.try_into().unwrap())
}

fn with_writable_file(path: &Path, operation: impl FnOnce()) {
    let original = fs::metadata(path).unwrap().permissions();
    let mut writable = original.clone();
    writable.set_mode(original.mode() | 0o200);
    fs::set_permissions(path, writable).unwrap();
    operation();
    fs::set_permissions(path, original).unwrap();
}

fn set_xattr(path: &Path, name: &str, value: &[u8]) {
    with_writable_file(path, || xattr::set(path, name, value).unwrap());
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
    let metadata_capabilities = repository.capabilities().blob_metadata();
    assert!(metadata_capabilities.created());
    assert!(metadata_capabilities.updated());
    assert!(metadata_capabilities.accessed());
    assert!(metadata_capabilities.expires());
    assert!(metadata_capabilities.media_type());

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
    assert_eq!(blob.metadata().len(), memory_data.len() as u64);
    assert_eq!(
        blob.metadata().created_nanos(),
        metadata_after_get.created().ok().map(nanos_since_epoch)
    );
    assert_eq!(
        blob.metadata().updated_nanos(),
        u64::try_from(ctime_nanos(&metadata_after_get)).ok()
    );
    assert_eq!(
        xattr::get(&memory_path, "user.bitcache.updated").unwrap(),
        None
    );
    assert_eq!(
        blob.metadata().accessed_nanos(),
        Some(nanos_since_epoch(metadata_after_get.accessed().unwrap()))
    );

    // Re-store the same blob through put_file. Its inode and all file metadata
    // except ctime must remain unchanged, while logical updated advances.
    let memory_input = temp_dir.path().join("memory-input");
    fs::write(&memory_input, memory_data).unwrap();
    let before_updated = blob.metadata().updated_nanos().unwrap();
    let before_memory_duplicate = fs::metadata(&memory_path).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(repository.put_file(&memory_input).await.unwrap(), memory_id);
    let after_memory_duplicate = fs::metadata(&memory_path).unwrap();
    assert_same_file_except_ctime(&before_memory_duplicate, &after_memory_duplicate);
    let blob = repository.get(&memory_id).await.unwrap().unwrap();
    assert!(blob.metadata().updated_nanos().unwrap() > before_updated);
    assert_eq!(read_blob_file(&repository, &memory_id).await, memory_data);

    // Store a file and verify that the inverse duplicate path has the same
    // no-replacement behavior.
    let file_data = b"stored from a file";
    let file_input = temp_dir.path().join("file-input");
    fs::write(&file_input, file_data).unwrap();
    let file_id = repository.put_file(&file_input).await.unwrap();
    let file_path = blob_path(&repository_path, &file_id);
    let before_updated = repository
        .get(&file_id)
        .await
        .unwrap()
        .unwrap()
        .metadata()
        .updated_nanos()
        .unwrap();
    let before_file_duplicate = fs::metadata(&file_path).unwrap();
    assert_eq!(before_file_duplicate.permissions().mode() & 0o777, 0o444);

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        repository.put(Bytes::from_static(file_data)).await.unwrap(),
        file_id
    );
    let after_file_duplicate = fs::metadata(&file_path).unwrap();
    assert_same_file_except_ctime(&before_file_duplicate, &after_file_duplicate);
    let blob = repository.get(&file_id).await.unwrap().unwrap();
    assert!(blob.metadata().updated_nanos().unwrap() > before_updated);
    assert_eq!(read_blob_file(&repository, &file_id).await, file_data);

    assert!(fs::read_dir(&repository_path).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".put-")
    }));
}

#[tokio::test]
async fn test_compressed_storage_listing_and_uncompressed_preference() {
    let temp_dir = TestDir::new();
    let repository_path = temp_dir.path().join("repository");
    let mut repository = FsRepository::create(repository_path.to_str().unwrap()).unwrap();
    let data = b"the compressed representation";
    let id = repository.put(Bytes::from_static(data)).await.unwrap();
    let compressed_path = blob_path(&repository_path, &id);
    let uncompressed_path = uncompressed_blob_path(&repository_path, &id);

    assert!(compressed_path.exists());
    assert!(!uncompressed_path.exists());
    assert!(
        fs::read(&compressed_path)
            .unwrap()
            .starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0])
    );
    assert_eq!(
        repository.get_len(&id).await.unwrap(),
        Some(data.len() as u64)
    );
    assert_eq!(read_blob_file(&repository, &id).await, data);
    assert_eq!(
        repository
            .list(ListOptions::default())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec![id.clone()]
    );

    fs::write(&uncompressed_path, b"uncompressed wins").unwrap();
    assert_eq!(read_blob_file(&repository, &id).await, b"uncompressed wins");
    assert_eq!(
        repository
            .list(ListOptions::default())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec![id.clone()]
    );

    fs::remove_file(&uncompressed_path).unwrap();
    assert_eq!(read_blob_file(&repository, &id).await, data);

    fs::write(&uncompressed_path, data).unwrap();
    assert!(repository.remove(&id).await.unwrap());
    assert!(!uncompressed_path.exists());
    assert!(!compressed_path.exists());
}

#[tokio::test]
async fn test_fs_repository_expiry_and_media_type_round_trip() {
    let temp_dir = TestDir::new();
    let repository_path = temp_dir.path().join("repository");
    let mut repository = FsRepository::create(repository_path.to_str().unwrap()).unwrap();
    let options = PutOptions::new()
        .with_ttl(Duration::from_secs(60))
        .with_media_type(Some("text/plain".into()));

    let id = repository
        .put_with_options(Bytes::from_static(b"metadata"), options)
        .await
        .unwrap();
    let path = blob_path(&repository_path, &id);
    let blob = repository.get(&id).await.unwrap().unwrap();
    let original_updated = blob.metadata().updated_nanos().unwrap();
    assert_eq!(blob.metadata().media_type(), Some("text/plain"));
    assert!(blob.metadata().expires_nanos().unwrap() > now_nanos());
    assert_eq!(
        xattr_timestamp(&path, "user.bitcache.expires"),
        blob.metadata().expires_nanos().unwrap()
    );
    assert_eq!(
        xattr::get(&path, "user.bitcache.media-type")
            .unwrap()
            .unwrap(),
        b"text/plain"
    );

    std::thread::sleep(Duration::from_millis(20));
    assert!(
        repository
            .set_media_type(&id, Some("text/markdown"))
            .await
            .unwrap()
    );
    assert!(repository.set_expiry(&id, None).await.unwrap());
    let blob = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(blob.metadata().media_type(), Some("text/markdown"));
    assert_eq!(blob.metadata().expires_nanos(), None);
    assert!(blob.metadata().updated_nanos().unwrap() > original_updated);
    assert_eq!(xattr::get(&path, "user.bitcache.expires").unwrap(), None);

    assert!(repository.set_media_type(&id, None).await.unwrap());
    assert_eq!(
        repository
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .metadata()
            .media_type(),
        None
    );
    assert_eq!(xattr::get(&path, "user.bitcache.media-type").unwrap(), None);
    assert_eq!(xattr::get(&path, "user.bitcache.updated").unwrap(), None);
}

#[tokio::test]
async fn test_expired_blobs_are_absent_but_clear_removes_them() {
    let temp_dir = TestDir::new();
    let repository_path = temp_dir.path().join("repository");
    let mut repository = FsRepository::create(repository_path.to_str().unwrap()).unwrap();
    let id = repository
        .put(Bytes::from_static(b"expired"))
        .await
        .unwrap();
    let path = blob_path(&repository_path, &id);

    assert!(repository.set_expiry(&id, Some(0)).await.unwrap());
    assert!(!repository.contains(&id).await.unwrap());
    assert!(repository.get(&id).await.unwrap().is_none());
    assert!(repository.get_file(&id).await.unwrap().is_none());
    assert!(repository.get_len(&id).await.unwrap().is_none());
    assert!(repository.is_empty().await.unwrap());
    assert_eq!(repository.len().await.unwrap(), 0);
    assert!(
        repository
            .list(ListOptions::default())
            .collect::<Vec<_>>()
            .await
            .is_empty()
    );
    assert!(!repository.remove(&id).await.unwrap());
    assert!(
        !repository
            .set_media_type(&id, Some("text/plain"))
            .await
            .unwrap()
    );
    assert!(path.exists());

    repository.clear().await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn test_reinsertion_revives_expired_blob_without_replacing_it() {
    let temp_dir = TestDir::new();
    let repository_path = temp_dir.path().join("repository");
    let mut repository = FsRepository::create(repository_path.to_str().unwrap()).unwrap();
    let data = Bytes::from_static(b"revive me");
    let id = repository.put(data.clone()).await.unwrap();
    let path = blob_path(&repository_path, &id);
    let before = fs::metadata(&path).unwrap();
    let before_updated = repository
        .get(&id)
        .await
        .unwrap()
        .unwrap()
        .metadata()
        .updated_nanos()
        .unwrap();
    assert!(repository.set_expiry(&id, Some(0)).await.unwrap());

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(repository.put(data).await.unwrap(), id);
    let after = fs::metadata(&path).unwrap();
    let blob = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(after.dev(), before.dev());
    assert_eq!(after.ino(), before.ino());
    assert_eq!(blob.metadata().expires_nanos(), None);
    assert!(blob.metadata().updated_nanos().unwrap() > before_updated);
}

#[tokio::test]
async fn test_malformed_extended_metadata_is_rejected() {
    let temp_dir = TestDir::new();
    let repository_path = temp_dir.path().join("repository");
    let mut repository = FsRepository::create(repository_path.to_str().unwrap()).unwrap();
    let id = repository
        .put(Bytes::from_static(b"malformed"))
        .await
        .unwrap();
    let path = blob_path(&repository_path, &id);

    set_xattr(&path, "user.bitcache.expires", b"bad");
    let error = repository.get(&id).await.unwrap_err();
    assert!(matches!(
        error,
        RepositoryError::Io(ref error) if error.kind() == std::io::ErrorKind::InvalidData
    ));

    set_xattr(&path, "user.bitcache.expires", &[]);
    set_xattr(&path, "user.bitcache.media-type", &[0xff]);
    let error = repository.get(&id).await.unwrap_err();
    assert!(matches!(
        error,
        RepositoryError::Io(ref error) if error.kind() == std::io::ErrorKind::InvalidData
    ));
}
