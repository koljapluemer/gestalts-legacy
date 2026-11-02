//! gestalts-connector: makes a folder on disk the source of truth for browser apps.
//!
//! Binds to 127.0.0.1 only and answers CORS requests from localhost origins plus any `--allow-origin`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use clap::Parser;
use gestalts_core::StorageError;
use gestalts_fs::FsStorage;
use serde::Serialize;
use tower_http::cors::{AllowOrigin, CorsLayer};

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
    /// instead of serving, print each note's gestalts and lints, and exit non-zero on lints
    #[arg(long)]
    lint: bool,
    /// instead of serving, prepare the folder as a published deck (content-addressed blobs,
    /// unreferenced ones deleted, index.json), then lint; exits non-zero on problems
    #[arg(long)]
    publish: bool,
}

type Store = Arc<FsStorage>;

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

type Res<T> = Result<T, ApiError>;

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

async fn health(State(s): State<Store>) -> Json<Health> {
    Json(Health { app: "gestalts-connector", version: env!("CARGO_PKG_VERSION"), root: s.root().display().to_string() })
}

async fn list_notes(State(s): State<Store>) -> Res<Json<Vec<NoteEntry>>> {
    let mut out = Vec::new();
    for key in s.list_notes_sync()? {
        if let Some(text) = s.read_note_sync(&key)? {
            out.push(NoteEntry { key, text });
        }
    }
    Ok(Json(out))
}

async fn read_note(State(s): State<Store>, Path(key): Path<String>) -> Res<Response> {
    Ok(match s.read_note_sync(&key)? {
        Some(t) => ([(header::CONTENT_TYPE, "application/yaml; charset=utf-8")], t).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    })
}

async fn write_note(State(s): State<Store>, Path(key): Path<String>, body: String) -> Res<StatusCode> {
    s.write_note_sync(&key, &body)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_note(State(s): State<Store>, Path(key): Path<String>) -> Res<StatusCode> {
    s.delete_note_sync(&key)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_blobs(State(s): State<Store>) -> Res<Json<Vec<String>>> {
    Ok(Json(s.list_blobs_sync()?))
}

async fn read_blob(State(s): State<Store>, Path(name): Path<String>) -> Res<Response> {
    Ok(match s.read_blob_sync(&name)? {
        Some(b) => {
            let mime = mime_guess::from_path(&name).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.to_string())], b).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    })
}

async fn write_blob(State(s): State<Store>, Path(name): Path<String>, body: Bytes) -> Res<StatusCode> {
    s.write_blob_sync(&name, &body)?;
    Ok(StatusCode::NO_CONTENT)
}

fn is_local_origin(origin: &str) -> bool {
    let rest = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://"));
    rest.is_some_and(|r| {
        let host = r.rsplit_once(':').map_or(r, |(h, _)| h);
        matches!(host, "localhost" | "127.0.0.1" | "[::1]")
    })
}

fn app(store: Store, extra_origins: Vec<String>) -> Router {
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
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        .layer(cors)
        .with_state(store)
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
    println!("gestalts-connector serving {} on http://{addr}", store.root().display());
    axum::serve(listener, app(store, args.allow_origin))
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
