//! The desktop app: one connector per user, shown as a tray icon, with a status window (a separate
//! `status` process, so closing it never touches the server) for choosing the folder.

pub mod status;

use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::paths::Paths;
use crate::server::{CONTROL_HEADER, FolderInfo, UiRequest};
use crate::{Opts, Server};

pub const ICON: &[u8] = include_bytes!("../../assets/icon.png");
pub const TITLE: &str = "Gestalts Connector";
/// Passed to the status window so it can use `/control/*`.
pub const TOKEN_ENV: &str = "GESTALTS_CONTROL_TOKEN";

pub enum Msg {
    Ui(UiRequest),
    Menu(MenuId),
}

pub fn run(opts: &Opts) -> Result<(), String> {
    let result = (|| {
        let paths = Paths::new(opts.profile.as_deref())?;
        if hand_over_to_running(opts, &paths)? { return Ok(()); }
        platform::run(opts.clone(), paths)
    })();
    // launched from the app menu, nobody sees stderr
    if let Err(e) = &result { let _ = rfd::MessageDialog::new().set_title(TITLE).set_description(e).set_level(rfd::MessageLevel::Error).show(); }
    result
}

/// If our connector already runs on the port, ask it to show its window and report `true`.
fn hand_over_to_running(opts: &Opts, paths: &Paths) -> Result<bool, String> {
    let base = format!("http://127.0.0.1:{}", opts.port);
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?;
    let Ok(response) = client.get(format!("{base}/health")).send() else { return Ok(false) };
    let health: serde_json::Value = response.json().unwrap_or_default();
    if health["app"] != "gestalts-connector" { return Err(format!("port {} is used by another program", opts.port)); }
    let token = std::fs::read_to_string(paths.control_token_file()).unwrap_or_default();
    match client.post(format!("{base}/control/show")).header(CONTROL_HEADER, token.trim()).send() {
        Ok(r) if r.status().is_success() => Ok(true),
        _ => Err(format!("another Gestalts connector (a different install or profile) already runs on port {}; quit it first", opts.port)),
    }
}

/// The tray, the server and the status window process.
struct App {
    opts: Opts,
    server: Option<Server>,
    _tray: Option<TrayIcon>,
    folder_item: MenuItem,
    show: MenuId,
    choose: MenuId,
    quit: MenuId,
    window: Option<Child>,
}

impl App {
    fn start(opts: Opts, paths: Paths, send: Arc<dyn Fn(Msg) + Send + Sync>) -> Result<App, String> {
        let folder = crate::remembered_folder(&paths);
        let label = folder_label(folder.as_ref().map(|f| f.info()));
        let ui = { let send = send.clone(); Arc::new(move |r: UiRequest| send(Msg::Ui(r))) };
        let server = crate::start_server(&opts, paths, folder, true, ui)?;
        { let send = send.clone(); server.spawn(async move { crate::stop_requested().await; send(Msg::Ui(UiRequest::Quit)); }); }

        let folder_item = MenuItem::new(&label, false, None);
        let show = MenuItem::new("Show status…", true, None);
        let choose = MenuItem::new("Choose folder…", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&folder_item, &PredefinedMenuItem::separator(), &show, &choose, &PredefinedMenuItem::separator(), &quit])
            .map_err(|e| e.to_string())?;
        { let send = send.clone(); MenuEvent::set_event_handler(Some(move |e: MenuEvent| send(Msg::Menu(e.id)))); }
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e { send(Msg::Ui(UiRequest::ShowStatus)); }
        }));
        let icon = eframe::icon_data::from_png_bytes(ICON).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_id("gestalts-connector")
            .with_title(TITLE)
            .with_tooltip(format!("{TITLE}: {label}"))
            .with_icon(tray_icon::Icon::from_rgba(icon.rgba, icon.width, icon.height).map_err(|e| e.to_string())?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build();
        // without a tray (e.g. GNOME without an extension) the window, and launching the app again, still work
        let tray = tray.map_err(|e| eprintln!("gestalts-connector: no tray icon: {e}")).ok();
        let open = !opts.background || label == NO_FOLDER;
        let mut app = App { opts, server: Some(server), _tray: tray, folder_item, show: show.id().clone(), choose: choose.id().clone(), quit: quit.id().clone(), window: None };
        // launching the app shows it; only the login autostart starts it silently
        if open { app.open_window(false); }
        Ok(app)
    }

    /// Handles one message; `false` means quit.
    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Ui(UiRequest::ShowStatus) => self.open_window(false),
            Msg::Ui(UiRequest::FolderChanged(info)) => {
                let label = folder_label(info.as_ref());
                self.folder_item.set_text(&label);
                if let Some(tray) = &self._tray { let _ = tray.set_tooltip(Some(format!("{TITLE}: {label}"))); }
            }
            Msg::Ui(UiRequest::Quit) => return false,
            Msg::Menu(id) if id == self.show => self.open_window(false),
            Msg::Menu(id) if id == self.choose => self.open_window(true),
            Msg::Menu(id) if id == self.quit => return false,
            Msg::Menu(_) => {}
        }
        true
    }

    /// (Re)opens the status window; reopening brings it to the front.
    fn open_window(&mut self, choose: bool) {
        if let Some(mut old) = self.window.take() { let _ = old.kill(); let _ = old.wait(); }
        let Some(server) = &self.server else { return };
        let exe = match own_exe() { Ok(exe) => exe, Err(e) => { eprintln!("gestalts-connector: {e}"); return } };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("status").arg("--port").arg(self.opts.port.to_string()).env(TOKEN_ENV, &server.control_token);
        if let Some(profile) = &self.opts.profile { cmd.arg("--profile").arg(profile); }
        if choose { cmd.arg("--choose"); }
        match cmd.spawn() {
            Ok(child) => self.window = Some(child),
            Err(e) => eprintln!("gestalts-connector: cannot open the status window: {e}"),
        }
    }

    fn shutdown(&mut self) {
        if let Some(mut w) = self.window.take() { let _ = w.kill(); }
        if let Some(server) = self.server.take() { server.stop(); }
    }
}

const NO_FOLDER: &str = "No folder chosen";

/// This program's path; after an update replaced the file while we run (Linux reports
/// "<path> (deleted)"), the new file at the same path.
fn own_exe() -> std::io::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    if exe.exists() { return Ok(exe); }
    let path = exe.to_string_lossy();
    Ok(path.strip_suffix(" (deleted)").map_or(exe.clone(), std::path::PathBuf::from))
}

fn folder_label(info: Option<&FolderInfo>) -> String {
    info.map_or_else(|| NO_FOLDER.to_owned(), |f| format!("Folder: {}", f.path))
}

/// Linux: the tray talks D-Bus on its own thread, so the main thread just handles messages.
#[cfg(target_os = "linux")]
mod platform {
    use super::*;

    pub fn run(opts: Opts, paths: Paths) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        let tx = Mutex::new(tx);
        let mut app = App::start(opts, paths, Arc::new(move |m| { let _ = tx.lock().expect("sender lock").send(m); }))?;
        while let Ok(msg) = rx.recv() {
            if !app.handle(msg) { break; }
        }
        app.shutdown();
        Ok(())
    }
}

/// macOS and Windows: the tray needs an event loop on the main thread.
#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;
    use tao::event::{Event, StartCause};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};

    pub fn run(opts: Opts, paths: Paths) -> Result<(), String> {
        let event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
        let proxy = Mutex::new(event_loop.create_proxy());
        let send: Arc<dyn Fn(Msg) + Send + Sync> = Arc::new(move |m| { let _ = proxy.lock().expect("proxy lock").send_event(m); });
        let mut start = Some((opts, paths, send));
        let mut app: Option<App> = None;
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                // macOS wants the tray created once the loop runs
                Event::NewEvents(StartCause::Init) => {
                    let (opts, paths, send) = start.take().expect("started once");
                    match App::start(opts, paths, send) {
                        Ok(a) => app = Some(a),
                        Err(e) => {
                            let _ = rfd::MessageDialog::new().set_title(TITLE).set_description(&e).set_level(rfd::MessageLevel::Error).show();
                            *control_flow = ControlFlow::ExitWithCode(1);
                        }
                    }
                }
                Event::UserEvent(msg) => {
                    if let Some(a) = app.as_mut() && !a.handle(msg) {
                        a.shutdown();
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => {}
            }
        });
    }
}
