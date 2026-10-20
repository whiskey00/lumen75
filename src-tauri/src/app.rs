//! App-level things: saved settings, the tray icon, starting with Windows, and the
//! animations (which stream from the computer, so they live as long as the app does).

use crate::{anim, AppState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

/// Passed by the autostart entry: start hidden in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub animation: anim::Settings,
    /// Animation running when the app last closed: it starts again with the app.
    pub animation_on: bool,
    /// Pause animations after this many minutes without input (0 = never), so the keyboard
    /// can go to sleep.
    pub idle_pause_minutes: u32,
    /// Closing the window keeps the app running in the tray.
    pub close_to_tray: bool,
    /// Your saved animations, by name.
    pub anim_presets: Vec<AnimPreset>,
    /// Your saved per-key colour layouts, by name.
    pub perkey_presets: Vec<PerkeyPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimPreset {
    pub name: String,
    pub settings: anim::Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerkeyPreset {
    pub name: String,
    /// 126 colours by matrix index.
    pub colors: Vec<crate::protocol::Rgb>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            animation: anim::Settings::default(),
            animation_on: false,
            idle_pause_minutes: 5,
            close_to_tray: true,
            anim_presets: Vec::new(),
            perkey_presets: Vec::new(),
        }
    }
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    config_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(app: &AppHandle, c: &AppConfig) {
    let Some(path) = config_path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(c) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

fn with_config<T>(app: &AppHandle, f: impl FnOnce(&mut AppConfig) -> T) -> T {
    let state = app.state::<AppState>();
    let mut c = state.config.lock().unwrap();
    let r = f(&mut c);
    save_config(app, &c);
    r
}

// ------------------------------------------------------------ animations

#[derive(Serialize, Clone)]
pub struct AnimState {
    running: bool,
    settings: anim::Settings,
}

fn anim_state(app: &AppHandle) -> AnimState {
    let state = app.state::<AppState>();
    let running = state.anim.lock().map(|a| a.is_some()).unwrap_or(false);
    let settings = state.config.lock().unwrap().animation.clone();
    AnimState { running, settings }
}

fn notify(app: &AppHandle) {
    let s = anim_state(app);
    if let Some(item) = app.state::<AppState>().tray_anim.lock().unwrap().as_ref() {
        let _ = item.set_checked(s.running);
    }
    let _ = app.emit("anim-state", s);
}

/// Start (or keep running with new settings) the animation; stops the audio lighting, since
/// only one thing can stream frames at a time.
pub fn start_animation(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(h) = state.audio.lock().unwrap().take() {
        h.stop();
        let _ = app.emit("audio-stopped", ());
    }
    let (settings, idle) = {
        let c = state.config.lock().unwrap();
        (c.animation.clone(), c.idle_pause_minutes * 60)
    };
    {
        let mut slot = state.anim.lock().unwrap();
        if let Some(h) = slot.as_ref() {
            *h.settings.lock().unwrap() = settings;
            h.idle_pause.store(idle, std::sync::atomic::Ordering::Relaxed);
        } else {
            let emitter = app.clone();
            *slot = Some(anim::run::start(state.kb.clone(), crate::open_keyboard, settings, idle, move |ev| {
                let _ = match ev {
                    anim::run::Event::Frame(f) => emitter.emit("anim-frame", f),
                    anim::run::Event::Error(e) => emitter.emit("anim-error", e),
                };
            }));
        }
    }
    with_config(app, |c| c.animation_on = true);
    notify(app);
}

pub fn stop_animation(app: &AppHandle, remember: bool) {
    let handle = app.state::<AppState>().anim.lock().unwrap().take();
    if let Some(h) = handle {
        h.stop();
    }
    if remember {
        with_config(app, |c| c.animation_on = false);
    }
    notify(app);
}

#[tauri::command]
pub fn anim_get(app: AppHandle) -> AnimState {
    anim_state(&app)
}

#[tauri::command]
pub async fn anim_start(app: AppHandle, settings: anim::Settings) -> Result<AnimState, String> {
    if crate::win::vendor_app_running() {
        return Err("The official AULA app (OemDrv.exe) is running. Quit it first (also from the tray).".into());
    }
    with_config(&app, |c| c.animation = settings);
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || start_animation(&a)).await.map_err(|e| e.to_string())?;
    Ok(anim_state(&app))
}

/// New settings: saved, and applied at once if the animation runs.
#[tauri::command]
pub fn anim_update(app: AppHandle, settings: anim::Settings) {
    with_config(&app, |c| c.animation = settings.clone());
    if let Some(h) = app.state::<AppState>().anim.lock().unwrap().as_ref() {
        *h.settings.lock().unwrap() = settings;
    }
}

#[tauri::command]
pub async fn anim_stop(app: AppHandle) -> Result<AnimState, String> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || stop_animation(&a, true)).await.map_err(|e| e.to_string())?;
    Ok(anim_state(&app))
}

// ------------------------------------------------------------ your presets

#[derive(Serialize)]
pub struct Presets {
    animations: Vec<AnimPreset>,
    perkey: Vec<PerkeyPreset>,
}

fn presets(c: &AppConfig) -> Presets {
    Presets { animations: c.anim_presets.clone(), perkey: c.perkey_presets.clone() }
}

fn clean_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Give the preset a name.".into());
    }
    Ok(n.chars().take(40).collect())
}

/// Insert or replace (same name, ignoring case) and keep the list sorted by name.
fn upsert<T>(list: &mut Vec<T>, item: T, name_of: impl Fn(&T) -> &str) {
    let name = name_of(&item).to_lowercase();
    match list.iter().position(|x| name_of(x).to_lowercase() == name) {
        Some(i) => list[i] = item,
        None => list.push(item),
    }
    list.sort_by_key(|x| name_of(x).to_lowercase());
}

#[tauri::command]
pub fn presets_get(app: AppHandle) -> Presets {
    presets(&app.state::<AppState>().config.lock().unwrap())
}

/// Save an animation under a name (replaces one with the same name).
#[tauri::command]
pub fn anim_preset_save(app: AppHandle, name: String, settings: anim::Settings) -> Result<Presets, String> {
    let name = clean_name(&name)?;
    if settings.colors.is_empty() || settings.colors.len() > 7 {
        return Err("An animation needs 1 to 7 colours.".into());
    }
    Ok(with_config(&app, |c| {
        upsert(&mut c.anim_presets, AnimPreset { name, settings }, |p| &p.name);
        presets(c)
    }))
}

/// Save a per-key layout under a name (replaces one with the same name).
#[tauri::command]
pub fn perkey_preset_save(app: AppHandle, name: String, colors: Vec<crate::protocol::Rgb>) -> Result<Presets, String> {
    let name = clean_name(&name)?;
    if colors.len() != crate::protocol::MATRIX_LEN {
        return Err("A per-key layout needs 126 colours.".into());
    }
    Ok(with_config(&app, |c| {
        upsert(&mut c.perkey_presets, PerkeyPreset { name, colors }, |p| &p.name);
        presets(c)
    }))
}

/// Delete a preset: `kind` is "animation" or "perkey".
#[tauri::command]
pub fn preset_delete(app: AppHandle, kind: String, name: String) -> Result<Presets, String> {
    let n = name.to_lowercase();
    with_config(&app, |c| {
        match kind.as_str() {
            "animation" => c.anim_presets.retain(|p| p.name.to_lowercase() != n),
            "perkey" => c.perkey_presets.retain(|p| p.name.to_lowercase() != n),
            _ => return Err(format!("unknown preset kind {kind}")),
        }
        Ok(presets(c))
    })
}

// ------------------------------------------------------------ app settings

#[derive(Serialize)]
pub struct AppSettings {
    start_with_windows: bool,
    close_to_tray: bool,
    idle_pause_minutes: u32,
}

#[derive(Deserialize)]
pub struct AppSettingsChange {
    start_with_windows: Option<bool>,
    close_to_tray: Option<bool>,
    idle_pause_minutes: Option<u32>,
}

fn app_settings(app: &AppHandle) -> AppSettings {
    let c = app.state::<AppState>().config.lock().unwrap().clone();
    AppSettings {
        start_with_windows: app.autolaunch().is_enabled().unwrap_or(false),
        close_to_tray: c.close_to_tray,
        idle_pause_minutes: c.idle_pause_minutes,
    }
}

#[tauri::command]
pub fn app_settings_get(app: AppHandle) -> AppSettings {
    app_settings(&app)
}

#[tauri::command]
pub fn app_settings_set(app: AppHandle, change: AppSettingsChange) -> Result<AppSettings, String> {
    if let Some(on) = change.start_with_windows {
        let r = if on { app.autolaunch().enable() } else { app.autolaunch().disable() };
        r.map_err(|e| format!("Start with Windows: {e}"))?;
    }
    with_config(&app, |c| {
        if let Some(v) = change.close_to_tray {
            c.close_to_tray = v;
        }
        if let Some(v) = change.idle_pause_minutes {
            c.idle_pause_minutes = v.min(240);
        }
    });
    if let Some(m) = change.idle_pause_minutes {
        if let Some(h) = app.state::<AppState>().anim.lock().unwrap().as_ref() {
            h.idle_pause.store(m.min(240) * 60, std::sync::atomic::Ordering::Relaxed);
        }
    }
    Ok(app_settings(&app))
}

// ------------------------------------------------------------ window and tray

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Stop streaming, then exit (the keyboard goes back to its own effect).
pub fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(h) = state.anim.lock().unwrap().take() {
        h.stop();
    }
    if let Some(h) = state.audio.lock().unwrap().take() {
        h.stop();
    }
    app.exit(0);
}

fn build_tray(app: &AppHandle, animation_on: bool) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open AULA Studio", true, None::<&str>)?;
    let anim_item = CheckMenuItem::with_id(app, "animation", "Animation", true, animation_on, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &anim_item, &sep, &quit_item])?;
    *app.state::<AppState>().tray_anim.lock().unwrap() = Some(anim_item);
    // A tray-sized version of the logo (tray-icon.svg): the full app icon is a blur at 16-24 px.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("AULA Studio")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "animation" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    let running = app.state::<AppState>().anim.lock().unwrap().is_some();
                    if running {
                        stop_animation(&app, true);
                    } else {
                        start_animation(&app);
                    }
                });
            }
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Load the settings, add the tray icon, show the window unless autostarted, and resume the
/// animation if it was running when the app last closed.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let config = load_config(app);
    let resume = config.animation_on;
    *app.state::<AppState>().config.lock().unwrap() = config;
    build_tray(app, false)?;
    if !std::env::args().any(|a| a == MINIMIZED_ARG) {
        show_window(app);
    }
    if resume {
        let app = app.clone();
        std::thread::spawn(move || start_animation(&app));
    }
    Ok(())
}

/// Window close: keep running in the tray, or quit.
pub fn on_close_requested(window: &tauri::Window<Wry>, api: &tauri::CloseRequestApi) {
    let app = window.app_handle();
    let to_tray = app.state::<AppState>().config.lock().map(|c| c.close_to_tray).unwrap_or(true);
    if to_tray {
        api.prevent_close();
        let _ = window.hide();
    } else {
        quit(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_under_an_existing_name_replaces_it() {
        let mut list: Vec<(String, u8)> = Vec::new();
        upsert(&mut list, ("Sunset".into(), 1), |p| &p.0);
        upsert(&mut list, ("aqua".into(), 2), |p| &p.0);
        upsert(&mut list, ("SUNSET".into(), 3), |p| &p.0); // same name, other case
        assert_eq!(list, vec![("aqua".to_string(), 2), ("SUNSET".to_string(), 3)]);
    }

    #[test]
    fn preset_names() {
        assert!(clean_name("   ").is_err());
        assert_eq!(clean_name("  My red  ").unwrap(), "My red");
        assert_eq!(clean_name(&"x".repeat(80)).unwrap().len(), 40);
    }

    #[test]
    fn old_settings_files_still_load() {
        // written before presets existed: the preset lists default to empty
        let c: AppConfig = serde_json::from_str(r#"{"animation_on": true, "idle_pause_minutes": 10}"#).unwrap();
        assert!(c.animation_on && c.anim_presets.is_empty() && c.perkey_presets.is_empty());
        assert_eq!(c.idle_pause_minutes, 10);
    }
}
