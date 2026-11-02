//! The status window: which folder is open, which web apps are connected, how the plugins are doing.
//! It runs as its own process and talks to the connector over `/control/*`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

use super::{ICON, TITLE, TOKEN_ENV};
use crate::Opts;
use crate::paths::Paths;
use crate::plugins::PluginState;
use crate::server::{CONTROL_HEADER, OFFICIAL_ORIGINS, StatusView};
use crate::uv::UvState;

#[derive(Clone)]
struct Api {
    base: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl Api {
    fn status(&self) -> Result<StatusView, String> {
        let r = self.client.get(format!("{}/control/status", self.base)).header(CONTROL_HEADER, &self.token).send().map_err(|_| "The connector isn't running.".to_owned())?;
        r.error_for_status().map_err(|e| e.to_string())?.json().map_err(|e| e.to_string())
    }

    fn choose_folder(&self, path: &std::path::Path) -> Result<StatusView, String> {
        let r = self.client.post(format!("{}/control/folder", self.base)).header(CONTROL_HEADER, &self.token)
            .json(&serde_json::json!({ "path": path })).send().map_err(|e| e.to_string())?;
        if !r.status().is_success() { return Err(r.text().unwrap_or_default()); }
        r.json().map_err(|e| e.to_string())
    }

    fn quit(&self) { let _ = self.client.post(format!("{}/control/quit", self.base)).header(CONTROL_HEADER, &self.token).send(); }
}

#[derive(Default)]
struct Latest {
    status: Option<StatusView>,
    error: Option<String>,
}

pub fn run(opts: &Opts, choose: bool) -> Result<(), String> {
    let paths = Paths::new(opts.profile.as_deref())?;
    let token = std::env::var(TOKEN_ENV).ok()
        .or_else(|| std::fs::read_to_string(paths.control_token_file()).ok().map(|t| t.trim().to_owned()))
        .ok_or("the connector isn't running")?;
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build().map_err(|e| e.to_string())?;
    let api = Api { base: format!("http://127.0.0.1:{}", opts.port), token, client };
    let latest = Arc::new(Mutex::new(Latest::default()));
    let autostart = autostart(opts);
    let icon = eframe::icon_data::from_png_bytes(ICON).map_err(|e| e.to_string())?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title(TITLE).with_app_id("gestalts-connector")
            .with_inner_size([460.0, 440.0]).with_icon(Arc::new(icon)),
        ..Default::default()
    };
    eframe::run_native(TITLE, options, Box::new(move |cc| {
        poll(cc.egui_ctx.clone(), api.clone(), latest.clone());
        let autostart_on = autostart.as_ref().and_then(|a| a.is_enabled().ok()).unwrap_or(false);
        Ok(Box::new(StatusApp { api, latest, choose_pending: choose, autostart, autostart_on, message: None }))
    })).map_err(|e| e.to_string())
}

/// Refreshes the status every second.
fn poll(ctx: egui::Context, api: Api, latest: Arc<Mutex<Latest>>) {
    std::thread::spawn(move || loop {
        let result = api.status();
        {
            let mut l = latest.lock().expect("status lock");
            match result {
                Ok(s) => { l.status = Some(s); l.error = None; }
                Err(e) => l.error = Some(e),
            }
        }
        ctx.request_repaint();
        std::thread::sleep(Duration::from_secs(1));
    });
}

/// Start on login, pointing at the installed binary (or the AppImage, whose mount path changes per run).
fn autostart(opts: &Opts) -> Option<auto_launch::AutoLaunch> {
    let exe = std::env::var("APPIMAGE").ok().or_else(|| std::env::current_exe().ok().map(|p| p.display().to_string()))?;
    let name = opts.profile.as_ref().map_or_else(|| "gestalts-connector".to_owned(), |p| format!("gestalts-connector-{p}"));
    let mut args = vec!["--background".to_owned()];
    if let Some(p) = &opts.profile { args.extend(["--profile".to_owned(), p.clone()]); }
    if opts.port != 7717 { args.extend(["--port".to_owned(), opts.port.to_string()]); }
    auto_launch::AutoLaunchBuilder::new().set_app_name(&name).set_app_path(&exe).set_args(&args).build().ok()
}

struct StatusApp {
    api: Api,
    latest: Arc<Mutex<Latest>>,
    choose_pending: bool,
    autostart: Option<auto_launch::AutoLaunch>,
    autostart_on: bool,
    /// the last action's error
    message: Option<String>,
}

impl StatusApp {
    fn choose_folder(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Choose your Gestalts folder").pick_folder() else { return };
        match self.api.choose_folder(&path) {
            Ok(status) => { self.latest.lock().expect("status lock").status = Some(status); self.message = None; }
            Err(e) => self.message = Some(e),
        }
    }
}

impl eframe::App for StatusApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.choose_pending) { self.choose_folder(); }
        let (status, error) = { let l = self.latest.lock().expect("status lock"); (l.status.clone(), l.error.clone()) };
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(TITLE);
            if let Some(s) = &status { ui.weak(format!("v{} · {}", s.version, s.address)); }
            if let Some(e) = &error { ui.colored_label(ui.visuals().error_fg_color, e); }
            let Some(s) = status else { return };
            ui.add_space(8.0);

            section(ui, "Folder");
            match &s.folder {
                Some(f) => { ui.monospace(&f.path); }
                None => { ui.label("No folder chosen yet. Your notes live in a folder you pick, for example in your documents."); }
            }
            ui.horizontal(|ui| {
                if ui.button(if s.folder.is_some() { "Choose another folder…" } else { "Choose folder…" }).clicked() { self.choose_folder(); }
                if let Some(f) = &s.folder && ui.button("Open").clicked() && let Err(e) = opener::open(&f.path) { self.message = Some(e.to_string()); }
            });
            if let Some(m) = &self.message { ui.colored_label(ui.visuals().error_fg_color, m); }

            section(ui, "Web apps");
            if s.clients.is_empty() {
                ui.label("None connected.");
            } else {
                for c in &s.clients { ui.label(format!("{}  ·  {} s ago", c.origin, c.seconds_ago)); }
            }
            for origin in OFFICIAL_ORIGINS { ui.hyperlink_to(format!("Open {}", origin.trim_start_matches("https://")), *origin); }

            section(ui, "Plugins");
            match &s.uv {
                UvState::Locating => { ui.weak("Looking for uv…"); }
                UvState::Downloading => { ui.horizontal(|ui| { ui.spinner(); ui.label("Downloading uv (runs the plugins)…"); }); }
                UvState::Ready { version, .. } => { ui.weak(format!("uv {version}")); }
                UvState::Failed { error } => { ui.colored_label(ui.visuals().error_fg_color, format!("uv: {error}")); }
            }
            if s.plugins.is_empty() { ui.label("No plugins."); }
            for p in &s.plugins {
                let state = match p.state {
                    PluginState::Stopped => "stopped", PluginState::Starting => "starting…", PluginState::Ready => "ready",
                    PluginState::Running => "running", PluginState::Error => "error",
                };
                ui.label(format!("{} {}  ·  {state}", p.id, p.version));
                if let Some(e) = &p.error { ui.colored_label(ui.visuals().error_fg_color, e); }
            }

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(a) = &self.autostart && ui.checkbox(&mut self.autostart_on, "Start on login").changed() {
                    let result = if self.autostart_on { a.enable() } else { a.disable() };
                    if let Err(e) = result { self.message = Some(e.to_string()); self.autostart_on = !self.autostart_on; }
                }
                if ui.button("Quit connector").clicked() {
                    self.api.quit();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.strong(title);
    ui.separator();
}
