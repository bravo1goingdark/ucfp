use crate::IndexError;
use std::sync::RwLock;

/// Trait for a key-value storage backend for the index.
/// This allows for different storage implementations (e.g., in-memory, Redb).
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
///
/// This enum provides a unified way to configure different storage backends.
/// Each variant contains the necessary configuration for its respective backend.
///
/// # Example
/// ```
/// use index::BackendConfig;
///
/// // In-memory (for testing)
/// let config = BackendConfig::in_memory();
///
/// // Redb (pure Rust, recommended)
/// let config = BackendConfig::redb("/data/ucfp.redb");
/// ```
#[derive(Clone, Debug, Default)]
pub enum BackendConfig {
    /// Use Redb for storage. The `path` is the file path for the database.
    ///
    /// Redb is a pure Rust embedded database that doesn't require C++ dependencies.
    /// This is the recommended backend for most deployments.
    ///
    /// Requires the `backend-redb` feature to be enabled at compile time (enabled by default).
    Redb { path: String },
    /// Use an in-memory HashMap for storage. This is useful for testing.
    #[default]
    InMemory,
}

impl BackendConfig {
    /// Create an in-memory backend configuration.
    pub fn in_memory() -> Self {
        BackendConfig::InMemory
    }

    /// Create a Redb backend configuration.
    ///
    /// # Arguments
    /// * `path` - The file path where the database will be stored
    ///
    /// # Example
    /// ```
    /// use index::BackendConfig;
    ///
    /// let config = BackendConfig::redb("/data/ucfp.redb");
    /// ```
    pub fn redb<P: Into<String>>(path: P) -> Self {
        BackendConfig::Redb { path: path.into() }
    }

    /// Build the backend based on the configuration.
    ///
    /// This method creates the appropriate backend implementation based on the
    /// configuration variant. Each backend type is only available if its
    /// corresponding feature flag is enabled at compile time.
    ///
    /// # Returns
    /// * `Ok(Box<dyn IndexBackend>)` - Successfully created backend
    /// * `Err(IndexError)` - Failed to create backend or feature not enabled
    pub fn build(&self) -> Result<Box<dyn IndexBackend>, IndexError> {
        match self {
            BackendConfig::InMemory => Ok(Box::new(InMemoryBackend::new())),
            BackendConfig::Redb { path } => {
                // The Redb backend is only available if the `backend-redb` feature is enabled.
                #[cfg(feature = "backend-redb")]
                {
                    Ok(Box::new(RedbBackend::open(path)?))
                }
                #[cfg(not(feature = "backend-redb"))]
                {
                    // If the feature is not enabled, return an error.
                    let _ = path;
                    Err(IndexError::backend("redb backend disabled at compile time"))
                }
            }
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

/// The Redb backend implementation.
///
/// Redb is a pure Rust ACID-compliant embedded database that serves as the
/// default storage backend for UCFP.
#[cfg(feature = "backend-redb")]
pub mod redb;

#[cfg(feature = "backend-redb")]
pub use redb::RedbBackend;
