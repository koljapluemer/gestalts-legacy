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
    /// `name@version` of gestalts the note must not have
    pub gestalts_none: Vec<String>,
    pub tags_all: Vec<String>,
    pub tags_none: Vec<String>,
    pub keys: Option<Vec<String>>,
    /// Exact matches on top-level scalar fields.
    #[serde(rename = "where")]
    pub where_: BTreeMap<String, Value>,
}

impl Query {
    /// Whether a note with these gestalts is one the query asks for.
    pub fn matches(&self, note: &Note, gestalts: &[String]) -> bool {
        self.gestalt.as_ref().is_none_or(|g| gestalts.contains(g))
            && !self.gestalts_none.iter().any(|g| gestalts.contains(g))
            && self.tags_all.iter().all(|t| note.has_tag(t))
            && !self.tags_none.iter().any(|t| note.has_tag(t))
            && self.keys.as_ref().is_none_or(|ks| ks.contains(&note.key))
            && self.where_.iter().all(|(k, v)| note.get(k) == Some(v))
    }
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

    /// Rereads just `keys` from storage (after something wrote them behind the repo's back, like a sync);
    /// a key no longer in storage leaves the index.
    pub async fn reindex(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            match self.storage.read_note(key).await? {
                Some(raw) => {
                    let n = IndexedNote::new(Note::parse(key, raw), &self.gestalts);
                    self.index.borrow_mut().insert(key.clone(), n);
                }
                None => {
                    self.index.borrow_mut().remove(key);
                }
            }
        }
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
            .filter(|n| q.matches(&n.note, &n.gestalts))
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

/// The key of a learner-state note (`fsrs-state`, `practice-state`) for `target`, or for one `facet` of it,
/// so two devices rating the same card write one note. Keys can't contain `/`, so the joined name is unambiguous.
pub fn state_key(gestalt: &str, target: &str, facet: Option<&str>) -> String {
    match facet {
        Some(f) => stable_id(gestalt, &format!("{target}/{f}")),
        None => stable_id(gestalt, target),
    }
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
            let q = Query { gestalts_none: vec!["flashcard@1".into()], ..Default::default() };
            let keys: Vec<_> = repo.query(&q).into_iter().map(|n| n.note.key).collect();
            assert_eq!(keys, vec!["c"]);

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
    fn reindex_reads_only_the_given_keys() {
        pollster::block_on(async {
            let repo = Repo::new(Memory::default(), Gestalts::builtin());
            repo.storage().write_note("a", "title: a").await.unwrap();
            repo.storage().write_note("b", "title: b").await.unwrap();
            repo.load().await.unwrap();

            // written behind the repo's back, as a sync does
            repo.storage().write_note("a", "tags: [flashcard]\nfront: x\nback: y\n").await.unwrap();
            repo.storage().delete_note("b").await.unwrap();
            repo.storage().write_note("c", "title: c").await.unwrap();
            repo.storage().write_note("d", "title: d").await.unwrap();
            repo.reindex(&["a".into(), "b".into(), "c".into()]).await.unwrap();

            assert_eq!(repo.get("a").unwrap().gestalts, vec!["flashcard@1"]);
            assert!(repo.get("b").is_none());
            assert!(repo.get("c").is_some());
            assert!(repo.get("d").is_none(), "not asked for");
        });
    }

    #[test]
    fn stable_ids() {
        assert_eq!(stable_id("x", "δελφίς"), stable_id("x", "δελφίς"));
        assert_ne!(stable_id("x", "a"), stable_id("x", "b"));
        assert_eq!(state_key("fsrs-state", "c", None), stable_id("fsrs-state", "c"));
        assert_eq!(state_key("fsrs-state", "c", Some("level:2")), stable_id("fsrs-state", "c/level:2"));
        assert_ne!(state_key("fsrs-state", "c", Some("a")), state_key("practice-state", "c", Some("a")));
    }
}
