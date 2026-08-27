// This is free and unencumbered software released into the public domain.

use bitcache_core::{
    Bytes, Id, ListOptions, ListOrder, Repository,
    futures_util::{AsyncReadExt, TryStreamExt},
};
use bitcache_turso::TursoRepository;
use core::time::Duration;
use turso::Builder;

#[tokio::test]
async fn test_turso_repository() {
    let data = b"Hello, world!";
    let mut repository = TursoRepository::open(":memory:").await.unwrap();

    assert!(repository.is_empty().await.unwrap());

    let id = repository.put(Bytes::from_static(data)).await.unwrap();
    assert_eq!(id, Id::of(data));
    assert_eq!(repository.len().await.unwrap(), 1);
    assert!(repository.contains(&id).await.unwrap());
    assert_eq!(
        repository.get_len(&id).await.unwrap(),
        Some(data.len() as u64)
    );

    let blob = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(blob.id(), &id);
    assert_eq!(blob.len(), data.len() as u64);
    assert!(blob.metadata().created_nanos().is_some());
    assert!(blob.metadata().accessed_nanos().is_some());

    let mut contents = Vec::new();
    blob.read().read_to_end(&mut contents).await.unwrap();
    assert_eq!(contents, data);

    let absent = Id::of(b"absent");
    assert!(!repository.contains(&absent).await.unwrap());
    assert!(repository.get(&absent).await.unwrap().is_none());
    assert_eq!(repository.get_len(&absent).await.unwrap(), None);
    assert!(!repository.remove(&absent).await.unwrap());

    assert!(repository.remove(&id).await.unwrap());
    assert!(!repository.remove(&id).await.unwrap());
    assert!(repository.is_empty().await.unwrap());

    repository.put(Bytes::from_static(b"foo")).await.unwrap();
    repository.put(Bytes::from_static(b"bar")).await.unwrap();
    repository.clear().await.unwrap();
    assert!(repository.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_turso_repository_list() {
    let mut repository = TursoRepository::open(":memory:").await.unwrap();
    let mut ids = Vec::new();
    for n in 0u32..10 {
        ids.push(
            repository
                .put(Bytes::from(n.to_le_bytes().to_vec()))
                .await
                .unwrap(),
        );
    }
    ids.sort_unstable();

    let listed: Vec<Id> = repository
        .list(ListOptions::default())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(listed, ids);

    let page: Vec<Id> = repository
        .list(ListOptions::new().with_after(ids[2].clone()).with_limit(4))
        .try_collect()
        .await
        .unwrap();
    assert_eq!(page, ids[3..7]);

    let descending: Vec<Id> = repository
        .list(ListOptions::new().with_order(ListOrder::Descending))
        .try_collect()
        .await
        .unwrap();
    assert_eq!(descending, ids.iter().rev().cloned().collect::<Vec<_>>());

    let hex = ids[3].to_hex();
    let prefix = &hex.as_str()[..2];
    let expected: Vec<Id> = ids
        .iter()
        .filter(|id| id.to_hex().starts_with(prefix))
        .cloned()
        .collect();
    let filtered: Vec<Id> = repository
        .list(ListOptions::new().with_prefix(prefix))
        .try_collect()
        .await
        .unwrap();
    assert_eq!(filtered, expected);
}

#[tokio::test]
async fn test_turso_repository_expiry() {
    let mut repository = TursoRepository::open(":memory:").await.unwrap();
    let id = repository
        .put_with_ttl(
            Bytes::from_static(b"temporary"),
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

    assert!(repository.contains(&id).await.unwrap());
    assert!(
        repository
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .metadata()
            .expires_nanos()
            .is_some()
    );

    assert!(repository.set_expiry(&id, Some(0)).await.unwrap());
    assert!(!repository.contains(&id).await.unwrap());
    assert_eq!(repository.get_len(&id).await.unwrap(), None);
    assert!(
        repository
            .list(ListOptions::default())
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(repository.len().await.unwrap(), 0);
}

#[tokio::test]
async fn test_turso_repository_reuses_existing_schema() {
    let database = Builder::new_local(":memory:").build().await.unwrap();
    let mut repository = TursoRepository::new(database.clone()).await.unwrap();
    let id = repository
        .put(Bytes::from_static(b"persistent"))
        .await
        .unwrap();

    let reopened = TursoRepository::new(database).await.unwrap();
    assert!(reopened.contains(&id).await.unwrap());
}

#[tokio::test]
async fn test_turso_repository_default_ttl() {
    let database = Builder::new_local(":memory:").build().await.unwrap();
    let repository = TursoRepository::new(database)
        .await
        .unwrap()
        .with_ttl(Duration::from_secs(30));
    assert_eq!(repository.ttl(), Some(Duration::from_secs(30)));
}

#[tokio::test]
async fn test_turso_repository_rejects_unknown_url_scheme() {
    assert!(TursoRepository::open("bogus://database").await.is_err());
}
