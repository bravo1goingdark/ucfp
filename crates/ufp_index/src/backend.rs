use crate::IndexError;
use std::sync::RwLock;

/// Trait for a key-value storage backend for the index.
/// This allows for different storage implementations (e.g., in-memory, RocksDB).
pub trait IndexBackend: Send + Sync {
    /// Insert or update a key-value pair.
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError>;
    /// Retrieve a value by key.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError>;
    /// Delete a key-value pair.
    fn delete(&self, key: &str) -> Result<(), IndexError>;
    /// Insert or update multiple key-value pairs in a batch.
    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError>;
    /// Scan all values in the backend, calling the visitor for each one.
    fn scan(
        &self,
        visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
    ) -> Result<(), IndexError>;
    /// Flush any buffered writes to the backend.
    fn flush(&self) -> Result<(), IndexError> {
        Ok(())
    }
}

/// Configuration for selecting and building a backend.
#[derive(Clone, Debug)]
pub enum BackendConfig {
    /// Use RocksDB for storage. The `path` is the directory where the database will be stored.
    RocksDb { path: String },
    /// Use an in-memory HashMap for storage. This is useful for testing.
    InMemory,
}

impl BackendConfig {
    /// Create a RocksDB backend configuration.
    pub fn rocksdb<P: Into<String>>(path: P) -> Self {
        BackendConfig::RocksDb { path: path.into() }
    }

    /// Create an in-memory backend configuration.
    pub fn in_memory() -> Self {
        BackendConfig::InMemory
    }

    /// Build the backend based on the configuration.
    pub fn build(&self) -> Result<Box<dyn IndexBackend>, IndexError> {
        match self {
            BackendConfig::InMemory => Ok(Box::new(InMemoryBackend::new())),
            BackendConfig::RocksDb { path } => {
                // The RocksDB backend is only available if the `backend-rocksdb` feature is enabled.
                #[cfg(feature = "backend-rocksdb")]
                {
                    Ok(Box::new(RocksDbBackend::open(path)?))
                }
                #[cfg(not(feature = "backend-rocksdb"))]
                {
                    // If the feature is not enabled, return an error.
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
        // The default backend is RocksDB if the feature is enabled.
        #[cfg(feature = "backend-rocksdb")]
        {
            BackendConfig::RocksDb {
                path: "data/ufp_index".into(),
            }
        }
        // Otherwise, fallback to In-memory.
        #[cfg(not(feature = "backend-rocksdb"))]
        {
            BackendConfig::InMemory
        }
    }
}

/// An in-memory backend using a `RwLock` around a `HashMap`.
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
        // The lock is held for the duration of the insert.
        self.records
            .write()
            .map_err(|_| IndexError::backend("poisoned lock"))?
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
        // The read lock is held for the duration of the get.
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
        // A single write lock is held for the entire batch insert.
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
        // A read lock is held for the duration of the scan.
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

/// The RocksDB backend implementation.
#[cfg(feature = "backend-rocksdb")]
mod rocksdb_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use rocksdb::{IteratorMode, Options, WriteBatch, DB};

    pub struct RocksDbBackend {
        db: DB,
    }

    impl RocksDbBackend {
        /// Open or create a RocksDB database at the given path.
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
            // RocksDB's WriteBatch provides atomic batch writes.
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
            // The iterator allows for scanning all key-value pairs.
            for item in self.db.iterator(IteratorMode::Start) {
                let (_, value) = item.map_err(IndexError::backend)?;
                visitor(&value)?;
            }
            Ok(())
        }

        fn flush(&self) -> Result<(), IndexError> {
            // Manually flush the memtable to disk.
            self.db.flush().map_err(IndexError::backend)
        }
    }
}

#[cfg(feature = "backend-rocksdb")]
pub use rocksdb_backend::RocksDbBackend;
