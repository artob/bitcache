// This is free and unencumbered software released into the public domain.

use bitcache_core::{Hasher, Id, Repository};
use bitcache_opendal::DalRepository;
use opendal::{Operator, services::Memory};

fn hash(data: &[u8]) -> Id {
    Id::from(Hasher::new().update(data).finalize())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dal_repository() {
    let data = b"Hello, world!";
    let id = hash(data);

    let operator = Operator::new(Memory::default()).unwrap();
    operator
        .write(id.to_hex().as_str(), data.as_slice())
        .await
        .unwrap();

    let repository = DalRepository::new(operator).unwrap();
    tokio::task::spawn_blocking(move || {
        assert_eq!(repository.len(), 1);
        assert!(!repository.is_empty());
        assert!(repository.contains(&id));
        assert_eq!(repository.get_len(&id), Some(data.len() as u64));
        assert_eq!(repository.get(&id).unwrap().len(), data.len() as u64);

        let absent = hash(b"absent");
        assert!(!repository.contains(&absent));
        assert!(repository.get(&absent).is_none());
        assert!(repository.get_len(&absent).is_none());
    })
    .await
    .unwrap();
}
