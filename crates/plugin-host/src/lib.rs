//! Domain contracts for plugins. Runtime adapters belong in the connector.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use gestalts_core::{Note, Query};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("cannot read {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("invalid plugin manifest: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("invalid plugin manifest: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema: u32,
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub gestalts: Vec<GestaltSource>,
    pub worker: WorkerManifest,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GestaltSource {
    pub id: String,
    pub definition: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerManifest {
    pub runtime: Runtime,
    #[serde(default = "default_project")]
    pub project: PathBuf,
    pub command: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub network: Vec<String>,
}

fn default_project() -> PathBuf { PathBuf::from(".") }

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Runtime { Uv, Binary, Container }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "on", rename_all = "lowercase", deny_unknown_fields)]
pub enum Trigger {
    Change { query: Query },
    Schedule { cron: String },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Access {
    pub read: ReadAccess,
    pub write: WriteAccess,
    pub blobs: BlobAccess,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReadAccess { pub gestalts: Vec<String>, pub tags_all: Vec<String> }

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WriteAccess { pub tags_all: Vec<String> }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BlobAccess { pub read: BlobRead, pub write: bool }

impl Default for BlobAccess { fn default() -> Self { Self { read: BlobRead::None, write: false } } }

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BlobRead { #[default] None, Referenced }

#[derive(Debug, Clone)]
pub struct PluginBundle { pub root: PathBuf, pub manifest: PluginManifest }

impl PluginBundle {
    pub fn load(root: impl Into<PathBuf>) -> Result<Self, ManifestError> {
        let root = root.into();
        let path = root.join("plugin.yml");
        let text = std::fs::read_to_string(&path).map_err(|e| ManifestError::Read(path, e))?;
        let manifest: PluginManifest = serde_yaml_ng::from_str(&text)?;
        validate(&manifest)?;
        for p in std::iter::once(&manifest.worker.project).chain(manifest.gestalts.iter().map(|g| &g.definition)) {
            safe_relative(p)?;
        }
        Ok(Self { root, manifest })
    }
}

fn safe_relative(path: &Path) -> Result<(), ManifestError> {
    if path.is_absolute() || path.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(ManifestError::Invalid(format!("unsafe bundle path {}", path.display())));
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'@' | b'.'))
}

fn valid_version(v: &str) -> bool {
    let mut parts = v.split('.');
    parts.by_ref().take(3).count() == 3 && parts.next().is_none() && v.split('.').all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
}

fn validate(m: &PluginManifest) -> Result<(), ManifestError> {
    if m.schema != 1 { return Err(ManifestError::Invalid(format!("unsupported schema {}", m.schema))); }
    if !valid_id(&m.id) || m.id.contains('@') { return Err(ManifestError::Invalid(format!("invalid plugin id {}", m.id))); }
    if !valid_version(&m.version) { return Err(ManifestError::Invalid(format!("invalid version {}", m.version))); }
    if m.worker.runtime != Runtime::Uv { return Err(ManifestError::Invalid("only the uv runtime is supported".into())); }
    if m.worker.command.is_empty() || m.worker.command.iter().any(String::is_empty) { return Err(ManifestError::Invalid("worker command is empty".into())); }
    let mut services = BTreeSet::new();
    for id in &m.worker.services {
        if !valid_id(id) || !id.contains('@') || !services.insert(id) { return Err(ManifestError::Invalid(format!("invalid or duplicate service {id}"))); }
    }
    for trigger in &m.worker.triggers {
        if let Trigger::Schedule { cron } = trigger {
            let fields: Vec<_> = cron.split_whitespace().collect();
            if fields.len() != 5 { return Err(ManifestError::Invalid(format!("cron must have five fields: {cron}"))); }
        }
    }
    Ok(())
}

/// Capability checks are pure domain policy and shared by every HTTP/storage adapter.
#[derive(Debug, Clone)]
pub struct AccessPolicy { pub access: Access, pub gestalt_ids: BTreeMap<String, Vec<String>> }

impl AccessPolicy {
    pub fn can_read(&self, note: &Note) -> bool {
        let gs = self.gestalt_ids.get(&note.key).map(Vec::as_slice).unwrap_or_default();
        self.access.read.gestalts.iter().any(|g| gs.contains(g)) && self.access.read.tags_all.iter().all(|t| note.has_tag(t))
    }
    pub fn can_write(&self, note: &Note) -> bool {
        note.parse_error.is_none() && !self.access.write.tags_all.is_empty() && self.access.write.tags_all.iter().all(|t| note.has_tag(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_path_escape() { assert!(safe_relative(Path::new("../worker")).is_err()); }
    #[test]
    fn validates_semver() { assert!(valid_version("1.2.0")); assert!(!valid_version("1.2")); }
    #[test]
    fn access_policy_requires_declared_shapes_and_owned_tags() {
        let access = Access { read: ReadAccess { gestalts: vec!["flashcard@1".into()], tags_all: vec![] },
            write: WriteAccess { tags_all: vec!["owned".into()] }, blobs: BlobAccess::default() };
        let mut gestalt_ids = BTreeMap::new(); gestalt_ids.insert("a".into(), vec!["flashcard@1".into()]);
        let policy = AccessPolicy { access, gestalt_ids };
        assert!(policy.can_read(&Note::parse("a", "tags: [flashcard]")));
        assert!(policy.can_write(&Note::parse("x", "tags: [owned]")));
        assert!(!policy.can_write(&Note::parse("x", "tags: [other]")));
    }
}
