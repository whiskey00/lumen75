//! The S75 Pro's commands for the app. The keyboard can't report its settings back (only
//! per-key colours, over USB), so the app keeps what you set in its own settings.

use super::device::S75;
use super::protocol::{Entry, Lighting, Settings, Step, BAR_MODES, EFFECTS, MOUSE};
use crate::keymap::Key;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// What the app last set on the S75 Pro (saved with the app's settings).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub lighting: Lighting,
    /// The side light bar's mode (see `BAR_MODES`).
    pub bar: u8,
    pub settings: Settings,
    /// Per-key colours by key index (128 slots; unused ones stay black).
    pub perkey: Vec<[u8; 3]>,
    /// Brightness of the per-key mode, 0-5.
    pub perkey_brightness: u8,
    /// Remapped keys of the base and Fn layers: (key index, entry). Keys not listed keep
    /// their own function.
    pub keymap: [Vec<(u8, Entry)>; 2],
    /// Your macros; a macro key plays the one at its position.
    pub macros: Vec<Macro>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<Step>,
    /// The steps as you typed them, to show and edit again.
    #[serde(default)]
    pub text: String,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            lighting: Lighting::default(),
            bar: 0x20,
            settings: Settings::default(),
            perkey: vec![[0, 0, 0]; 128],
            perkey_brightness: 5,
            keymap: [Vec::new(), Vec::new()],
            macros: Vec::new(),
        }
    }
}

/// The colours of the S75 Pro's keys, from a 128-slot list.
fn key_colors(slots: &[[u8; 3]]) -> Vec<(u8, [u8; 3])> {
    super::layout::KEYS.iter().map(|k| (k.index, slots.get(k.index as usize).copied().unwrap_or([0, 0, 0]))).collect()
}

#[derive(Serialize)]
pub struct Catalog {
    keys: Vec<Key>,
    /// (mode, name, controls: 1 brightness, 2 speed, 4 left/right, 8 up/down, 0x10
    /// colourful, 0x20 colour)
    effects: Vec<(u8, &'static str, u8)>,
    bar_modes: Vec<(u8, &'static str)>,
    sleep: Vec<(u8, &'static str)>,
    /// Keys a key can send: (name, label, HID usage).
    hid: Vec<(&'static str, &'static str, u8)>,
    /// (name, label, modifier bit)
    modifiers: Vec<(&'static str, &'static str, u8)>,
    /// Media and app keys: (label, consumer usage).
    media: Vec<(&'static str, u16)>,
    /// Mouse actions: (label, the entry's two bytes after its type).
    mouse: Vec<(&'static str, [u8; 2])>,
}

/// Consumer-control usages (USB HID usage tables, page 0x0C).
const MEDIA: [(&str, u16); 12] = [
    ("Play / pause", 0xcd),
    ("Stop", 0xb7),
    ("Previous track", 0xb6),
    ("Next track", 0xb5),
    ("Mute", 0xe2),
    ("Volume up", 0xe9),
    ("Volume down", 0xea),
    ("Calculator", 0x192),
    ("Mail", 0x18a),
    ("Browser home", 0x223),
    ("Search", 0x221),
    ("This PC", 0x194),
];

#[derive(Serialize)]
pub struct State {
    /// How it's connected now ("USB cable" / "2.4G dongle"), or None.
    connection: Option<&'static str>,
    saved: Saved,
    catalog: Catalog,
}

fn catalog() -> Catalog {
    Catalog {
        keys: super::layout::KEYS.to_vec(),
        effects: EFFECTS.to_vec(),
        bar_modes: BAR_MODES.to_vec(),
        sleep: vec![(0, "Never"), (1, "1 minute"), (2, "5 minutes"), (3, "30 minutes")],
        hid: crate::keymap::HID.to_vec(),
        modifiers: crate::keymap::MODIFIERS.to_vec(),
        media: MEDIA.to_vec(),
        mouse: MOUSE.to_vec(),
    }
}

fn connection() -> Option<&'static str> {
    let f = super::winlink::find();
    if f.usb.is_some() {
        Some("USB cable")
    } else if f.dongle.is_some() {
        Some("2.4G dongle")
    } else {
        None
    }
}

/// Run `f` on the S75 Pro, opening it first; on an error it's closed, so the next call opens
/// it again.
pub(crate) async fn with_s75<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(&mut S75) -> crate::device::Result<T> + Send + 'static,
) -> Result<T, String> {
    let shared = app.state::<AppState>().s75.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = shared.lock().map_err(|_| "S75 Pro lock poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(super::winlink::open_s75().map_err(|e| e.0)?);
        }
        let r = f(guard.as_mut().unwrap());
        if r.is_err() {
            *guard = None;
        }
        r.map_err(|e| e.0)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn saved(app: &AppHandle) -> Saved {
    app.state::<AppState>().config.lock().unwrap().s75.clone()
}

#[tauri::command]
pub async fn s75_get(app: AppHandle) -> State {
    let connection = tauri::async_runtime::spawn_blocking(connection).await.unwrap_or(None);
    State { connection, saved: saved(&app), catalog: catalog() }
}

#[tauri::command]
pub async fn s75_set_lighting(app: AppHandle, lighting: Lighting) -> Result<Saved, String> {
    with_s75(&app, move |kb| kb.set_lighting(&lighting)).await?;
    Ok(crate::app::with_config(&app, |c| {
        c.s75.lighting = lighting;
        c.s75.clone()
    }))
}

#[tauri::command]
pub async fn s75_set_bar(app: AppHandle, mode: u8) -> Result<Saved, String> {
    with_s75(&app, move |kb| kb.set_bar(mode)).await?;
    Ok(crate::app::with_config(&app, |c| {
        c.s75.bar = mode;
        c.s75.clone()
    }))
}

#[tauri::command]
pub async fn s75_set_settings(app: AppHandle, settings: Settings) -> Result<Saved, String> {
    with_s75(&app, move |kb| kb.set_settings(&settings)).await?;
    Ok(crate::app::with_config(&app, |c| {
        c.s75.settings = settings;
        c.s75.clone()
    }))
}

/// Per-key colours (128 slots by key index): switches the keyboard to its per-key mode.
#[tauri::command]
pub async fn s75_set_perkey(app: AppHandle, colors: Vec<[u8; 3]>, brightness: u8) -> Result<Saved, String> {
    let keys = key_colors(&colors);
    let b = brightness.min(5);
    with_s75(&app, move |kb| kb.set_perkey(&keys, b)).await?;
    Ok(crate::app::with_config(&app, |c| {
        c.s75.perkey = colors;
        c.s75.perkey_brightness = b;
        c.s75.lighting.mode = super::protocol::USER_LIGHT_MODE;
        c.s75.clone()
    }))
}

/// The per-key colours stored on the keyboard (USB only), saved as the app's copy too.
#[tauri::command]
pub async fn s75_read_perkey(app: AppHandle) -> Result<Saved, String> {
    let indexes: Vec<u8> = super::layout::KEYS.iter().map(|k| k.index).collect();
    let read = with_s75(&app, move |kb| kb.read_perkey(&indexes)).await?;
    Ok(crate::app::with_config(&app, |c| {
        let mut slots = vec![[0u8; 3]; 128];
        for (i, rgb) in read {
            slots[i as usize] = rgb;
        }
        c.s75.perkey = slots;
        c.s75.clone()
    }))
}

/// Remap one key on a layer (0 base, 1 Fn), or give it back its own function (`entry` None).
/// The keyboard takes a whole layer at a time, so the layer's other remaps go with it.
#[tauri::command]
pub async fn s75_set_key(app: AppHandle, layer: u8, index: u8, entry: Option<Entry>) -> Result<Saved, String> {
    let l = (layer as usize).min(1);
    let mut map = saved(&app).keymap[l].clone();
    map.retain(|(i, _)| *i != index);
    if let Some(e) = entry {
        map.push((index, e));
    }
    map.sort_by_key(|(i, _)| *i);
    let send = map.clone();
    with_s75(&app, move |kb| kb.set_keymap(l == 1, &send)).await?;
    Ok(crate::app::with_config(&app, |c| {
        c.s75.keymap[l] = map;
        c.s75.clone()
    }))
}

/// Every macro at once (the keyboard stores them as one block).
#[tauri::command]
pub async fn s75_set_macros(app: AppHandle, macros: Vec<Macro>) -> Result<Saved, String> {
    let steps: Vec<Vec<Step>> = macros.iter().map(|m| m.steps.clone()).collect();
    with_s75(&app, move |kb| kb.set_macros(&steps)).await?;
    Ok(crate::app::with_config(&app, |c| {
        c.s75.macros = macros;
        c.s75.clone()
    }))
}

/// The first frame of a picture as the screen would show it, as a data: URL.
#[tauri::command]
pub async fn s75_screen_preview(path: String, fit: crate::imagelight::Fit) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let png = super::screen::preview_png(std::path::Path::new(&path), fit)?;
        Ok(format!("data:image/png;base64,{}", super::screen::base64(&png)))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Put a GIF or picture on the screen (USB only). Progress arrives as "s75-screen-progress"
/// (0..1). Returns how many frames went up.
#[tauri::command]
pub async fn s75_upload_screen(app: AppHandle, path: String, fit: crate::imagelight::Fit) -> Result<usize, String> {
    let (image, frames) = tauri::async_runtime::spawn_blocking(move || super::screen::image(std::path::Path::new(&path), fit))
        .await
        .map_err(|e| e.to_string())??;
    let a = app.clone();
    with_s75(&app, move |kb| {
        kb.upload_screen(&image, 1, |done| {
            let _ = a.emit("s75-screen-progress", done);
        })
    })
    .await?;
    Ok(frames)
}

/// Set the keyboard's clock to this PC's time.
#[tauri::command]
pub async fn s75_sync_clock(app: AppHandle) -> Result<(), String> {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let st = unsafe {
        let mut st = std::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    let t = super::protocol::Time {
        year: st.wYear,
        month: st.wMonth as u8,
        day: st.wDay as u8,
        hour: st.wHour as u8,
        minute: st.wMinute as u8,
        second: st.wSecond as u8,
        weekday: st.wDayOfWeek as u8,
    };
    with_s75(&app, move |kb| kb.set_time(&t)).await
}

/// Battery percentage over the dongle (None on the cable).
#[tauri::command]
pub async fn s75_battery(app: AppHandle) -> Result<Option<u8>, String> {
    with_s75(&app, |kb| kb.battery()).await
}
