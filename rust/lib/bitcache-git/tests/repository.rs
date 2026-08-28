// This is free and unencumbered software released into the public domain.

use bitcache_core::{
    Bytes, Id, ListOptions, Repository, RepositoryError, futures_util::TryStreamExt,
};
use bitcache_git::GitRepository;

const FIRST_ID: &str = "008d7148c9eba29a11e9265707ff6121f0d73297bf34be54a5337a223c62c16e";

fn repository() -> GitRepository {
    // See: https://github.com/asimov-datasets/gutenberg.org
    GitRepository::github("asimov-datasets", "gutenberg.org", "master")
}

#[tokio::test]
#[ignore = "requires network access to api.github.com"]
async fn test_github_repository() {
    let mut repository = repository();
    let ids: Vec<Id> = repository
        .list(ListOptions::default())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids.len(), 10);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));

    let first_id = Id::from_hex(FIRST_ID).unwrap();
    assert!(repository.contains(&first_id).await.unwrap());
    assert_eq!(repository.get_len(&first_id).await.unwrap(), Some(48_326));

    let blob = repository.get(&first_id).await.unwrap().unwrap();
    assert_eq!(blob.id(), &first_id);
    assert_eq!(blob.len(), 48_326);
    assert_eq!(Id::of(blob.read().into_bytes()), first_id);

    let absent = Id::of(b"absent");
    assert!(!repository.contains(&absent).await.unwrap());
    assert!(repository.get(&absent).await.unwrap().is_none());
    assert_eq!(repository.get_len(&absent).await.unwrap(), None);

    assert!(matches!(
        repository.put(Bytes::from_static(b"data")).await,
        Err(RepositoryError::UnsupportedOperation)
    ));
    assert!(matches!(
        repository.remove(&first_id).await,
        Err(RepositoryError::UnsupportedOperation)
    ));
    assert!(matches!(
        repository.clear().await,
        Err(RepositoryError::UnsupportedOperation)
    ));
}
