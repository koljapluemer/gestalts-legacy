//! JS API of the gestalts core. Wrapped by `packages/client`; not meant to be used directly.

mod http;
mod opfs;

use std::rc::Rc;

use gestalts_core::storage::{Layered, Storage, StorageError};
use gestalts_core::{Gestalts, IndexedNote, Note, Query, Repo};
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::http::{Connector, Static};
use crate::opfs::Opfs;

type Dyn = Rc<dyn Storage>;

fn err(e: StorageError) -> JsError {
    JsError::new(&e.to_string())
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsError> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible()).map_err(|e| JsError::new(&e.to_string()))
}

/// A repo over browser storage. Every method returning data hands out plain JSON-like objects.
#[wasm_bindgen]
pub struct WasmRepo {
    repo: Rc<Repo<Dyn>>,
    /// where this repo's own writes land, and what syncs with a connector
    writable: Dyn,
    /// OPFS folder name, used to keep sync base snapshots next to the data
    dir: String,
}

#[wasm_bindgen]
impl WasmRepo {
    /// Notes in OPFS under `dir`.
    pub async fn opfs(dir: String) -> Result<WasmRepo, JsError> {
        let writable: Dyn = Rc::new(Opfs::open(&dir).await.map_err(err)?);
        Self::build(writable.clone(), writable, dir).await
    }

    /// Read-only notes published at `static_base` (see `Static`), overlaid with writable notes in OPFS under `dir`.
    pub async fn layered(static_base: String, dir: String) -> Result<WasmRepo, JsError> {
        let base = Static::open(&static_base).await.map_err(err)?;
        let writable: Dyn = Rc::new(Opfs::open(&dir).await.map_err(err)?);
        let storage: Dyn = Rc::new(Layered { base, top: writable.clone() });
        Self::build(storage, writable, dir).await
    }

    async fn build(storage: Dyn, writable: Dyn, dir: String) -> Result<WasmRepo, JsError> {
        let repo = Rc::new(Repo::new(storage, Gestalts::builtin()));
        repo.load().await.map_err(err)?;
        Ok(WasmRepo { repo, writable, dir })
    }

    /// Rereads everything from storage.
    pub async fn reload(&self) -> Result<(), JsError> {
        let repo = self.repo.clone();
        repo.load().await.map_err(err)
    }

    /// `query`: `{gestalt?, tagsAll?, tagsNone?, keys?}`
    pub fn query(&self, query: JsValue) -> Result<JsValue, JsError> {
        let q: Query = if query.is_undefined() || query.is_null() {
            Query::default()
        } else {
            serde_wasm_bindgen::from_value(query).map_err(|e| JsError::new(&e.to_string()))?
        };
        to_js(&self.repo.query(&q))
    }

    pub fn get(&self, key: &str) -> Result<JsValue, JsError> {
        to_js(&self.repo.get(key))
    }

    #[wasm_bindgen(js_name = keyOf)]
    pub fn key_of(&self, id: &str) -> Option<String> {
        self.repo.key_of(id)
    }

    #[wasm_bindgen(js_name = putRaw)]
    pub async fn put_raw(&self, key: String, raw: String) -> Result<JsValue, JsError> {
        let repo = self.repo.clone();
        to_js(&repo.put_raw(&key, &raw).await.map_err(err)?)
    }

    /// `json`: the note as JSON text; without `key`, a new uuid key/id is assigned.
    #[wasm_bindgen(js_name = putValue)]
    pub async fn put_value(&self, key: Option<String>, json: String) -> Result<JsValue, JsError> {
        let value: gestalts_core::Value = serde_json::from_str(&json).map_err(|e| JsError::new(&e.to_string()))?;
        let repo = self.repo.clone();
        to_js(&repo.put_value(key.as_deref(), value).await.map_err(err)?)
    }

    pub async fn delete(&self, key: String) -> Result<(), JsError> {
        let repo = self.repo.clone();
        repo.delete(&key).await.map_err(err)
    }

    #[wasm_bindgen(js_name = readBlob)]
    pub async fn read_blob(&self, name: String) -> Result<Option<Vec<u8>>, JsError> {
        let repo = self.repo.clone();
        repo.storage().read_blob(&name).await.map_err(err)
    }

    #[wasm_bindgen(js_name = writeBlob)]
    pub async fn write_blob(&self, name: String, bytes: Vec<u8>) -> Result<(), JsError> {
        let repo = self.repo.clone();
        repo.storage().write_blob(&name, &bytes).await.map_err(err)
    }

    /// All gestalt definitions.
    pub fn gestalts(&self) -> Result<JsValue, JsError> {
        to_js(&self.repo.gestalts().defs())
    }

    /// What saving `raw` would give, without saving: `{id, gestalts, lints, parse_error, ...}`.
    pub fn inspect(&self, key: &str, raw: &str) -> Result<JsValue, JsError> {
        let note = Note::parse(key, raw);
        let g = self.repo.gestalts();
        to_js(&IndexedNote { id: note.id().to_owned(), gestalts: g.of(&note), lints: g.lint(&note), note })
    }

    /// Syncs this repo's writable notes and blobs with a connector, then reloads. `tag` names conflict notes.
    pub async fn sync(&self, connector_url: String, tag: String) -> Result<JsValue, JsError> {
        let remote = Connector::new(&connector_url);
        remote.health().await.map_err(err)?;
        let slug: String = connector_url.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let base = Opfs::open(&format!("{}-sync-base/{slug}", self.dir)).await.map_err(err)?;
        let report = gestalts_core::sync(&*self.writable, &remote, &base, &tag).await.map_err(err)?;
        let repo = self.repo.clone();
        repo.load().await.map_err(err)?;
        to_js(&report)
    }
}

#[wasm_bindgen(js_name = newId)]
pub fn new_id() -> String {
    gestalts_core::new_id()
}

#[wasm_bindgen(js_name = stableId)]
pub fn stable_id(namespace: &str, name: &str) -> String {
    gestalts_core::stable_id(namespace, name)
}
