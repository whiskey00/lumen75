//! The S75 Pro's commands for the app. The keyboard can't report its settings back (only
//! per-key colours, over USB), so the app keeps what you set in its own settings.

use super::device::S75;
use super::protocol::{Lighting, Settings, BAR_MODES, EFFECTS};
use crate::keymap::Key;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

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
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            lighting: Lighting::default(),
            bar: 0x20,
            settings: Settings::default(),
            perkey: vec![[0, 0, 0]; 128],
            perkey_brightness: 5,
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
}

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

/// Battery percentage over the dongle (None on the cable).
#[tauri::command]
pub async fn s75_battery(app: AppHandle) -> Result<Option<u8>, String> {
    with_s75(&app, |kb| kb.battery()).await
}
