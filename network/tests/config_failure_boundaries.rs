use rlvgl_network::{
    ConfigOrigin, NetworkConfig, NetworkConfigStore, WifiCredentials, load_or_seed,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorageFailure {
    Load,
    Store,
}

#[derive(Default)]
struct AtomicStore {
    saved: Option<NetworkConfig>,
    fail_load: bool,
    fail_store: bool,
    loads: usize,
    writes: usize,
}

impl NetworkConfigStore for AtomicStore {
    type Error = StorageFailure;

    fn load(&mut self) -> Result<Option<NetworkConfig>, Self::Error> {
        self.loads += 1;
        if self.fail_load {
            Err(StorageFailure::Load)
        } else {
            Ok(self.saved.clone())
        }
    }

    fn store(&mut self, config: &NetworkConfig) -> Result<(), Self::Error> {
        self.writes += 1;
        if self.fail_store {
            Err(StorageFailure::Store)
        } else {
            self.saved = Some(config.clone());
            Ok(())
        }
    }
}

fn credentials(name: &str) -> WifiCredentials {
    WifiCredentials::new(name, "test-only-password").unwrap()
}

#[test]
fn omitted_seed_never_attempts_a_write_even_when_storage_writes_would_fail() {
    for saved in [None, Some(NetworkConfig::new(7, credentials("saved")))] {
        let mut store = AtomicStore {
            saved: saved.clone(),
            fail_store: true,
            ..Default::default()
        };
        let resolved = load_or_seed(&mut store, None).unwrap();
        assert_eq!(resolved.as_ref().map(|r| &r.config), saved.as_ref());
        assert_eq!(
            resolved.map(|r| r.origin),
            saved.map(|_| ConfigOrigin::Stored)
        );
        assert_eq!((store.loads, store.writes), (1, 0));
    }
}

#[test]
fn load_failure_cannot_be_treated_as_unprovisioned_or_overwritten_by_seed() {
    let saved = NetworkConfig::new(7, credentials("saved"));
    let mut store = AtomicStore {
        saved: Some(saved.clone()),
        fail_load: true,
        ..Default::default()
    };
    assert_eq!(
        load_or_seed(&mut store, Some(credentials("replacement"))),
        Err(StorageFailure::Load)
    );
    assert_eq!((store.loads, store.writes), (1, 0));
    assert_eq!(store.saved, Some(saved));
}

#[test]
fn failed_atomic_persistence_cannot_return_a_seeded_or_updated_configuration() {
    for saved in [None, Some(NetworkConfig::new(7, credentials("saved")))] {
        let mut store = AtomicStore {
            saved: saved.clone(),
            fail_store: true,
            ..Default::default()
        };
        assert_eq!(
            load_or_seed(&mut store, Some(credentials("replacement"))),
            Err(StorageFailure::Store)
        );
        assert_eq!((store.loads, store.writes), (1, 1));
        assert_eq!(store.saved, saved);
    }
}
