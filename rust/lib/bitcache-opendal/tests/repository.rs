// This is free and unencumbered software released into the public domain.

use bitcache_core::{Bytes, Id, Repository};
use bitcache_opendal::DalRepository;
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
}
