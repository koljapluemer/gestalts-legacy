use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use gestalts_core::{Gestalts, Note, Query};
use gestalts_plugin_host::{PluginBundle, Runtime, Trigger};
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub id: String,
    pub version: String,
    pub state: PluginState,
    pub services: Vec<String>,
    pub error: Option<String>,
    pub current_run: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginState { Stopped, Starting, Ready, Running, Error }

pub struct HostedPlugin {
    pub bundle: PluginBundle,
    pub view: PluginView,
    pub token: String,
    pub worker_url: Option<String>,
    child: Option<Child>,
}

#[derive(Clone)]
pub struct PluginHost {
    inner: Arc<RwLock<BTreeMap<String, Arc<Mutex<HostedPlugin>>>>>,
    connector_url: String,
    cache_dir: PathBuf,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunRequest<'a> { run_id: &'a str, trigger: TriggerRequest, note_keys: Vec<String> }

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum TriggerRequest { Manual, Change }

#[derive(Debug, Deserialize)]
struct Health { protocol: u32 }

impl PluginHost {
    pub fn discover(dirs: &[PathBuf], connector_url: String, cache_dir: PathBuf) -> Result<Self, String> {
        let mut found = BTreeMap::new();
        let mut services = BTreeSet::new();
        for dir in dirs {
            let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot read plugin directory {}: {e}", dir.display()))?;
            for entry in entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) {
                if !entry.path().join("plugin.yml").is_file() { continue; }
                let bundle = PluginBundle::load(entry.path()).map_err(|e| e.to_string())?;
                if found.contains_key(&bundle.manifest.id) { return Err(format!("duplicate plugin {}", bundle.manifest.id)); }
                for service in &bundle.manifest.worker.services {
                    if !services.insert(service.clone()) { return Err(format!("duplicate service {service}")); }
                }
                let id = bundle.manifest.id.clone();
                let view = PluginView { id: id.clone(), version: bundle.manifest.version.clone(), state: PluginState::Stopped,
                    services: bundle.manifest.worker.services.clone(), error: None, current_run: None };
                found.insert(id, Arc::new(Mutex::new(HostedPlugin { bundle, view, token: uuid::Uuid::new_v4().to_string(), worker_url: None, child: None })));
            }
        }
        Ok(Self { inner: Arc::new(RwLock::new(found)), connector_url, cache_dir, client: reqwest::Client::new() })
    }

    pub async fn start_all(&self) {
        let plugins: Vec<_> = self.inner.read().await.values().cloned().collect();
        for plugin in plugins {
            if let Err(e) = self.start(plugin.clone()).await {
                let mut p = plugin.lock().await;
                p.view.state = PluginState::Error;
                p.view.error = Some(e);
            }
        }
    }

    async fn start(&self, plugin: Arc<Mutex<HostedPlugin>>) -> Result<(), String> {
        let mut p = plugin.lock().await;
        p.view.state = PluginState::Starting;
        if p.bundle.manifest.worker.runtime != Runtime::Uv { return Err("unsupported runtime".into()); }
        let project = p.bundle.root.join(&p.bundle.manifest.worker.project);
        let environment = self.cache_dir.join(&p.bundle.manifest.id).join("venv");
        std::fs::create_dir_all(environment.parent().unwrap()).map_err(|e| e.to_string())?;
        let sync = Command::new("uv").args(["sync", "--locked", "--project"]).arg(&project)
            .env("UV_PROJECT_ENVIRONMENT", &environment).stdout(Stdio::null()).stderr(Stdio::piped()).output().await.map_err(|e| format!("cannot run uv: {e}"))?;
        if !sync.status.success() { return Err(format!("uv sync failed: {}", String::from_utf8_lossy(&sync.stderr).trim())); }
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);
        let command = &p.bundle.manifest.worker.command;
        let mut cmd = Command::new("uv");
        cmd.args(["run", "--locked", "--project"]).arg(&project).arg("--").arg(&command[0]).args(&command[1..])
            .current_dir(&project)
            .env("UV_PROJECT_ENVIRONMENT", &environment)
            .env("GESTALTS_CONNECTOR_URL", &self.connector_url).env("GESTALTS_PLUGIN_TOKEN", &p.token)
            .env("GESTALTS_PLUGIN_ID", &p.bundle.manifest.id).env("GESTALTS_WORKER_PORT", port.to_string())
            .stdout(Stdio::inherit()).stderr(Stdio::inherit()).kill_on_drop(true);
        p.child = Some(cmd.spawn().map_err(|e| format!("cannot start worker: {e}"))?);
        let url = format!("http://127.0.0.1:{port}");
        p.worker_url = Some(url.clone());
        drop(p);
        for _ in 0..50 {
            if let Ok(response) = self.client.get(format!("{url}/_gestalts/health")).send().await
                && let Ok(health) = response.json::<Health>().await && health.protocol == 1 {
                let mut p = plugin.lock().await; p.view.state = PluginState::Ready; return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("worker readiness timed out".into())
    }

    pub async fn views(&self) -> Vec<PluginView> {
        let plugins: Vec<_> = self.inner.read().await.values().cloned().collect();
        let mut out = Vec::new();
        for p in plugins { out.push(p.lock().await.view.clone()); }
        out
    }

    pub async fn run(&self, id: &str) -> Result<String, String> {
        self.run_with(id, TriggerRequest::Manual, vec![]).await
    }

    async fn run_with(&self, id: &str, trigger: TriggerRequest, note_keys: Vec<String>) -> Result<String, String> {
        let plugin = self.inner.read().await.get(id).cloned().ok_or_else(|| "unknown plugin".to_string())?;
        let (url, run_id) = {
            let mut p = plugin.lock().await;
            if p.view.state != PluginState::Ready { return Err("plugin is not ready".into()); }
            let run_id = uuid::Uuid::new_v4().to_string();
            p.view.state = PluginState::Running; p.view.current_run = Some(run_id.clone());
            (p.worker_url.clone().unwrap(), run_id)
        };
        let host = self.clone(); let id = id.to_owned(); let returned = run_id.clone();
        tokio::spawn(async move {
            let result = host.client.post(format!("{url}/_gestalts/runs"))
                .json(&RunRequest { run_id: &run_id, trigger, note_keys }).send().await;
            if let Some(plugin) = host.inner.read().await.get(&id).cloned() {
                let mut p = plugin.lock().await; p.view.current_run = None;
                match result { Ok(r) if r.status().is_success() => p.view.state = PluginState::Ready,
                    Ok(r) => { p.view.state = PluginState::Error; p.view.error = Some(format!("run returned {}", r.status())); },
                    Err(e) => { p.view.state = PluginState::Error; p.view.error = Some(e.to_string()); } }
            }
        });
        Ok(returned)
    }

    pub async fn note_changed(&self, key: &str, text: &str) {
        let note = Note::parse(key, text);
        let gestalts = Gestalts::builtin().of(&note);
        let plugins: Vec<_> = self.inner.read().await.values().cloned().collect();
        for plugin in plugins {
            let id = {
                let p = plugin.lock().await;
                let matched = p.bundle.manifest.worker.triggers.iter().any(|t| match t {
                    Trigger::Change { query } => query_matches(query, &note, &gestalts),
                    Trigger::Schedule { .. } => false,
                });
                matched.then(|| p.bundle.manifest.id.clone())
            };
            if let Some(id) = id { let _ = self.run_with(&id, TriggerRequest::Change, vec![key.to_owned()]).await; }
        }
    }

    pub async fn services(&self) -> Vec<(String, bool)> {
        let plugins: Vec<_> = self.inner.read().await.values().cloned().collect(); let mut out = vec![];
        for p in plugins { let p = p.lock().await; for s in &p.view.services { out.push((s.clone(), matches!(p.view.state, PluginState::Ready | PluginState::Running))); } }
        out
    }

    pub async fn service_url(&self, service: &str) -> Option<String> {
        let plugins: Vec<_> = self.inner.read().await.values().cloned().collect();
        for p in plugins { let p = p.lock().await; if p.view.services.iter().any(|s| s == service) && matches!(p.view.state, PluginState::Ready | PluginState::Running) { return p.worker_url.clone(); } }
        None
    }

    pub async fn policy_for_token(&self, token: &str) -> Option<gestalts_plugin_host::Access> {
        let plugins: Vec<_> = self.inner.read().await.values().cloned().collect();
        for p in plugins { let p = p.lock().await; if p.token == token { return Some(p.bundle.manifest.worker.access.clone()); } }
        None
    }
}

fn query_matches(query: &Query, note: &Note, gestalts: &[String]) -> bool {
    query.gestalt.as_ref().is_none_or(|g| gestalts.contains(g))
        && query.tags_all.iter().all(|t| note.has_tag(t))
        && !query.tags_none.iter().any(|t| note.has_tag(t))
        && query.keys.as_ref().is_none_or(|keys| keys.contains(&note.key))
        && query.where_.iter().all(|(field, value)| note.get(field) == Some(value))
}

pub fn default_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("gestalts-cache")).join("gestalts/plugins")
}
