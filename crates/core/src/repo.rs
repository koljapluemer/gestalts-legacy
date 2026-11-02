//! A storage plus an in-memory index of its notes, queryable by gestalt and tags.

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::gestalt::{Gestalts, Lint};
use crate::note::{Note, to_yaml};
use crate::storage::{Result, Storage, check_key};

/// Which notes a perspective wants. All given conditions must hold.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    /// `name@version`
    pub gestalt: Option<String>,
    pub tags_all: Vec<String>,
    pub tags_none: Vec<String>,
    pub keys: Option<Vec<String>>,
    /// Exact matches on top-level scalar fields.
    #[serde(rename = "where")]
    pub where_: BTreeMap<String, Value>,
}

/// A note together with what the core knows about it, as handed to frontends.
#[derive(Debug, Clone, Serialize)]
pub struct IndexedNote {
    #[serde(flatten)]
    pub note: Note,
    pub id: String,
    pub gestalts: Vec<String>,
    pub lints: Vec<Lint>,
}

impl IndexedNote {
    fn new(note: Note, gestalts: &Gestalts) -> Self {
        IndexedNote { id: note.id().to_owned(), gestalts: gestalts.of(&note), lints: gestalts.lint(&note), note }
    }
}

pub struct Repo<S> {
    storage: S,
    gestalts: Gestalts,
    index: RefCell<BTreeMap<String, IndexedNote>>,
}

impl<S: Storage> Repo<S> {
    pub fn new(storage: S, gestalts: Gestalts) -> Self {
        Repo { storage, gestalts, index: RefCell::default() }
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    pub fn gestalts(&self) -> &Gestalts {
        &self.gestalts
    }

    /// (Re)reads every note from storage.
    pub async fn load(&self) -> Result<()> {
        let all = self.storage.read_all_notes().await?;
        let index = all
            .into_iter()
            .map(|(k, raw)| (k.clone(), IndexedNote::new(Note::parse(k, raw), &self.gestalts)))
            .collect();
        *self.index.borrow_mut() = index;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<IndexedNote> {
        self.index.borrow().get(key).cloned()
    }

    /// The key of the note whose `id` (or key) is `id`.
    pub fn key_of(&self, id: &str) -> Option<String> {
        let index = self.index.borrow();
        if index.contains_key(id) {
            return Some(id.to_owned());
        }
        index.values().find(|n| n.id == id).map(|n| n.note.key.clone())
    }

    pub fn query(&self, q: &Query) -> Vec<IndexedNote> {
        self.index
            .borrow()
            .values()
            .filter(|n| q.gestalt.as_ref().is_none_or(|g| n.gestalts.contains(g)))
            .filter(|n| q.tags_all.iter().all(|t| n.note.has_tag(t)))
            .filter(|n| !q.tags_none.iter().any(|t| n.note.has_tag(t)))
            .filter(|n| q.keys.as_ref().is_none_or(|ks| ks.contains(&n.note.key)))
            .filter(|n| q.where_.iter().all(|(k, v)| n.note.get(k) == Some(v)))
            .cloned()
            .collect()
    }

    /// Writes raw text as-is (it may even be invalid YAML).
    pub async fn put_raw(&self, key: &str, raw: &str) -> Result<IndexedNote> {
        check_key(key)?;
        self.storage.write_note(key, raw).await?;
        let n = IndexedNote::new(Note::parse(key, raw), &self.gestalts);
        self.index.borrow_mut().insert(key.to_owned(), n.clone());
        Ok(n)
    }

    /// Writes a structured note. Without a key it gets a new uuid, used as both key and `id`.
    pub async fn put_value(&self, key: Option<&str>, mut value: Value) -> Result<IndexedNote> {
        let key = match key {
            Some(k) => k.to_owned(),
            None => {
                let id = value.get("id").and_then(Value::as_str).map(str::to_owned);
                let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if let Value::Mapping(m) = &mut value
                    && !m.contains_key("id")
                {
                    let mut with_id = Mapping::new();
                    with_id.insert("id".into(), id.clone().into());
                    with_id.extend(std::mem::take(m));
                    *m = with_id;
                }
                id
            }
        };
        self.put_raw(&key, &to_yaml(&value)).await
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.storage.delete_note(key).await?;
        self.index.borrow_mut().remove(key);
        Ok(())
    }
}

/// A fresh random uuid, for frontends that name new notes themselves.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// A stable uuid derived from a name, for idempotent imports.
pub fn stable_id(namespace: &str, name: &str) -> String {
    let ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, namespace.as_bytes());
    uuid::Uuid::new_v5(&ns, name.as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Memory;

    #[test]
    fn query_and_put() {
        pollster::block_on(async {
            let repo = Repo::new(Memory::default(), Gestalts::builtin());
            repo.storage().write_note("a", "tags: [flashcard]\nfront: x\nback: y\n").await.unwrap();
            repo.storage().write_note("b", "tags: [flashcard, archived]\nfront: x\nback: y\n").await.unwrap();
            repo.storage().write_note("c", "this: [is broken").await.unwrap();
            repo.load().await.unwrap();

            let q = Query { gestalt: Some("flashcard@1".into()), tags_none: vec!["archived".into()], ..Default::default() };
            let keys: Vec<_> = repo.query(&q).into_iter().map(|n| n.note.key).collect();
            assert_eq!(keys, vec!["a"]);
            assert_eq!(repo.query(&Query::default()).len(), 3);

            let mut m = Mapping::new();
            m.insert("title".into(), "new".into());
            let n = repo.put_value(None, Value::Mapping(m)).await.unwrap();
            assert_eq!(n.id, n.note.key);
            assert!(n.note.raw.starts_with("id: "));
            assert_eq!(repo.key_of(&n.id), Some(n.note.key.clone()));
            repo.delete(&n.note.key).await.unwrap();
            assert!(repo.get(&n.note.key).is_none());
        });
    }

    #[test]
    fn stable_ids() {
        assert_eq!(stable_id("x", "δελφίς"), stable_id("x", "δελφίς"));
        assert_ne!(stable_id("x", "a"), stable_id("x", "b"));
    }
}
