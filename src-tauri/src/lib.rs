mod am8;
mod devices;
mod engine;

use arc_swap::ArcSwap;
use devices::{Choice, Detect};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
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

/// Menu wording in the two interface languages.
struct Labels {
    show: &'static str,
    presets: [&'static str; 5],
    voices_title: &'static str,
    voices: [&'static str; 7],
    mute_off: &'static str,
    mute_on: &'static str,
    bypass_off: &'static str,
    bypass_on: &'static str,
    quit: &'static str,
    tooltip: &'static str,
}

fn labels(lang: &str) -> Labels {
    if lang == "ru" {
        Labels {
            show: "Открыть окно",
            voices_title: "Голоса",
            voices: [
                "Тёплый эфир",
                "Глубокий",
                "Яркий подкаст",
                "Объёмный",
                "Радио",
                "Робот",
                "Высокий",
            ],
            presets: [
                "Пресет: новичок",
                "Пресет: стрим",
                "Пресет: закадр",
                "Пресет: созвон",
                "Пресет: мой",
            ],
            mute_off: "Выключить микрофон",
            mute_on: "Включить микрофон",
            bypass_off: "Без обработки",
            bypass_on: "Вернуть обработку",
            quit: "Выйти",
            tooltip: "тембр",
        }
    } else {
        Labels {
            show: "Open window",
            voices_title: "Voices",
            voices: [
                "Warm broadcast",
                "Deep",
                "Bright podcast",
                "Thick",
                "Radio",
                "Robot",
                "High",
            ],
            presets: [
                "Preset: beginner",
                "Preset: stream",
                "Preset: voice-over",
                "Preset: call",
                "Preset: mine",
            ],
            mute_off: "Turn the microphone off",
            mute_on: "Turn the microphone on",
            bypass_off: "Bypass",
            bypass_on: "Resume processing",
            quit: "Quit",
            tooltip: "timbre",
        }
    }
}

struct TrayItems {
    show: MenuItem<tauri::Wry>,
    presets: Vec<MenuItem<tauri::Wry>>,
    voices: Submenu<tauri::Wry>,
    voice_items: Vec<MenuItem<tauri::Wry>>,
    mute: MenuItem<tauri::Wry>,
    bypass: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

struct AppState {
    shared: Arc<ArcSwap<UiState>>,
    engine: Mutex<Option<engine::EngineHandle>>,
    choice: Mutex<Choice>,
    tray: Mutex<Option<TrayItems>>,
    lang: Mutex<String>,
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
async fn diagnostics(app_state: State<'_, AppState>) -> Result<String, String> {
    let choice = app_state.choice.lock().unwrap().clone();
    // the report listens to the microphone buttons for a few seconds, so it must not run on the UI thread
    tauri::async_runtime::spawn_blocking(move || devices::report(&choice))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_state(state: UiState, app_state: State<'_, AppState>) {
    let lang = app_state.lang.lock().unwrap().clone();
    let l = labels(&lang);
    if let Some(t) = app_state.tray.lock().unwrap().as_ref() {
        let _ = t
            .mute
            .set_text(if state.muted { l.mute_on } else { l.mute_off });
        let _ = t.bypass.set_text(if state.bypass {
            l.bypass_on
        } else {
            l.bypass_off
        });
    }
    app_state.shared.store(Arc::new(state));
}

/// The window tells which language it runs in, so the tray menu follows.
#[tauri::command]
fn set_lang(lang: String, app_state: State<'_, AppState>) {
    *app_state.lang.lock().unwrap() = lang.clone();
    let l = labels(&lang);
    let st = app_state.shared.load();
    if let Some(t) = app_state.tray.lock().unwrap().as_ref() {
        let _ = t.show.set_text(l.show);
        for (item, text) in t.presets.iter().zip(l.presets.iter()) {
            let _ = item.set_text(*text);
        }
        let _ = t
            .mute
            .set_text(if st.muted { l.mute_on } else { l.mute_off });
        let _ = t
            .bypass
            .set_text(if st.bypass { l.bypass_on } else { l.bypass_off });
        let _ = t.quit.set_text(l.quit);
        let _ = t.voices.set_text(l.voices_title);
        for (item, text) in t.voice_items.iter().zip(l.voices.iter()) {
            let _ = item.set_text(*text);
        }
    }
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

fn build_tray(app: &AppHandle, lang: &str) -> tauri::Result<TrayItems> {
    let l = labels(lang);
    let show = MenuItem::with_id(app, "show", l.show, true, None::<&str>)?;
    let ids = [
        "preset:novice",
        "preset:stream",
        "preset:voiceover",
        "preset:call",
        "preset:mine",
    ];
    let mut presets = Vec::new();
    for (id, text) in ids.iter().zip(l.presets.iter()) {
        presets.push(MenuItem::with_id(app, *id, *text, true, None::<&str>)?);
    }
    let voice_ids = [
        "preset:warm",
        "preset:deep",
        "preset:bright",
        "preset:thick",
        "preset:radio",
        "preset:robot",
        "preset:chipmunk",
    ];
    let mut voice_items = Vec::new();
    for (id, text) in voice_ids.iter().zip(l.voices.iter()) {
        voice_items.push(MenuItem::with_id(app, *id, *text, true, None::<&str>)?);
    }
    let voice_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = voice_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let voices = Submenu::with_items(app, l.voices_title, true, &voice_refs)?;
    let mute = MenuItem::with_id(app, "mute", l.mute_off, true, None::<&str>)?;
    let bypass = MenuItem::with_id(app, "bypass", l.bypass_off, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &sep1,
            &presets[0],
            &presets[1],
            &presets[2],
            &presets[3],
            &presets[4],
            &voices,
            &sep2,
            &mute,
            &bypass,
            &sep3,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(l.tooltip)
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
    Ok(TrayItems {
        show,
        presets,
        voices,
        voice_items,
        mute,
        bypass,
        quit,
    })
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
            lang: Mutex::new("en".to_string()),
        })
        .invoke_handler(tauri::generate_handler![
            detect_devices,
            diagnostics,
            set_state,
            set_lang,
            restart_engine,
            set_hotkey,
            set_autostart,
            open_url
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let items = build_tray(&handle, "en")?;
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
