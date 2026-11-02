//! gestalts-connector: makes a folder on disk the source of truth for browser apps.
//!
//! Binds to 127.0.0.1 only and answers CORS requests from localhost origins plus any `--allow-origin`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get};
use clap::Parser;
use gestalts_core::StorageError;
use gestalts_fs::FsStorage;
use serde::Serialize;
use tower_http::cors::{AllowOrigin, CorsLayer};

mod plugins;
use plugins::{PluginHost, default_cache_dir};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// the data folder (holding notes/ and blobs/); created if missing
    #[arg(long)]
    data: PathBuf,
    #[arg(long, default_value_t = 7717)]
    port: u16,
    /// extra origins allowed to connect, besides http://localhost:* and http://127.0.0.1:*
    #[arg(long)]
    allow_origin: Vec<String>,
    /// directories containing built-in plugin bundles
    #[arg(long, default_value = "plugins")]
    plugins_dir: Vec<PathBuf>,
    /// instead of serving, print each note's gestalts and lints, and exit non-zero on lints
    #[arg(long)]
    lint: bool,
    /// instead of serving, prepare the folder as a published deck (content-addressed blobs,
    /// unreferenced ones deleted, index.json), then lint; exits non-zero on problems
    #[arg(long)]
    publish: bool,
}

#[derive(Clone)]
struct AppState { store: Arc<FsStorage>, plugins: PluginHost }

struct ApiError(StorageError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            StorageError::InvalidKey(_) => StatusCode::BAD_REQUEST,
            StorageError::ReadOnly => StatusCode::FORBIDDEN,
            StorageError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.0.to_string()).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        ApiError(e)
    }
}

#[derive(Serialize)]
struct Health {
    app: &'static str,
    version: &'static str,
    root: String,
}

#[derive(Serialize)]
struct NoteEntry {
    key: String,
    text: String,
}

async fn health(State(s): State<AppState>) -> Json<Health> {
    Json(Health { app: "gestalts-connector", version: env!("CARGO_PKG_VERSION"), root: s.store.root().display().to_string() })
}

async fn worker_access(headers: &HeaderMap, state: &AppState) -> Result<Option<gestalts_plugin_host::Access>, StatusCode> {
    let Some(value) = headers.get(header::AUTHORIZATION) else { return Ok(None) };
    let value = value.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?;
    let token = value.strip_prefix("Bearer ").ok_or(StatusCode::UNAUTHORIZED)?;
    state.plugins.policy_for_token(token).await.map(Some).ok_or(StatusCode::UNAUTHORIZED)
}

fn readable(access: &gestalts_plugin_host::Access, key: &str, text: &str) -> bool {
    let note = gestalts_core::Note::parse(key, text);
    let gs = gestalts_core::Gestalts::builtin().of(&note);
    access.read.gestalts.iter().any(|g| gs.contains(g)) && access.read.tags_all.iter().all(|t| note.has_tag(t))
}

fn writable(access: &gestalts_plugin_host::Access, key: &str, text: &str) -> bool {
    let note = gestalts_core::Note::parse(key, text);
    note.parse_error.is_none() && !access.write.tags_all.is_empty() && access.write.tags_all.iter().all(|t| note.has_tag(t))
}

async fn list_notes(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<NoteEntry>>, Response> {
    let s = &state.store;
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    let mut out = Vec::new();
    for key in s.list_notes_sync().map_err(ApiError::from).map_err(IntoResponse::into_response)? {
        if let Some(text) = s.read_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)?
            && access.as_ref().is_none_or(|a| readable(a, &key, &text)) {
            out.push(NoteEntry { key, text });
        }
    }
    Ok(Json(out))
}

async fn read_note(State(state): State<AppState>, Path(key): Path<String>, headers: HeaderMap) -> Result<Response, Response> {
    let s = &state.store;
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    Ok(match s.read_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)? {
        Some(t) if access.as_ref().is_none_or(|a| readable(a, &key, &t)) => ([(header::CONTENT_TYPE, "application/yaml; charset=utf-8")], t).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
        Some(_) => StatusCode::NOT_FOUND.into_response(),
    })
}

async fn write_note(State(state): State<AppState>, Path(key): Path<String>, headers: HeaderMap, body: String) -> Result<StatusCode, Response> {
    let s = &state.store;
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    if let Some(a) = &access {
        if !writable(a, &key, &body) { return Err(StatusCode::FORBIDDEN.into_response()); }
        if let Some(old) = s.read_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)?
            && !writable(a, &key, &old) { return Err(StatusCode::FORBIDDEN.into_response()); }
    }
    s.write_note_sync(&key, &body).map_err(ApiError::from).map_err(IntoResponse::into_response)?;
    state.plugins.note_changed(&key, &body).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_note(State(state): State<AppState>, Path(key): Path<String>, headers: HeaderMap) -> Result<StatusCode, Response> {
    let s = &state.store;
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    if let Some(a) = &access {
        let old = s.read_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)?;
        if old.as_ref().is_none_or(|text| !writable(a, &key, text)) { return Err(StatusCode::FORBIDDEN.into_response()); }
    }
    s.delete_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_blobs(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<String>>, Response> {
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    if let Some(access) = access {
        if access.blobs.read != gestalts_plugin_host::BlobRead::Referenced { return Ok(Json(vec![])); }
        let mut names = std::collections::BTreeSet::new();
        for key in state.store.list_notes_sync().map_err(ApiError::from).map_err(IntoResponse::into_response)? {
            if let Some(text) = state.store.read_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)?
                && readable(&access, &key, &text) {
                names.extend(gestalts_core::Note::parse(key, text).blobs().into_iter().map(|b| b.target));
            }
        }
        return Ok(Json(names.into_iter().collect()));
    }
    Ok(Json(state.store.list_blobs_sync().map_err(ApiError::from).map_err(IntoResponse::into_response)?))
}

async fn read_blob(State(state): State<AppState>, Path(name): Path<String>, headers: HeaderMap) -> Result<Response, Response> {
    let s = &state.store;
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    if let Some(access) = access {
        let mut allowed = false;
        if access.blobs.read == gestalts_plugin_host::BlobRead::Referenced {
            for key in s.list_notes_sync().map_err(ApiError::from).map_err(IntoResponse::into_response)? {
                if let Some(text) = s.read_note_sync(&key).map_err(ApiError::from).map_err(IntoResponse::into_response)?
                    && readable(&access, &key, &text)
                    && gestalts_core::Note::parse(key, text).blobs().iter().any(|b| b.target == name) { allowed = true; break; }
            }
        }
        if !allowed { return Err(StatusCode::NOT_FOUND.into_response()); }
    }
    Ok(match s.read_blob_sync(&name).map_err(ApiError::from).map_err(IntoResponse::into_response)? {
        Some(b) => {
            let mime = mime_guess::from_path(&name).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.to_string())], b).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    })
}

async fn write_blob(State(state): State<AppState>, Path(name): Path<String>, headers: HeaderMap, body: Bytes) -> Result<StatusCode, Response> {
    let s = &state.store;
    let access = worker_access(&headers, &state).await.map_err(IntoResponse::into_response)?;
    if access.as_ref().is_some_and(|a| !a.blobs.write) { return Err(StatusCode::FORBIDDEN.into_response()); }
    s.write_blob_sync(&name, &body).map_err(ApiError::from).map_err(IntoResponse::into_response)?;
    Ok(StatusCode::NO_CONTENT)
}

fn is_local_origin(origin: &str) -> bool {
    let rest = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://"));
    rest.is_some_and(|r| {
        let host = r.rsplit_once(':').map_or(r, |(h, _)| h);
        matches!(host, "localhost" | "127.0.0.1" | "[::1]")
    })
}

async fn list_plugins(State(s): State<AppState>) -> Json<Vec<plugins::PluginView>> { Json(s.plugins.views().await) }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunStarted { run_id: String }

async fn run_plugin(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<RunStarted>, (StatusCode, String)> {
    s.plugins.run(&id).await.map(|run_id| Json(RunStarted { run_id })).map_err(|e| (StatusCode::CONFLICT, e))
}

#[derive(Serialize)]
struct ServiceView { id: String, available: bool }

async fn list_services(State(s): State<AppState>) -> Json<Vec<ServiceView>> {
    Json(s.plugins.services().await.into_iter().map(|(id, available)| ServiceView { id, available }).collect())
}

async fn proxy_service(State(s): State<AppState>, Path((service, path)): Path<(String, String)>, method: Method, body: Bytes) -> Response {
    let Some(base) = s.plugins.service_url(&service).await else { return StatusCode::SERVICE_UNAVAILABLE.into_response() };
    let url = format!("{base}/services/{service}/{path}");
    match reqwest::Client::new().request(method, url).body(body).send().await {
        Ok(r) => {
            let status = r.status();
            let content_type = r.headers().get(header::CONTENT_TYPE).cloned();
            match r.bytes().await {
                Ok(bytes) => {
                    let mut response = (status, bytes).into_response();
                    if let Some(value) = content_type { response.headers_mut().insert(header::CONTENT_TYPE, value); }
                    response
                }
                Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

fn app(state: AppState, extra_origins: Vec<String>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            origin.to_str().is_ok_and(|o| is_local_origin(o) || extra_origins.iter().any(|e| e == o))
        }))
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
        .allow_private_network(true);
    Router::new()
        .route("/health", get(health))
        .route("/notes", get(list_notes))
        .route("/notes/{key}", get(read_note).put(write_note).delete(delete_note))
        .route("/blobs", get(list_blobs))
        .route("/blobs/{name}", get(read_blob).put(write_blob))
        .route("/plugins", get(list_plugins))
        .route("/plugins/{id}/runs", axum::routing::post(run_plugin))
        .route("/services", get(list_services))
        .route("/services/{service}/{*path}", any(proxy_service))
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        .layer(cors)
        .with_state(state)
}

/// Prints `key: gestalts` per note and any lints; returns the number of lints.
fn lint(store: &FsStorage) -> usize {
    let gestalts = gestalts_core::Gestalts::builtin();
    let mut count = 0;
    for key in store.list_notes_sync().expect("can list notes") {
        let raw = store.read_note_sync(&key).ok().flatten().unwrap_or_default();
        let note = gestalts_core::Note::parse(key.clone(), raw);
        println!("{key}: {}", gestalts.of(&note).join(", "));
        if let Some(e) = &note.parse_error {
            println!("  ! {}", e.message);
            count += 1;
        }
        for l in gestalts.lint(&note) {
            println!("  ! {} {}: {}", l.gestalt, l.path, l.message);
            count += 1;
        }
    }
    count
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.publish {
        let r = gestalts_fs::publish(&args.data).expect("can publish the folder");
        r.renamed.iter().for_each(|(a, b)| println!("renamed {a} -> {b}"));
        r.removed.iter().for_each(|b| println!("removed unreferenced {b}"));
        r.problems.iter().for_each(|p| println!("! {p}"));
        let n = r.problems.len() + lint(&FsStorage::new(&args.data));
        eprintln!("{n} problem(s)");
        std::process::exit(i32::from(n > 0));
    }
    if args.lint {
        let n = lint(&FsStorage::new(&args.data));
        eprintln!("{n} problem(s)");
        std::process::exit(i32::from(n > 0));
    }
    for sub in ["notes", "blobs"] {
        std::fs::create_dir_all(args.data.join(sub)).expect("can create the data folder");
    }
    let store = Arc::new(FsStorage::new(args.data.canonicalize().expect("data folder exists")));
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port is free");
    let plugins = PluginHost::discover(&args.plugins_dir, format!("http://{addr}"), default_cache_dir()).expect("plugins are valid");
    let state = AppState { store: store.clone(), plugins: plugins.clone() };
    tokio::spawn(async move { plugins.start_all().await; });
    println!("gestalts-connector serving {} on http://{addr}", store.root().display());
    axum::serve(listener, app(state, args.allow_origin))
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await
        .expect("server runs");
}

#[cfg(test)]
mod tests {
    use super::is_local_origin;

    #[test]
    fn local_origins() {
        assert!(is_local_origin("http://localhost:5173"));
        assert!(is_local_origin("http://127.0.0.1:4173"));
        assert!(!is_local_origin("https://evil.example"));
        assert!(!is_local_origin("http://localhost.evil.example:80"));
    }
}
