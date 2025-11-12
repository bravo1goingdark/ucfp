use crate::IndexError;
use std::sync::RwLock;

pub trait IndexBackend: Send + Sync {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError>;
    fn delete(&self, key: &str) -> Result<(), IndexError>;
    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError>;
    fn scan(
        &self,
        visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
    ) -> Result<(), IndexError>;
    fn flush(&self) -> Result<(), IndexError> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum BackendConfig {
    RocksDb { path: String },
    InMemory,
}

impl BackendConfig {
    pub fn rocksdb<P: Into<String>>(path: P) -> Self {
        BackendConfig::RocksDb { path: path.into() }
    }

    pub fn in_memory() -> Self {
        BackendConfig::InMemory
    }

    pub fn build(&self) -> Result<Box<dyn IndexBackend>, IndexError> {
        match self {
            BackendConfig::InMemory => Ok(Box::new(InMemoryBackend::new())),
            BackendConfig::RocksDb { path } => {
                #[cfg(feature = "backend-rocksdb")]
                {
                    Ok(Box::new(RocksDbBackend::open(path)?))
                }
                #[cfg(not(feature = "backend-rocksdb"))]
                {
                    let _ = path;
                    Err(IndexError::backend(
                        "rocksdb backend disabled at compile time",
                    ))
                }
            }
        }
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        BackendConfig::RocksDb {
            path: "data/ufp_index".into(),
        }
    }
}

pub struct InMemoryBackend {
    records: RwLock<std::collections::HashMap<String, Vec<u8>>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexBackend for InMemoryBackend {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
        self.records
            .write()
            .map_err(|_| IndexError::backend("poisoned lock"))?
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
        let guard = self
            .records
            .read()
            .map_err(|_| IndexError::backend("poisoned lock"))?;
        Ok(guard.get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<(), IndexError> {
        self.records
            .write()
            .map_err(|_| IndexError::backend("poisoned lock"))?
            .remove(key);
        Ok(())
    }

    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
        let mut guard = self
            .records
            .write()
            .map_err(|_| IndexError::backend("poisoned lock"))?;
        for (key, value) in entries {
            guard.insert(key, value);
        }
        Ok(())
    }

    fn scan(
        &self,
        visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
    ) -> Result<(), IndexError> {
        let guard = self
            .records
            .read()
            .map_err(|_| IndexError::backend("poisoned lock"))?;
        for value in guard.values() {
            visitor(value)?;
        }
        Ok(())
    }
}

#[cfg(feature = "backend-rocksdb")]
mod rocksdb_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use rocksdb::{IteratorMode, Options, WriteBatch, DB};

    pub struct RocksDbBackend {
        db: DB,
    }

    impl RocksDbBackend {
        pub fn open(path: &str) -> Result<Self, IndexError> {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            let db = DB::open(&opts, path).map_err(IndexError::backend)?;
            Ok(Self { db })
        }
    }

    impl IndexBackend for RocksDbBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            self.db.put(key, value).map_err(IndexError::backend)
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            self.db.get(key).map_err(IndexError::backend)
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            self.db.delete(key).map_err(IndexError::backend)
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            let mut batch = WriteBatch::default();
            for (key, value) in entries {
                batch.put(key, value);
            }
            self.db.write(batch).map_err(IndexError::backend)
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            for item in self.db.iterator(IteratorMode::Start) {
                let (_, value) = item.map_err(IndexError::backend)?;
                visitor(&value)?;
            }
            Ok(())
        }

        fn flush(&self) -> Result<(), IndexError> {
            self.db.flush().map_err(IndexError::backend)
        }
    }
}

#[cfg(feature = "backend-rocksdb")]
pub use rocksdb_backend::RocksDbBackend;
