// This is free and unencumbered software released into the public domain.

use bitcache_core::{Bytes, Id, ListOptions, Repository};
use bitcache_heap::HeapRepository;
use futures::{io::AsyncReadExt, stream::TryStreamExt};

/// Exercises the repository on a non-Tokio executor, since the `Repository`
/// trait and `Blob::read` are runtime-agnostic.
#[test]
fn test_heap_repository() {
    futures::executor::block_on(async {
        let data = b"Hello, world!";
        let mut repository = HeapRepository::new();

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
        let created = blob.metadata().created_nanos().unwrap();
        let updated = blob.metadata().updated_nanos().unwrap();
        assert_eq!(updated, created);
        assert_eq!(blob.metadata().accessed_nanos(), None);

        std::thread::sleep(std::time::Duration::from_millis(1));
        repository.put(Bytes::from_static(data)).await.unwrap();
        let reinserted = repository.get(&id).await.unwrap().unwrap();
        assert_eq!(reinserted.metadata().created_nanos(), Some(created));
        assert!(reinserted.metadata().updated_nanos().unwrap() > updated);

        let mut contents = Vec::new();
        let mut reader = blob.read();
        AsyncReadExt::read_to_end(&mut reader, &mut contents)
            .await
            .unwrap();
        assert_eq!(contents, data);

        let absent = Id::of(b"absent");
        assert!(!repository.contains(&absent).await.unwrap());
        assert!(repository.get(&absent).await.unwrap().is_none());

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
    });
}

#[test]
fn test_heap_repository_list() {
    futures::executor::block_on(async {
        let mut repository = HeapRepository::new();

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

        // Enumeration after an exclusive ID cursor:
        let listed: Vec<Id> = repository
            .list(ListOptions::new().with_after(ids[6].clone()))
            .try_collect()
            .await
            .unwrap();
        assert_eq!(listed, ids[7..]);

        // Enumeration limited to a page size:
        let listed: Vec<Id> = repository
            .list(ListOptions::new().with_limit(4))
            .try_collect()
            .await
            .unwrap();
        assert_eq!(listed, ids[..4]);

        // Cursor and limit combined:
        let listed: Vec<Id> = repository
            .list(ListOptions::new().with_after(ids[2].clone()).with_limit(4))
            .try_collect()
            .await
            .unwrap();
        assert_eq!(listed, ids[3..7]);

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
    });
}
