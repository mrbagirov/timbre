mod devices;
mod engine;

use arc_swap::ArcSwap;
use devices::{Choice, Detect};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_opener::OpenerExt as _;

/// What the window sends: the chain parameters and the two switches.
#[derive(Deserialize, Clone, Default)]
pub struct UiState {
    pub params: timbre_dsp::Params,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub bypass: bool,
}

struct TrayItems {
    mute: MenuItem<tauri::Wry>,
    bypass: MenuItem<tauri::Wry>,
}

struct AppState {
    shared: Arc<ArcSwap<UiState>>,
    engine: Mutex<Option<engine::EngineHandle>>,
    choice: Mutex<Choice>,
    tray: Mutex<Option<TrayItems>>,
}

fn restart(app: &AppHandle, state: &AppState) {
    let mut guard = state.engine.lock().unwrap();
    if let Some(old) = guard.take() {
        old.stop();
    }
    let choice = state.choice.lock().unwrap().clone();
    *guard = Some(engine::start(app.clone(), state.shared.clone(), choice));
}

#[tauri::command]
fn detect_devices() -> Detect {
    devices::detect()
}

#[tauri::command]
fn set_state(state: UiState, app_state: State<'_, AppState>) {
    if let Some(t) = app_state.tray.lock().unwrap().as_ref() {
        let _ = t.mute.set_text(if state.muted {
            "Включить микрофон"
        } else {
            "Выключить микрофон"
        });
        let _ = t.bypass.set_text(if state.bypass {
            "Вернуть обработку"
        } else {
            "Без обработки"
        });
    }
    app_state.shared.store(Arc::new(state));
}

#[tauri::command]
fn restart_engine(
    app: AppHandle,
    input: Option<String>,
    output: Option<String>,
    app_state: State<'_, AppState>,
) {
    {
        let mut c = app_state.choice.lock().unwrap();
        if let Some(i) = input {
            c.input = i;
        }
        if let Some(o) = output {
            c.output = o;
        }
    }
    restart(&app, &app_state);
}

#[tauri::command]
fn set_hotkey(app: AppHandle, accel: String) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let sc: Shortcut = accel.parse().map_err(|e| format!("{e}"))?;
    gs.on_shortcut(sc, |app, _sc, ev| {
        if ev.state == ShortcutState::Pressed {
            let _ = app.emit("hotkey-mute", ());
        }
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https links are opened".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<TrayItems> {
    let show = MenuItem::with_id(app, "show", "Открыть окно", true, None::<&str>)?;
    let p_novice = MenuItem::with_id(app, "preset:novice", "Пресет: новичок", true, None::<&str>)?;
    let p_stream = MenuItem::with_id(app, "preset:stream", "Пресет: стрим", true, None::<&str>)?;
    let p_voice = MenuItem::with_id(
        app,
        "preset:voiceover",
        "Пресет: закадр",
        true,
        None::<&str>,
    )?;
    let p_call = MenuItem::with_id(app, "preset:call", "Пресет: созвон", true, None::<&str>)?;
    let p_mine = MenuItem::with_id(app, "preset:mine", "Пресет: мой", true, None::<&str>)?;
    let mute = MenuItem::with_id(app, "mute", "Выключить микрофон", true, None::<&str>)?;
    let bypass = MenuItem::with_id(app, "bypass", "Без обработки", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show, &sep1, &p_novice, &p_stream, &p_voice, &p_call, &p_mine, &sep2, &mute, &bypass,
            &sep3, &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("тембр")
        .menu(&menu)
        .show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "mute" => {
                let _ = app.emit("tray-flag", "muted");
            }
            "bypass" => {
                let _ = app.emit("tray-flag", "bypass");
            }
            "quit" => {
                if let Some(st) = app.try_state::<AppState>() {
                    if let Some(e) = st.engine.lock().unwrap().take() {
                        e.stop();
                    }
                }
                app.exit(0);
            }
            id if id.starts_with("preset:") => {
                let _ = app.emit("tray-preset", id.trim_start_matches("preset:"));
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(TrayItems { mute, bypass })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(AppState {
            shared: Arc::new(ArcSwap::from_pointee(UiState::default())),
            engine: Mutex::new(None),
            choice: Mutex::new(Choice::default()),
            tray: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            detect_devices,
            set_state,
            restart_engine,
            set_hotkey,
            set_autostart,
            open_url
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let items = build_tray(&handle)?;
            let state = app.state::<AppState>();
            *state.tray.lock().unwrap() = Some(items);
            restart(&handle, &state);
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running timbre");
}
