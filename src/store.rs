//! What the game remembers between runs, in one file: text, numbers and blobs, each by
//! table and key.
//!
//! This module knows nothing about the game, as `pod` knows nothing about it, and has no
//! Bevy in it. It is [redb](https://www.redb.org) underneath, and no redb type appears in
//! its public API, so that redb can be swapped or upgraded without touching its users.
//!
//! redb stores `&str`, `f64` and `&[u8]` natively and a table has one type of value, so
//! there are three tables and no serialisation library. Keys are dotted names
//! (`choice.truck`), owned by whoever owns the value. A write is one transaction, so a
//! crash or a power cut while saving leaves the last good values and never half of the
//! new ones.

use std::fmt;
use std::path::Path;

use redb::backends::InMemoryBackend;
use redb::{Database, ReadableDatabase, TableDefinition, TableError};

const TEXT: TableDefinition<&str, &str> = TableDefinition::new("text");
const NUMBERS: TableDefinition<&str, f64> = TableDefinition::new("numbers");
const BLOBS: TableDefinition<&str, &[u8]> = TableDefinition::new("blobs");

pub struct Store {
    database: Database,
    /// For errors to name: the file, or "memory".
    name: String,
}

/// What went wrong, and with which store.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreError {
    pub store: String,
    pub what: String,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}: {}", self.store, self.what)
    }
}

impl std::error::Error for StoreError {}

/// Values to set, all together or not at all. See `Store::write`.
#[derive(Default)]
pub struct Batch {
    text: Vec<(String, String)>,
    numbers: Vec<(String, f64)>,
    blobs: Vec<(String, Vec<u8>)>,
}

impl Batch {
    pub fn set_text(&mut self, key: &str, value: &str) -> &mut Self {
        self.text.push((key.into(), value.into()));
        self
    }

    /// A number that isn't one (NaN, or infinite) fails the whole batch.
    pub fn set_number(&mut self, key: &str, value: f64) -> &mut Self {
        self.numbers.push((key.into(), value));
        self
    }

    pub fn set_blob(&mut self, key: &str, value: &[u8]) -> &mut Self {
        self.blobs.push((key.into(), value.into()));
        self
    }
}

impl Store {
    /// Opens the store in the file at `path`, making the file, and the folder it is in, if
    /// they aren't there.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let name = path.display().to_string();
        let error = |what: &dyn fmt::Display| StoreError {
            store: name.clone(),
            what: what.to_string(),
        };
        if let Some(folder) = path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
        {
            std::fs::create_dir_all(folder).map_err(|failure| error(&failure))?;
        }
        let database = Database::create(path).map_err(|failure| error(&failure))?;
        Ok(Self { database, name })
    }

    /// A store that is gone when it is dropped, for tests.
    pub fn in_memory() -> Self {
        let database = Database::builder()
            .create_with_backend(InMemoryBackend::new())
            .expect("an empty store in memory");
        Self {
            database,
            name: "memory".into(),
        }
    }

    /// `None` for a key that was never written.
    pub fn get_text(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.get(TEXT, key, |value| value.to_string())
    }

    pub fn get_number(&self, key: &str) -> Result<Option<f64>, StoreError> {
        self.get(NUMBERS, key, |value| value)
    }

    pub fn get_blob(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.get(BLOBS, key, |value| value.to_vec())
    }

    /// Sets any number of keys in a single transaction: `fill` says which, and either all
    /// of them are written or none is.
    pub fn write(&self, fill: impl FnOnce(&mut Batch)) -> Result<(), StoreError> {
        let mut batch = Batch::default();
        fill(&mut batch);
        if let Some((key, value)) = batch.numbers.iter().find(|(_, value)| !value.is_finite()) {
            return Err(self.error(&format!("{key} is {value}, which is not a number to keep")));
        }

        let transaction = self
            .database
            .begin_write()
            .map_err(|failure| self.error(&failure))?;
        // Dropping the transaction without committing it abandons everything in it.
        self.fill_tables(&transaction, &batch)?;
        transaction.commit().map_err(|failure| self.error(&failure))
    }

    fn fill_tables(
        &self,
        transaction: &redb::WriteTransaction,
        batch: &Batch,
    ) -> Result<(), StoreError> {
        let error = |failure: &dyn fmt::Display| self.error(failure);
        let mut text = transaction.open_table(TEXT).map_err(|e| error(&e))?;
        for (key, value) in &batch.text {
            text.insert(key.as_str(), value.as_str())
                .map_err(|e| error(&e))?;
        }
        let mut numbers = transaction.open_table(NUMBERS).map_err(|e| error(&e))?;
        for (key, value) in &batch.numbers {
            numbers.insert(key.as_str(), value).map_err(|e| error(&e))?;
        }
        let mut blobs = transaction.open_table(BLOBS).map_err(|e| error(&e))?;
        for (key, value) in &batch.blobs {
            blobs
                .insert(key.as_str(), value.as_slice())
                .map_err(|e| error(&e))?;
        }
        Ok(())
    }

    fn get<V: redb::Value + 'static, T>(
        &self,
        table: TableDefinition<&str, V>,
        key: &str,
        own: impl for<'a> FnOnce(V::SelfType<'a>) -> T,
    ) -> Result<Option<T>, StoreError> {
        let transaction = self
            .database
            .begin_read()
            .map_err(|failure| self.error(&failure))?;
        let table = match transaction.open_table(table) {
            Ok(table) => table,
            // Nothing has ever been written to it.
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(failure) => return Err(self.error(&failure)),
        };
        let value = table.get(key).map_err(|failure| self.error(&failure))?;
        Ok(value.map(|guard| own(guard.value())))
    }

    fn error(&self, what: &dyn fmt::Display) -> StoreError {
        StoreError {
            store: self.name.clone(),
            what: what.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_come_back_as_written() {
        let store = Store::in_memory();
        store
            .write(|batch| {
                batch
                    .set_text("choice.truck", "trucks/Big.pod")
                    .set_number("choice.laps", 5.0)
                    .set_blob("world.quick", &[1, 2, 3]);
            })
            .unwrap();
        assert_eq!(
            store.get_text("choice.truck").unwrap().as_deref(),
            Some("trucks/Big.pod")
        );
        assert_eq!(store.get_number("choice.laps").unwrap(), Some(5.0));
        assert_eq!(store.get_blob("world.quick").unwrap(), Some(vec![1, 2, 3]));

        // Writing again replaces, and leaves the rest alone.
        store
            .write(|batch| {
                batch.set_number("choice.laps", 2.0);
            })
            .unwrap();
        assert_eq!(store.get_number("choice.laps").unwrap(), Some(2.0));
        assert!(store.get_text("choice.truck").unwrap().is_some());
    }

    #[test]
    fn a_missing_key_is_none() {
        let store = Store::in_memory();
        // Before anything has been written at all, and after.
        assert_eq!(store.get_text("choice.truck").unwrap(), None);
        store
            .write(|batch| {
                batch.set_text("a", "b");
            })
            .unwrap();
        assert_eq!(store.get_text("choice.truck").unwrap(), None);
        assert_eq!(store.get_number("a").unwrap(), None);
        assert_eq!(store.get_blob("a").unwrap(), None);
    }

    #[test]
    fn a_batch_is_all_or_nothing() {
        let store = Store::in_memory();
        store
            .write(|batch| {
                batch.set_text("kept", "old");
            })
            .unwrap();

        let failed = store.write(|batch| {
            batch
                .set_text("kept", "new")
                .set_text("added", "new")
                .set_number("setup.gearing", f64::NAN);
        });
        let error = failed.unwrap_err();
        assert!(error.to_string().contains("setup.gearing"), "{error}");
        assert_eq!(store.get_text("kept").unwrap().as_deref(), Some("old"));
        assert_eq!(store.get_text("added").unwrap(), None);
    }
}
