use crate::IndexError;
#[cfg(feature = "plugin-loader")]
use std::sync::Arc;
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
    RocksDb {
        path: String,
    },
    InMemory,
    Redis {
        url: String,
        namespace: String,
    },
    Postgres {
        dsn: String,
        table: String,
    },
    Mongo {
        uri: String,
        database: String,
        collection: String,
    },
    Redb {
        path: String,
    },
    Sled {
        path: String,
    },
    Plugin {
        library_path: String,
        symbol: String,
        config: serde_json::Value,
    },
}

impl BackendConfig {
    pub fn rocksdb<P: Into<String>>(path: P) -> Self {
        BackendConfig::RocksDb { path: path.into() }
    }

    pub fn redis<U: Into<String>, N: Into<String>>(url: U, namespace: N) -> Self {
        BackendConfig::Redis {
            url: url.into(),
            namespace: namespace.into(),
        }
    }

    pub fn postgres<D: Into<String>, T: Into<String>>(dsn: D, table: T) -> Self {
        BackendConfig::Postgres {
            dsn: dsn.into(),
            table: table.into(),
        }
    }

    pub fn mongo<U: Into<String>, D: Into<String>, C: Into<String>>(
        uri: U,
        database: D,
        collection: C,
    ) -> Self {
        BackendConfig::Mongo {
            uri: uri.into(),
            database: database.into(),
            collection: collection.into(),
        }
    }

    pub fn redb<P: Into<String>>(path: P) -> Self {
        BackendConfig::Redb { path: path.into() }
    }

    pub fn sled<P: Into<String>>(path: P) -> Self {
        BackendConfig::Sled { path: path.into() }
    }

    pub fn plugin<P: Into<String>, S: Into<String>>(
        library_path: P,
        symbol: S,
        config: serde_json::Value,
    ) -> Self {
        BackendConfig::Plugin {
            library_path: library_path.into(),
            symbol: symbol.into(),
            config,
        }
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
            BackendConfig::Redis { url, namespace } => {
                #[cfg(feature = "backend-redis")]
                {
                    Ok(Box::new(RedisBackend::new(url, namespace)?))
                }
                #[cfg(not(feature = "backend-redis"))]
                {
                    let _ = (url, namespace);
                    Err(IndexError::backend(
                        "redis backend disabled at compile time",
                    ))
                }
            }
            BackendConfig::Postgres { dsn, table } => {
                #[cfg(feature = "backend-postgres")]
                {
                    Ok(Box::new(PostgresBackend::new(dsn, table)?))
                }
                #[cfg(not(feature = "backend-postgres"))]
                {
                    let _ = (dsn, table);
                    Err(IndexError::backend(
                        "postgres backend disabled at compile time",
                    ))
                }
            }
            BackendConfig::Mongo {
                uri,
                database,
                collection,
            } => {
                #[cfg(feature = "backend-mongo")]
                {
                    Ok(Box::new(MongoBackend::new(uri, database, collection)?))
                }
                #[cfg(not(feature = "backend-mongo"))]
                {
                    let _ = (uri, database, collection);
                    Err(IndexError::backend(
                        "mongo backend disabled at compile time",
                    ))
                }
            }
            BackendConfig::Redb { path } => {
                #[cfg(feature = "backend-redb")]
                {
                    Ok(Box::new(RedbBackend::open(path)?))
                }
                #[cfg(not(feature = "backend-redb"))]
                {
                    let _ = path;
                    Err(IndexError::backend("redb backend disabled at compile time"))
                }
            }
            BackendConfig::Sled { path } => {
                #[cfg(feature = "backend-sled")]
                {
                    Ok(Box::new(SledBackend::open(path)?))
                }
                #[cfg(not(feature = "backend-sled"))]
                {
                    let _ = path;
                    Err(IndexError::backend("sled backend disabled at compile time"))
                }
            }
            BackendConfig::Plugin {
                library_path,
                symbol,
                config,
            } => {
                #[cfg(feature = "plugin-loader")]
                {
                    Ok(Box::new(PluginBackend::load(library_path, symbol, config)?))
                }
                #[cfg(not(feature = "plugin-loader"))]
                {
                    let _ = (library_path, symbol, config);
                    Err(IndexError::backend(
                        "plugin loader disabled at compile time",
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

#[cfg(feature = "backend-redis")]
mod redis_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use redis::{Commands, Connection};
    use std::sync::Mutex;

    pub struct RedisBackend {
        client: redis::Client,
        namespace: String,
        connection: Mutex<Option<Connection>>,
    }

    impl RedisBackend {
        pub fn new(url: &str, namespace: &str) -> Result<Self, IndexError> {
            let client = redis::Client::open(url).map_err(IndexError::backend)?;
            Ok(Self {
                client,
                namespace: namespace.to_string(),
                connection: Mutex::new(None),
            })
        }

        fn with_connection<T>(
            &self,
            mut f: impl FnMut(&mut Connection) -> Result<T, IndexError>,
        ) -> Result<T, IndexError> {
            let mut guard = self
                .connection
                .lock()
                .map_err(|_| IndexError::backend("poisoned connection mutex"))?;
            if guard.is_none() {
                *guard = Some(self.client.get_connection().map_err(IndexError::backend)?);
            }
            let conn = guard
                .as_mut()
                .ok_or_else(|| IndexError::backend("missing redis connection"))?;
            f(conn)
        }

        fn namespaced(&self, key: &str) -> String {
            format!("{}:{}", self.namespace, key)
        }
    }

    impl IndexBackend for RedisBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            let namespaced = self.namespaced(key);
            self.with_connection(|conn| conn.set(namespaced, value).map_err(IndexError::backend))
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            let namespaced = self.namespaced(key);
            self.with_connection(|conn| conn.get(namespaced).map_err(IndexError::backend))
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            let namespaced = self.namespaced(key);
            self.with_connection(|conn| conn.del(namespaced).map_err(IndexError::backend))
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            self.with_connection(|conn| {
                let mut pipe = redis::pipe();
                for (key, value) in entries {
                    pipe.cmd("SET")
                        .arg(self.namespaced(&key))
                        .arg(value)
                        .ignore();
                }
                pipe.query(conn).map_err(IndexError::backend)
            })
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            self.with_connection(|conn| {
                let pattern = format!("{}:*", self.namespace);
                let mut iter: redis::Iter<String> =
                    conn.scan_match(pattern).map_err(IndexError::backend)?;
                while let Some(key) = iter.next() {
                    let value: Vec<u8> = conn.get(&key).map_err(IndexError::backend)?;
                    visitor(&value)?;
                }
                Ok(())
            })
        }

        fn flush(&self) -> Result<(), IndexError> {
            Ok(())
        }
    }
}

#[cfg(feature = "backend-redis")]
pub use redis_backend::RedisBackend;

#[cfg(feature = "backend-postgres")]
mod postgres_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use postgres::{Client, NoTls};
    use std::sync::Mutex;

    pub struct PostgresBackend {
        client: Mutex<Client>,
        table: String,
    }

    impl PostgresBackend {
        pub fn new(dsn: &str, table: &str) -> Result<Self, IndexError> {
            let mut client = Client::connect(dsn, NoTls).map_err(IndexError::backend)?;
            let table_ident = Self::quote_ident(table);
            let statement = format!(
                "CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, value BYTEA NOT NULL)",
                table_ident
            );
            client
                .batch_execute(&statement)
                .map_err(IndexError::backend)?;
            Ok(Self {
                client: Mutex::new(client),
                table: table_ident,
            })
        }

        fn quote_ident(name: &str) -> String {
            format!("\"{}\"", name.replace('"', "\"\""))
        }

        fn with_client<T>(
            &self,
            f: impl FnOnce(&mut Client, &str) -> Result<T, postgres::Error>,
        ) -> Result<T, IndexError> {
            let mut guard = self
                .client
                .lock()
                .map_err(|_| IndexError::backend("poisoned postgres mutex"))?;
            f(&mut guard, &self.table).map_err(IndexError::backend)
        }
    }

    impl IndexBackend for PostgresBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            self.with_client(|client, table| {
                let stmt = format!(
                    "INSERT INTO {} (key, value) VALUES ($1, $2) \
                     ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
                    table
                );
                client.execute(&stmt, &[&key, &value])?;
                Ok(())
            })
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            self.with_client(|client, table| {
                let stmt = format!("SELECT value FROM {} WHERE key = $1", table);
                let row = client.query_opt(&stmt, &[&key])?;
                Ok(row.map(|r| r.get::<_, Vec<u8>>(0)))
            })
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            self.with_client(|client, table| {
                let stmt = format!("DELETE FROM {} WHERE key = $1", table);
                client.execute(&stmt, &[&key])?;
                Ok(())
            })
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            self.with_client(|client, table| {
                let txn = client.transaction()?;
                let stmt = format!(
                    "INSERT INTO {} (key, value) VALUES ($1, $2) \
                     ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
                    table
                );
                for (key, value) in entries {
                    txn.execute(&stmt, &[&key, &value])?;
                }
                txn.commit()?;
                Ok(())
            })
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            self.with_client(|client, table| {
                let stmt = format!("SELECT value FROM {}", table);
                for row in client.query(&stmt, &[])? {
                    let value: Vec<u8> = row.get(0);
                    visitor(&value)?;
                }
                Ok(())
            })
        }

        fn flush(&self) -> Result<(), IndexError> {
            Ok(())
        }
    }
}

#[cfg(feature = "backend-postgres")]
pub use postgres_backend::PostgresBackend;

#[cfg(feature = "backend-mongo")]
mod mongo_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use mongodb::bson::{doc, Binary};
    use mongodb::options::ReplaceOptions;
    use mongodb::sync::{Client, Collection};

    pub struct MongoBackend {
        collection: Collection<mongodb::bson::Document>,
    }

    impl MongoBackend {
        pub fn new(uri: &str, database: &str, collection: &str) -> Result<Self, IndexError> {
            let client = Client::with_uri_str(uri).map_err(IndexError::backend)?;
            let coll = client
                .database(database)
                .collection::<mongodb::bson::Document>(collection);
            Ok(Self { collection: coll })
        }
    }

    impl IndexBackend for MongoBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            let filter = doc! { "key": key };
            let doc = doc! {
                "key": key,
                "value": Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: value.to_vec() },
            };
            let options = ReplaceOptions::builder().upsert(true).build();
            self.collection
                .replace_one(filter, doc, options)
                .map_err(IndexError::backend)?;
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            let filter = doc! { "key": key };
            let doc = self
                .collection
                .find_one(filter, None)
                .map_err(IndexError::backend)?;
            Ok(doc.and_then(|d| {
                d.get_binary_generic("value")
                    .map(|bytes| bytes.to_vec())
                    .ok()
            }))
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            let filter = doc! { "key": key };
            self.collection
                .delete_one(filter, None)
                .map_err(IndexError::backend)?;
            Ok(())
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            for (key, value) in entries {
                self.put(&key, &value)?;
            }
            Ok(())
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            let mut cursor = self
                .collection
                .find(None, None)
                .map_err(IndexError::backend)?;
            while let Some(doc) = cursor.next() {
                let doc = doc.map_err(IndexError::backend)?;
                if let Ok(value) = doc.get_binary_generic("value") {
                    visitor(value)?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(feature = "backend-mongo")]
pub use mongo_backend::MongoBackend;

#[cfg(feature = "backend-redb")]
mod redb_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use redb::{Database, ReadableTable, TableDefinition};

    const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("ufp_index");

    pub struct RedbBackend {
        db: Database,
    }

    impl RedbBackend {
        pub fn open(path: &str) -> Result<Self, IndexError> {
            let db = Database::create(path).map_err(IndexError::backend)?;
            Ok(Self { db })
        }
    }

    impl IndexBackend for RedbBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            let txn = self.db.begin_write().map_err(IndexError::backend)?;
            {
                let mut table = txn.open_table(TABLE).map_err(IndexError::backend)?;
                table.insert(key, value).map_err(IndexError::backend)?;
            }
            txn.commit().map_err(IndexError::backend)
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            let txn = self.db.begin_read().map_err(IndexError::backend)?;
            let table = txn.open_table(TABLE).map_err(IndexError::backend)?;
            let result = table.get(key).map_err(IndexError::backend)?;
            Ok(result.map(|guard| guard.value().to_vec()))
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            let txn = self.db.begin_write().map_err(IndexError::backend)?;
            {
                let mut table = txn.open_table(TABLE).map_err(IndexError::backend)?;
                table.remove(key).map_err(IndexError::backend)?;
            }
            txn.commit().map_err(IndexError::backend)
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            let txn = self.db.begin_write().map_err(IndexError::backend)?;
            {
                let mut table = txn.open_table(TABLE).map_err(IndexError::backend)?;
                for (key, value) in entries {
                    table
                        .insert(key.as_str(), value.as_slice())
                        .map_err(IndexError::backend)?;
                }
            }
            txn.commit().map_err(IndexError::backend)
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            let txn = self.db.begin_read().map_err(IndexError::backend)?;
            let table = txn.open_table(TABLE).map_err(IndexError::backend)?;
            for item in table.iter().map_err(IndexError::backend)? {
                let (_, value) = item.map_err(IndexError::backend)?;
                visitor(value.value())?;
            }
            Ok(())
        }

        fn flush(&self) -> Result<(), IndexError> {
            self.db.flush().map_err(IndexError::backend)
        }
    }
}

#[cfg(feature = "backend-redb")]
pub use redb_backend::RedbBackend;

#[cfg(feature = "backend-sled")]
mod sled_backend {
    use super::IndexBackend;
    use crate::IndexError;

    pub struct SledBackend {
        db: sled::Db,
    }

    impl SledBackend {
        pub fn open(path: &str) -> Result<Self, IndexError> {
            let db = sled::open(path).map_err(IndexError::backend)?;
            Ok(Self { db })
        }
    }

    impl IndexBackend for SledBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            self.db.insert(key, value).map_err(IndexError::backend)?;
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            let value = self.db.get(key).map_err(IndexError::backend)?;
            Ok(value.map(|v| v.to_vec()))
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            self.db.remove(key).map_err(IndexError::backend)?;
            Ok(())
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            let mut batch = sled::Batch::default();
            for (key, value) in entries {
                batch.insert(key.as_bytes(), value);
            }
            self.db.apply_batch(batch).map_err(IndexError::backend)?;
            Ok(())
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            for item in self.db.iter() {
                let (_, value) = item.map_err(IndexError::backend)?;
                visitor(value.as_ref())?;
            }
            Ok(())
        }

        fn flush(&self) -> Result<(), IndexError> {
            self.db.flush().map_err(IndexError::backend)?;
            Ok(())
        }
    }
}

#[cfg(feature = "backend-sled")]
pub use sled_backend::SledBackend;

#[cfg(feature = "plugin-loader")]
mod plugin_backend {
    use super::IndexBackend;
    use crate::IndexError;
    use libloading::{Library, Symbol};
    use serde_json::Value;
    use std::ffi::CString;
    use std::sync::Arc;

    type CreateFn =
        unsafe extern "C" fn(config_json: *const std::os::raw::c_char) -> *mut dyn IndexBackend;

    pub struct PluginBackend {
        backend: Box<dyn IndexBackend>,
        _lib: Arc<Library>,
    }

    impl PluginBackend {
        pub fn load(library_path: &str, symbol: &str, config: &Value) -> Result<Self, IndexError> {
            unsafe {
                let lib = Arc::new(Library::new(library_path).map_err(IndexError::backend)?);
                let constructor: Symbol<CreateFn> =
                    lib.get(symbol.as_bytes()).map_err(IndexError::backend)?;
                let cfg = serde_json::to_string(config).map_err(IndexError::backend)?;
                let c_string = CString::new(cfg).map_err(IndexError::backend)?;
                let raw = constructor(c_string.as_ptr());
                if raw.is_null() {
                    return Err(IndexError::backend("plugin returned null backend"));
                }
                let backend = Box::from_raw(raw);
                Ok(Self { backend, _lib: lib })
            }
        }
    }

    impl IndexBackend for PluginBackend {
        fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> {
            self.backend.put(key, value)
        }

        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> {
            self.backend.get(key)
        }

        fn delete(&self, key: &str) -> Result<(), IndexError> {
            self.backend.delete(key)
        }

        fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> {
            self.backend.batch_put(entries)
        }

        fn scan(
            &self,
            visitor: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>,
        ) -> Result<(), IndexError> {
            self.backend.scan(visitor)
        }

        fn flush(&self) -> Result<(), IndexError> {
            self.backend.flush()
        }
    }
}

#[cfg(feature = "plugin-loader")]
pub use plugin_backend::PluginBackend;
