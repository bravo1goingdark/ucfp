//! Redb (Rust embedded database) backend implementation for UCFP index storage.
//!
//! Redb is a pure Rust embedded key-value store that provides ACID transactions
//! without requiring external dependencies. This makes it ideal for all deployments
//! where fast compilation and easy setup are priorities.
//!
//! # Features
//! - ACID transactions with MVCC
//! - Zero-copy reads
//! - Crash-safe by default
//! - No external dependencies (pure Rust)
//!
//! # Configuration Example
//! ```yaml
//! index:
//!   backend: "redb"
//!   redb:
//!     path: "/data/ucfp.redb"
//! ```

use crate::{IndexBackend, IndexError};
use redb::{Database, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;

/// Table definition for the UCFP index data
const UCFP_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("ucfp_data");

/// Redb backend implementation for persistent key-value storage.
///
/// This backend uses redb's ACID transactions to ensure data consistency.
/// All operations are atomic and durable by default.
///
/// # Thread Safety
/// The `Arc<Database>` wrapper allows safe sharing across threads.
/// Redb handles its own internal locking and MVCC.
pub struct RedbBackend {
    db: Arc<Database>,
}

impl RedbBackend {
    /// Open or create a Redb database at the given path.
    ///
    /// # Arguments
    /// * `path` - The file path where the database will be stored
    ///
    /// # Returns
    /// * `Ok(RedbBackend)` - Successfully opened or created database
    /// * `Err(IndexError)` - Failed to open/create database
    ///
    /// # Example
    /// ```no_run
    /// use index::RedbBackend;
    ///
    /// let backend = RedbBackend::open("/tmp/test.redb").unwrap();
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, IndexError> {
        let db = Database::create(path).map_err(|e| IndexError::backend(e.to_string()))?;

        // Initialize the table if it doesn't exist
        let write_txn = db
            .begin_write()
            .map_err(|e| IndexError::backend(e.to_string()))?;
        {
            // Accessing the table creates it if it doesn't exist
            let _table = write_txn
                .open_table(UCFP_TABLE)
                .map_err(|e| IndexError::backend(e.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|e| IndexError::backend(e.to_string()))?;

        Ok(Self { db: Arc::new(db) })
    }
}

impl IndexBackend for RedbBackend {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| IndexError::backend(e.to_string()))?;

        {
            let mut table = write_txn
                .open_table(UCFP_TABLE)
                .map_err(|e| IndexError::backend(e.to_string()))?;
            table
                .insert(key, value)
                .map_err(|e| IndexError::backend(e.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|e| IndexError::backend(e.to_string()))?;
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| IndexError::backend(e.to_string()))?;
        let table = read_txn
            .open_table(UCFP_TABLE)
            .map_err(|e| IndexError::backend(e.to_string()))?;

        match table
            .get(key)
            .map_err(|e| IndexError::backend(e.to_string()))?
        {
            Some(value) => Ok(Some(value.value().to_vec())),
            None => Ok(None),
        }
    }

    fn delete(&self, key: &str) -> Result<(), IndexError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| IndexError::backend(e.to_string()))?;

        {
            let mut table = write_txn
                .open_table(UCFP_TABLE)
                .map_err(|e| IndexError::backend(e.to_string()))?;
            table
                .remove(key)
                .map_err(|e| IndexError::backend(e.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|e| IndexError::backend(e.to_string()))?;
        Ok(())
    }

    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| IndexError::backend(e.to_string()))?;

        {
            let mut table = write_txn
                .open_table(UCFP_TABLE)
                .map_err(|e| IndexError::backend(e.to_string()))?;

            for (key, value) in entries {
                table
                    .insert(key.as_str(), value.as_slice())
                    .map_err(|e| IndexError::backend(e.to_string()))?;
            }
        }

        write_txn
            .commit()
            .map_err(|e| IndexError::backend(e.to_string()))?;
        Ok(())
    }

    fn scan(
        &self,
        visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
    ) -> Result<(), IndexError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| IndexError::backend(e.to_string()))?;
        let table = read_txn
            .open_table(UCFP_TABLE)
            .map_err(|e| IndexError::backend(e.to_string()))?;

        for item in table
            .iter()
            .map_err(|e| IndexError::backend(e.to_string()))?
        {
            let (_, value) = item.map_err(|e| IndexError::backend(e.to_string()))?;
            visitor(value.value())?;
        }

        Ok(())
    }

    fn flush(&self) -> Result<(), IndexError> {
        // Redb commits are synchronous by default, so flush is a no-op
        // This ensures data is immediately durable
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_redb_backend_roundtrip() {
        let temp_file = NamedTempFile::new().unwrap();
        let backend = RedbBackend::open(temp_file.path()).unwrap();

        // Test put and get
        backend.put("key1", b"value1").unwrap();
        let result = backend.get("key1").unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Test non-existent key
        let result = backend.get("nonexistent").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_redb_backend_batch() {
        let temp_file = NamedTempFile::new().unwrap();
        let backend = RedbBackend::open(temp_file.path()).unwrap();

        let entries = vec![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
            ("key3".to_string(), b"value3".to_vec()),
        ];

        backend.batch_put(entries).unwrap();

        assert_eq!(backend.get("key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(backend.get("key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(backend.get("key3").unwrap(), Some(b"value3".to_vec()));
    }

    #[test]
    fn test_redb_backend_delete() {
        let temp_file = NamedTempFile::new().unwrap();
        let backend = RedbBackend::open(temp_file.path()).unwrap();

        backend.put("key1", b"value1").unwrap();
        assert_eq!(backend.get("key1").unwrap(), Some(b"value1".to_vec()));

        backend.delete("key1").unwrap();
        assert_eq!(backend.get("key1").unwrap(), None);
    }

    #[test]
    fn test_redb_backend_scan() {
        let temp_file = NamedTempFile::new().unwrap();
        let backend = RedbBackend::open(temp_file.path()).unwrap();

        backend.put("key1", b"value1").unwrap();
        backend.put("key2", b"value2").unwrap();

        let mut collected = Vec::new();
        backend
            .scan(&mut |value| {
                collected.push(value.to_vec());
                Ok(())
            })
            .unwrap();

        assert_eq!(collected.len(), 2);
        assert!(collected.contains(&b"value1".to_vec()));
        assert!(collected.contains(&b"value2".to_vec()));
    }
}
