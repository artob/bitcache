// This is free and unencumbered software released into the public domain.

use bitcache_core::{
    Bytes, Id, ListOptions, ListOrder, PutOptions, Repository,
    futures_util::{AsyncReadExt, TryStreamExt},
};
use bitcache_turso::TursoRepository;
use core::time::Duration;
use turso::{Builder, Value};

#[tokio::test]
async fn test_turso_repository() {
    let data = b"Hello, world!";
    let mut repository = TursoRepository::open(":memory:").await.unwrap();

    assert!(repository.is_empty().await.unwrap());

    let id = repository.put(Bytes::from_static(data)).await.unwrap();
    assert_eq!(id, Id::of(data));
    let connection = repository.database().connect().unwrap();
    let mut rows = connection
        .query("SELECT accessed FROM bitcache_meta", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert!(matches!(row.get_value(0).unwrap(), Value::Null));
    drop(rows);
    assert_eq!(repository.len().await.unwrap(), 1);
    assert!(repository.contains(&id).await.unwrap());
    assert_eq!(
        repository.get_len(&id).await.unwrap(),
        Some(data.len() as u64)
    );

    let blob = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(blob.id(), &id);
    assert_eq!(blob.len(), data.len() as u64);
    let created = blob.metadata().created_nanos().unwrap();
    let updated = blob.metadata().updated_nanos().unwrap();
    let accessed = blob.metadata().accessed_nanos().unwrap();
    assert_eq!(updated, created);
    assert!(created > 1_000_000_000_000_000_000);
    assert!(accessed >= updated);

    std::thread::sleep(Duration::from_millis(5));
    repository.put(Bytes::from_static(data)).await.unwrap();
    let reinserted = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(reinserted.metadata().created_nanos(), Some(created));
    assert!(reinserted.metadata().updated_nanos().unwrap() > updated);
    assert!(reinserted.metadata().accessed_nanos().unwrap() >= accessed);

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
    let blob = repository.get(&id).await.unwrap().unwrap();
    let updated = blob.metadata().updated_nanos().unwrap();
    let expires = blob.metadata().expires_nanos().unwrap();

    std::thread::sleep(Duration::from_millis(5));
    assert!(
        repository
            .set_expiry(&id, Some(expires.saturating_add(60_000_000_000)))
            .await
            .unwrap()
    );
    let blob = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(blob.metadata().updated_nanos(), Some(updated));

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
async fn test_turso_repository_media_type() {
    let mut repository = TursoRepository::open(":memory:").await.unwrap();
    let id = repository
        .put_with_options(
            Bytes::from_static(b"media type"),
            PutOptions::new().with_media_type(Some("text/plain".into())),
        )
        .await
        .unwrap();

    assert_eq!(
        repository
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .metadata()
            .media_type(),
        Some("text/plain")
    );
    assert!(
        repository
            .set_media_type(&id, Some("text/markdown"))
            .await
            .unwrap()
    );
    assert_eq!(
        repository
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .metadata()
            .media_type(),
        Some("text/markdown")
    );
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
