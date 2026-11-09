mod audio_in;
mod autostart;
mod commands;
mod dictation;
mod foreground;
mod http_client;
mod identity;
mod input;
mod meetings_run;
mod paste;
mod secrets;
mod store;

use bnflow_core::AppSettings;
use dictation::Engine;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

pub struct AppState {
    pub settings: Arc<Mutex<AppSettings>>,
    pub history: Arc<Mutex<Vec<bnflow_core::HistoryItem>>>,
    /// Extra sender so the dictation thread is not dropped when the hook reloads.
    #[allow(dead_code)]
    pub jobs: std::sync::mpsc::Sender<input::DictationJob>,
    pub hook: std::sync::mpsc::Sender<input::HookCmd>,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let settings = Arc::new(Mutex::new(store::load_settings()));
            let history = Arc::new(Mutex::new(store::load_history()));
            if let Ok(guard) = settings.lock() {
                let _ = autostart::set_launch_at_login(guard.launch_at_login);
            }
            let jobs = Engine::spawn(app.handle().clone(), settings.clone(), history.clone());
            let shortcuts = settings.lock().map(|s| s.shortcuts.clone()).unwrap_or_default();
            let hook = input::spawn(shortcuts, jobs.clone());
            app.manage(AppState { settings: settings.clone(), history, jobs, hook });
            meetings_run::spawn_loop(settings);

            let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
                .title("Business Flow")
                .inner_size(340.0, 72.0)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .shadow(false)
                .build()?;
            let _ = overlay.set_focusable(false);

            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            let open = MenuItem::with_id(app, "open", "Megnyitás", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Diktálás be/ki", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Kilépés", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &sep, &quit])?;
            if let Some(icon) = app.default_window_icon().cloned() {
            let tray = TrayIconBuilder::with_id("tray").icon(icon).menu(&menu).tooltip("Business Flow").build(app)?;
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "open" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "toggle" => {
                    if let Some(state) = app.try_state::<AppState>() {
                        let _ = state.hook.send(input::HookCmd::UiToggle);
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::save_settings,
            commands::set_api_key,
            commands::test_api_key,
            commands::clear_history,
            commands::paste_history_item,
            commands::toggle_dictation,
            commands::set_named_secret,
            commands::preview_meetings,
            commands::list_meeting_notes,
            commands::request_meeting_join,
            commands::polish_now,
            commands::learn_correction,
        ])
        .run(tauri::generate_context!())
        .expect("Business Flow failed to start");
}
