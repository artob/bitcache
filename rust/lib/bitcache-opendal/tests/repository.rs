// This is free and unencumbered software released into the public domain.

use bitcache_core::{Bytes, Id, ListOptions, Repository, futures_util::TryStreamExt};
use bitcache_opendal::{DalRepository, OpenOptions};
use opendal::{Operator, services::Memory};
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn test_dal_repository() {
    let data = b"Hello, world!";

    let operator = Operator::new(Memory::default()).unwrap();
    let mut repository = DalRepository::new(operator);

    assert!(repository.is_empty().await.unwrap());

    let id = repository.put(Bytes::from_static(data)).await.unwrap();
    assert_eq!(id, Id::of(data));

    assert_eq!(repository.len().await.unwrap(), 1);
    assert!(!repository.is_empty().await.unwrap());
    assert!(repository.contains(&id).await.unwrap());
    assert_eq!(
        repository.get_len(&id).await.unwrap(),
        Some(data.len() as u64)
    );

    let blob = repository.get(&id).await.unwrap().unwrap();
    assert_eq!(blob.id(), &id);
    assert_eq!(blob.len(), data.len() as u64);

    let mut contents = Vec::new();
    let mut reader = blob.read();
    AsyncReadExt::read_to_end(&mut reader, &mut contents)
        .await
        .unwrap();
    assert_eq!(contents, data);

    let absent = Id::of(b"absent");
    assert!(!repository.contains(&absent).await.unwrap());
    assert!(repository.get(&absent).await.unwrap().is_none());
    assert!(repository.get_len(&absent).await.unwrap().is_none());

    // Removal:
    assert!(!repository.remove(&absent).await.unwrap());
    assert!(repository.remove(&id).await.unwrap());
    assert!(!repository.remove(&id).await.unwrap());
    assert!(!repository.contains(&id).await.unwrap());
    assert!(repository.is_empty().await.unwrap());

    // Clearing:
    repository.put(Bytes::from_static(b"foo")).await.unwrap();
    repository.put(Bytes::from_static(b"bar")).await.unwrap();
    assert_eq!(repository.len().await.unwrap(), 2);
    repository.clear().await.unwrap();
    assert!(repository.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_dal_repository_open() {
    let mut repository = DalRepository::open("memory://").unwrap();

    let id = repository.put(Bytes::from_static(b"opened")).await.unwrap();
    assert!(repository.contains(&id).await.unwrap());

    // Separately opened repositories are independent:
    let other = DalRepository::open("memory://").unwrap();
    assert!(!other.contains(&id).await.unwrap());

    // Unknown schemes are rejected:
    assert!(DalRepository::open("bogus://").is_err());
}

#[tokio::test]
async fn test_dal_repository_open_options() {
    // Service configuration options are passed through:
    let repository = DalRepository::open_options(
        "memory://",
        OpenOptions::new().with_option("root", "/prefix"),
    )
    .unwrap();
    assert!(repository.is_empty().await.unwrap());

    // Layers are applied: overriding the `delete` capability is observable
    // in the operator's reported capabilities.
    let layer = opendal::layers::CapabilityOverrideLayer::new(|mut capability| {
        capability.delete = false;
        capability
    });
    let repository =
        DalRepository::open_options("memory://", OpenOptions::new().with_layer(layer)).unwrap();
    assert!(!repository.operator().info().capability().delete);
}

#[tokio::test]
async fn test_dal_repository_from_operator() {
    let operator = Operator::new(Memory::default()).unwrap();
    let repository = DalRepository::from(operator);
    assert!(repository.is_empty().await.unwrap());
}

/// Repository futures are `Send`, so repositories can be moved into and
/// driven from spawned tasks on multithreaded executors.
#[tokio::test]
async fn test_dal_repository_is_spawnable() {
    let operator = Operator::new(Memory::default()).unwrap();
    let mut repository = DalRepository::new(operator);

    let id = tokio::spawn(async move { repository.put(Bytes::from_static(b"spawned")).await })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(id, Id::of(b"spawned"));
}

#[tokio::test]
async fn test_dal_repository_list() {
    let operator = Operator::new(Memory::default()).unwrap();
    let mut repository = DalRepository::new(operator);

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

    assert_eq!(repository.len().await.unwrap(), 10);

    // Full enumeration, in ascending ID order:
    let listed: Vec<Id> = repository
        .list(ListOptions::default())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(listed, ids);

    // Stable pagination via an exclusive ID cursor and a page-size limit:
    let mut paginated = Vec::new();
    let mut cursor: Option<Id> = None;
    loop {
        let mut options = ListOptions::new().with_limit(3);
        if let Some(cursor) = cursor.take() {
            options = options.with_after(cursor);
        }
        let page: Vec<Id> = repository.list(options).try_collect().await.unwrap();
        assert!(page.len() <= 3);
        let Some(last) = page.last() else { break };
        cursor = Some(last.clone());
        paginated.extend(page);
    }
    assert_eq!(paginated, ids);

    // Prefix filtering (on the hexadecimal encoding):
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

    // Prefix and cursor combined:
    let filtered: Vec<Id> = repository
        .list(
            ListOptions::new()
                .with_prefix(prefix)
                .with_after(expected[0].clone()),
        )
        .try_collect()
        .await
        .unwrap();
    assert_eq!(filtered, expected[1..]);
}
