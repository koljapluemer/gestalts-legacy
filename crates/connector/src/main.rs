//! gestalts-connector: makes a folder on this computer the source of truth for the Gestalts web apps,
//! and hosts their plugins.
//!
//! Started without a subcommand it is a desktop app: a tray icon plus a small status window where you
//! choose the folder. `serve` runs the same server headless. It binds to 127.0.0.1 only.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use gestalts_fs::FsStorage;

#[cfg(feature = "desktop")]
mod desktop;
mod paths;
mod plugins;
mod server;
mod uv;

use paths::{Config, Paths};
use plugins::PluginHost;
use server::{AppState, Folder, UiRequest};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    opts: Opts,
}

#[derive(clap::Args, Clone, Debug)]
pub struct Opts {
    #[arg(long, global = true, default_value_t = 7717)]
    pub port: u16,
    /// plugin folders to use instead of the built-in plugins (for developing plugins)
    #[arg(long, global = true)]
    pub plugins_dir: Vec<PathBuf>,
    /// keeps settings separate from the installed app, e.g. `--profile dev`
    #[arg(long, global = true)]
    pub profile: Option<String>,
    /// extra web origins allowed to connect (besides localhost and the official frontends)
    #[arg(long, global = true)]
    pub allow_origin: Vec<String>,
    /// desktop app: start in the tray without opening the window (used by "Start on login")
    #[arg(long)]
    pub background: bool,
}

#[derive(Subcommand)]
enum Command {
    /// run without tray or window; the folder comes from --data or the remembered choice
    Serve {
        /// the data folder (notes/ + blobs/); created if missing; not remembered
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// the status window (opened from the tray)
    #[command(hide = true)]
    Status {
        /// open the folder picker right away
        #[arg(long)]
        choose: bool,
    },
    /// print each note's gestalts and lints; exits non-zero on lints
    Lint { dir: PathBuf },
    /// prepare a folder as a published deck (content-addressed blobs, unreferenced ones deleted,
    /// index.json), then lint; exits non-zero on problems
    Publish { dir: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        None => run_default(&cli.opts),
        Some(Command::Serve { data }) => serve(&cli.opts, data),
        Some(Command::Status { choose }) => status_window(&cli.opts, choose),
        Some(Command::Lint { dir }) => std::process::exit(i32::from(lint(&FsStorage::new(&dir)) > 0)),
        Some(Command::Publish { dir }) => std::process::exit(i32::from(publish(&dir) > 0)),
    };
    if let Err(e) = result {
        eprintln!("gestalts-connector: {e}");
        std::process::exit(1);
    }
}

#[cfg(feature = "desktop")]
fn run_default(opts: &Opts) -> Result<(), String> { desktop::run(opts) }
#[cfg(not(feature = "desktop"))]
fn run_default(opts: &Opts) -> Result<(), String> { serve(opts, None) }

#[cfg(feature = "desktop")]
fn status_window(opts: &Opts, choose: bool) -> Result<(), String> { desktop::status::run(opts, choose) }
#[cfg(not(feature = "desktop"))]
fn status_window(_: &Opts, _: bool) -> Result<(), String> { Err("this build has no desktop UI".into()) }

/// A running server: its own tokio runtime, so the desktop UI can own the main thread.
pub struct Server {
    rt: tokio::runtime::Runtime,
    plugins: PluginHost,
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
    pub paths: Paths,
    pub control_token: String,
}

impl Server {
    /// Stops serving and stops every plugin worker.
    pub fn stop(self) {
        let _ = self.shutdown.send(());
        let (plugins, task) = (self.plugins, self.task);
        self.rt.block_on(async move {
            plugins.stop_all().await;
            let _ = task.await;
        });
    }

    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output { self.rt.block_on(f) }

    pub fn spawn<F: std::future::Future<Output = ()> + Send + 'static>(&self, f: F) { self.rt.spawn(f); }
}

/// Starts the server and the plugins. `remember` saves folders chosen later in the app's config.
pub fn start_server(opts: &Opts, paths: Paths, folder: Option<Folder>, remember: bool, ui: Arc<dyn Fn(UiRequest) + Send + Sync>) -> Result<Server, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let control_token = uuid::Uuid::new_v4().to_string();
    paths::write_secret(&paths.control_token_file(), &control_token).map_err(|e| format!("cannot write the control token: {e}"))?;
    let plugin_dirs = if opts.plugins_dir.is_empty() {
        let builtin = plugins::unpack_builtin(&paths).map_err(|e| format!("cannot unpack the built-in plugins: {e}"))?;
        vec![builtin, paths.user_plugins()]
    } else {
        opts.plugins_dir.clone()
    };
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], opts.port));
    let listener = rt.block_on(tokio::net::TcpListener::bind(addr)).map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    let address = format!("http://{addr}");
    let plugins = PluginHost::discover(&plugin_dirs, address.clone(), paths.clone())?;
    let mut origins = Config::load(&paths).extra_origins;
    origins.extend(opts.allow_origin.iter().cloned());
    let state = AppState::new(folder, plugins.clone(), control_token.clone(), remember.then(|| paths.clone()), ui, address, origins);
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let starting = plugins.clone();
    rt.spawn(async move { starting.start_all().await });
    let task = rt.spawn(async move {
        let served = axum::serve(listener, server::app(state)).with_graceful_shutdown(async { let _ = stopped.await; }).await;
        if let Err(e) = served { eprintln!("gestalts-connector: server stopped: {e}"); }
    });
    Ok(Server { rt, plugins, shutdown, task, paths, control_token })
}

/// Resolves on Ctrl+C, or on SIGTERM (`kill`, logout, service managers) on Unix.
pub async fn stop_requested() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("can watch SIGTERM");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    { let _ = tokio::signal::ctrl_c().await; }
}

/// The remembered folder, if it still exists.
pub fn remembered_folder(paths: &Paths) -> Option<Folder> {
    let path = Config::load(paths).folder?;
    match Folder::open(&path) {
        Ok(f) => Some(f),
        Err(e) => { eprintln!("gestalts-connector: not reopening {}: {e}", path.display()); None }
    }
}

fn serve(opts: &Opts, data: Option<PathBuf>) -> Result<(), String> {
    let paths = Paths::new(opts.profile.as_deref())?;
    let (folder, remember) = match &data {
        Some(dir) => (Some(Folder::open(dir)?), false),
        None => (remembered_folder(&paths), true),
    };
    match &folder {
        Some(f) => println!("gestalts-connector serving {} on http://127.0.0.1:{}", f.info().path, opts.port),
        None => println!("gestalts-connector on http://127.0.0.1:{} (no folder yet: pass --data or choose one in the desktop app)", opts.port),
    }
    let server = start_server(opts, paths, folder, remember, Arc::new(|_| {}))?;
    server.block_on(stop_requested());
    server.stop();
    Ok(())
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
    eprintln!("{count} problem(s)");
    count
}

fn publish(dir: &std::path::Path) -> usize {
    let r = gestalts_fs::publish(dir).expect("can publish the folder");
    r.renamed.iter().for_each(|(a, b)| println!("renamed {a} -> {b}"));
    r.removed.iter().for_each(|b| println!("removed unreferenced {b}"));
    r.problems.iter().for_each(|p| println!("! {p}"));
    r.problems.len() + lint(&FsStorage::new(dir))
}
