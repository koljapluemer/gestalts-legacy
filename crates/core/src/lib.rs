//! gestalts core: notes (YAML) and blobs behind a storage port, gestalts, querying and merging.
//! No IO here; adapters live in `gestalts-fs` (disk) and `gestalts-wasm` (browser).

pub mod gestalt;
pub mod merge;
pub mod note;
pub mod repo;
pub mod storage;
pub mod upstream;

pub use gestalt::{GestaltDef, Gestalts, Lint};
pub use merge::{SyncReport, merge3, sync};
pub use note::{Note, ParseError, Ref};
pub use repo::{IndexedNote, Query, Repo, new_id, stable_id};
pub use storage::{Layered, Memory, Storage, StorageError};
pub use upstream::{UpstreamReport, content_hash, pull_upstream};
pub use serde_yaml_ng::Value;
