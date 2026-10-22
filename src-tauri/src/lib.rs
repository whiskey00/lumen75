pub mod anim;
#[cfg(windows)]
mod app;
pub mod audio;
pub mod device;
pub mod discord;
pub mod gamelight;
pub mod imagelight;
pub mod keymap;
pub mod league;
pub mod macros;
pub mod match_watch;
pub mod protocol;
pub mod s75;
#[cfg(windows)]
pub mod win;

use device::{derr, Keyboard, Result};
use protocol::{self as p, EffectSlot, Entry, Rgb};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------ data for the UI

#[derive(Serialize)]
struct EffectView {
    #[serde(flatten)]
    effect: p::Effect,
    slot: Option<EffectSlot>,
    /// The 7 colours its colour setting picks from (effects with a palette).
    palette: Option<Vec<Rgb>>,
}

#[derive(Serialize)]
struct DeviceSettings {
    side_mode: u8,
    knob_mode: u8,
    os_mode: u8,
    win_lock: bool,
    sleep: u32,
}

#[derive(Serialize)]
struct MacroView {
    name: String,
    events: Vec<String>,
    event_count: usize,
}

/// What a command read or changed. Every field is optional so a write can send back just
/// the tables it touched; the UI merges it into what it has.
#[derive(Serialize, Default)]
struct Snapshot {
    mode: Option<device::Mode>,
    battery: Option<Option<u8>>,
    active_effect: Option<u8>,
    effects: Option<Vec<EffectView>>,
    device: Option<DeviceSettings>,
    custom: Option<Vec<Rgb>>,
    /// Layers 0 (base) and 1 (Fn), 126 four-byte entries each.
    keymap: Option<Vec<Vec<Entry>>>,
    factory_keymap: Option<Vec<Vec<Entry>>>,
    macros: Option<Vec<MacroView>>,
}

fn entries(raw: &[u8]) -> Vec<Entry> {
    raw.chunks_exact(4).map(|c| c.try_into().unwrap()).collect()
}

fn effects_view(led: &p::LedConfig, palette: &[u8]) -> Vec<EffectView> {
    p::EFFECTS
        .iter()
        .map(|e| EffectView {
            effect: *e,
            slot: led.slot(e.hw).ok(),
            palette: (e.color && e.hw < p::PALETTE_COUNT)
                .then(|| (0..p::PALETTE_COLORS).map(|c| p::palette_color(palette, e.hw, c).unwrap()).collect()),
        })
        .collect()
}

fn device_settings(led: &p::LedConfig) -> DeviceSettings {
    DeviceSettings {
        side_mode: led.side_mode(),
        knob_mode: led.knob_mode(),
        os_mode: led.os_mode(),
        win_lock: led.win_lock(),
        sleep: led.sleep(),
    }
}

fn lighting(snap: &mut Snapshot, led: &p::LedConfig, palette: &[u8]) {
    snap.active_effect = Some(led.effect());
    snap.effects = Some(effects_view(led, palette));
    snap.device = Some(device_settings(led));
}

fn macros_view(store: &[u8]) -> Vec<MacroView> {
    macros::parse_store(store)
        .into_iter()
        .map(|m| MacroView {
            name: m.name.clone(),
            event_count: m.events.len(),
            events: m.events.iter().map(macros::Event::describe).collect(),
        })
        .collect()
}

#[derive(Serialize)]
struct Catalog {
    keys: Vec<keymap::Key>,
    effects: Vec<p::Effect>,
    colors: Vec<&'static str>,
    side_modes: Vec<&'static str>,
    knob_modes: Vec<&'static str>,
    os_modes: Vec<&'static str>,
    sleep_choices: Vec<u32>,
    hid: Vec<(&'static str, &'static str, u8)>,
    modifiers: Vec<(&'static str, &'static str, u8)>,
    layout: &'static str,
}

#[derive(Serialize)]
struct Status {
    devices: Vec<DeviceInfoView>,
    vendor_app_running: bool,
    connected: bool,
    /// An S75 Pro: how it's connected ("USB cable" / "2.4G dongle"), if one is there.
    s75: Option<String>,
}

#[derive(Serialize)]
struct DeviceInfoView {
    mode: String,
    supported: bool,
    name: String,
}

// ------------------------------------------------------------ keyboard access

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) kb: Arc<Mutex<Option<Keyboard>>>,
    /// An AULA S75 Pro (its own protocol), opened on first use.
    #[cfg(windows)]
    pub(crate) s75: Arc<Mutex<Option<s75::device::S75>>>,
    #[cfg(windows)]
    pub(crate) audio: Mutex<Option<audio::run::Handle>>,
    #[cfg(windows)]
    pub(crate) anim: Mutex<Option<anim::run::Handle>>,
    #[cfg(windows)]
    pub(crate) config: Mutex<app::AppConfig>,
    #[cfg(windows)]
    pub(crate) tray_anim: Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
    #[cfg(windows)]
    pub(crate) league: Mutex<Option<match_watch::run::Handle>>,
    /// What League lighting replaced at the start of this match, to bring back at its end.
    #[cfg(windows)]
    pub(crate) league_resume: Mutex<Option<app::Resume>>,
    #[cfg(windows)]
    pub(crate) league_live: std::sync::atomic::AtomicBool,
    #[cfg(windows)]
    pub(crate) league_status: Mutex<Option<String>>,
    #[cfg(windows)]
    pub(crate) discord: Mutex<Option<discord::run::Handle>>,
    /// The Audio page's preview: listening and drawing, nothing sent to the keyboard.
    #[cfg(windows)]
    pub(crate) audio_preview: Mutex<Option<audio::run::Handle>>,
    #[cfg(windows)]
    pub(crate) discord_status: Mutex<String>,
    #[cfg(windows)]
    pub(crate) discord_voice: Mutex<discord::Voice>,
}

#[cfg(windows)]
pub(crate) fn open_keyboard() -> Result<Keyboard> {
    if win::vendor_app_running() {
        return derr("The official AULA app (OemDrv.exe) is running. Quit it first (also from the tray): it uses the same channel.");
    }
    win::open_keyboard()
}

#[cfg(not(windows))]
fn open_keyboard() -> Result<Keyboard> {
    derr("Only Windows is supported so far.")
}

/// Run `f` with the open keyboard on a worker thread. On an error the keyboard is closed,
/// so the next command opens it again (it may have been unplugged or switched over).
async fn with_kb<T, F>(state: tauri::State<'_, AppState>, f: F) -> std::result::Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Keyboard) -> Result<T> + Send + 'static,
{
    let shared = state.kb.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = shared.lock().map_err(|_| "keyboard lock poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(open_keyboard().map_err(|e| e.0)?);
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

// ------------------------------------------------------------ commands

#[tauri::command]
fn catalog() -> Catalog {
    #[cfg(windows)]
    let layout = match win::detect_layout() {
        macros::Layout::TurkishQ => "Turkish Q",
        macros::Layout::Us => "English (US)",
    };
    #[cfg(not(windows))]
    let layout = "English (US)";
    Catalog {
        keys: keymap::KEYS.to_vec(),
        effects: p::EFFECTS.to_vec(),
        colors: p::COLORS.to_vec(),
        side_modes: p::SIDE_MODES.to_vec(),
        knob_modes: p::KNOB_MODES.to_vec(),
        os_modes: p::OS_MODES.to_vec(),
        sleep_choices: p::SLEEP_CHOICES.to_vec(),
        hid: keymap::HID.to_vec(),
        modifiers: keymap::MODIFIERS.to_vec(),
        layout,
    }
}

#[tauri::command]
async fn status(state: tauri::State<'_, AppState>) -> std::result::Result<Status, String> {
    let connected = state.kb.lock().map(|g| g.is_some()).unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        let (devices, vendor): (Vec<DeviceInfoView>, bool) = (
            win::find_devices().into_iter().map(|d| DeviceInfoView { mode: d.mode, supported: d.supported, name: d.name }).collect(),
            win::vendor_app_running(),
        );
        #[cfg(not(windows))]
        let (devices, vendor): (Vec<DeviceInfoView>, bool) = (vec![], false);
        #[cfg(windows)]
        let s75 = {
            let f = s75::winlink::find();
            if f.usb.is_some() {
                Some("USB cable".to_string())
            } else {
                f.dongle.map(|_| "2.4G dongle".to_string())
            }
        };
        #[cfg(not(windows))]
        let s75 = None;
        Status { devices, vendor_app_running: vendor, connected, s75 }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Read everything the UI shows.
#[tauri::command]
async fn read_all(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> std::result::Result<Snapshot, String> {
    let all = ["lighting", "keymap", "battery", "custom", "macros"].map(String::from).to_vec();
    read_parts(app, state, all).await
}

/// Read only some of the keyboard's tables: "lighting" (lighting block and colours, which
/// also hold the device settings), "keymap" (both layers), "battery", "custom" (per-key
/// colours) and "macros". Over Bluetooth every table is slow, so the app reads only what the
/// page in front needs.
#[tauri::command]
async fn read_parts(app: tauri::AppHandle, state: tauri::State<'_, AppState>, parts: Vec<String>) -> std::result::Result<Snapshot, String> {
    use tauri::Emitter;
    with_kb(state, move |kb| {
        let wired = kb.mode == device::Mode::Wired;
        let want = |p: &str| parts.iter().any(|x| x == p);
        // over the cable, where the per-key colours and macros live isn't known yet
        let custom = want("custom") && !wired;
        let macros = want("macros") && !wired;
        let total = 2 * want("lighting") as u32 + 2 * want("keymap") as u32 + want("battery") as u32 + custom as u32 + macros as u32;
        // each table is a separate read (slow over Bluetooth), so the page can show progress
        let mut step = 0;
        let mut next = |what: &str| {
            step += 1;
            let _ = app.emit("read-progress", ReadProgress { step, total, what: what.into() });
        };
        let mut s = Snapshot { mode: Some(kb.mode), ..Default::default() };
        if want("lighting") {
            next("lighting");
            let led = kb.get_led()?;
            next("colours");
            let palette = kb.get_palette()?;
            lighting(&mut s, &led, &palette);
        }
        if want("keymap") {
            next("the keymap");
            let base = entries(&kb.get_matrix(0)?);
            next("the Fn layer");
            s.keymap = Some(vec![base, entries(&kb.get_matrix(1)?)]);
            s.factory_keymap = Some(vec![entries(&p::factory::matrix(0)), entries(&p::factory::matrix(1))]);
        }
        if want("battery") {
            next("the battery");
            // over Bluetooth Windows already knows the level: no keyboard traffic needed
            #[cfg(windows)]
            let from_windows = if kb.mode == device::Mode::Bluetooth { win::bluetooth_battery() } else { None };
            #[cfg(not(windows))]
            let from_windows = None;
            s.battery = Some(match from_windows {
                Some(b) => Some(b),
                None => kb.get_battery()?,
            });
        }
        if custom {
            next("per-key colours");
            s.custom = Some(p::colors_from_custom(&kb.get_custom()?));
        }
        if macros {
            next("macros");
            s.macros = Some(macros_view(&kb.get_macro_store()?));
        }
        Ok(s)
    })
    .await
}

/// How the keyboard is connected (opening it if needed, without reading anything).
#[tauri::command]
async fn connection_mode(state: tauri::State<'_, AppState>) -> std::result::Result<device::Mode, String> {
    with_kb(state, |kb| Ok(kb.mode)).await
}

/// How far `read_all` got: table `step` of `total`, now reading `what`.
#[derive(Serialize, Clone)]
struct ReadProgress {
    step: u32,
    total: u32,
    what: String,
}

#[tauri::command]
async fn read_battery(state: tauri::State<'_, AppState>) -> std::result::Result<Option<u8>, String> {
    with_kb(state, |kb| kb.get_battery()).await
}

#[derive(Deserialize)]
struct EffectChange {
    hw: u8,
    brightness: Option<u8>,
    speed: Option<u8>,
    color: Option<u8>,
    /// Any colour: written into the effect's palette in place of its current colour.
    rgb: Option<Rgb>,
}

/// Select an effect and change its settings (like `aula effect`).
#[tauri::command]
async fn set_effect(state: tauri::State<'_, AppState>, change: EffectChange) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        let Some(e) = p::effect(change.hw) else { return derr(format!("unknown effect {}", change.hw)) };
        // where the cable keeps the per-key colours isn't known, and unknown effect numbers
        // have blanked the LEDs there (cloudglides/aula-f75-linux-drivers)
        if kb.mode == device::Mode::Wired && e.hw == p::CUSTOM_EFFECT {
            return derr("The per-key effect isn't available over the USB cable yet.");
        }
        let mut led = kb.get_led()?;
        let mut palette = kb.get_palette()?;
        led.set_effect(e.hw);
        if e.hw <= p::LED_SLOTS {
            let mut s = led.slot(e.hw)?;
            if let Some(b) = change.brightness {
                s.brightness = b;
            }
            if let Some(v) = change.speed {
                s.speed = v;
            }
            if let Some(c) = change.color {
                s.color = c;
            }
            if let Some(rgb) = change.rgb {
                if !e.color || e.hw >= p::PALETTE_COUNT {
                    return derr(format!("{} has no colour setting", e.label));
                }
                if s.color >= p::PALETTE_COLORS {
                    s.color = 0; // "multi": take over the first colour
                }
                p::set_palette_color(&mut palette, e.hw, s.color, rgb)?;
                kb.set_palette(&palette)?;
            }
            led.set_slot(e.hw, s)?;
        }
        kb.set_led(&led)?;
        let mut snap = Snapshot::default();
        lighting(&mut snap, &led, &palette);
        Ok(snap)
    })
    .await
}

/// Factory colours back for one effect's palette.
#[tauri::command]
async fn reset_palette(state: tauri::State<'_, AppState>, hw: u8) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        let led = kb.get_led()?;
        let mut palette = kb.get_palette()?;
        p::reset_palette(&mut palette, hw)?;
        kb.set_palette(&palette)?;
        let mut snap = Snapshot::default();
        lighting(&mut snap, &led, &palette);
        Ok(snap)
    })
    .await
}

/// Write the whole per-key table (126 colours by matrix index) and switch to the per-key effect.
#[tauri::command]
async fn set_custom(state: tauri::State<'_, AppState>, colors: Vec<Rgb>) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        if colors.len() != p::MATRIX_LEN {
            return derr("the per-key table needs 126 colours");
        }
        let mut led = kb.get_led()?;
        if led.effect() != p::CUSTOM_EFFECT {
            led.set_effect(p::CUSTOM_EFFECT);
            kb.set_led(&led)?; // the colour table has to be sent after the LED block
        }
        let pairs: Vec<(u8, Rgb)> = colors.iter().enumerate().map(|(i, c)| (i as u8, *c)).collect();
        kb.set_custom(&p::custom_from_colors(&pairs))?;
        Ok(Snapshot { active_effect: Some(led.effect()), custom: Some(colors), ..Default::default() })
    })
    .await
}

#[derive(Deserialize)]
struct DeviceChange {
    side_mode: Option<u8>,
    knob_mode: Option<u8>,
    os_mode: Option<u8>,
    win_lock: Option<bool>,
    sleep: Option<u32>,
}

#[tauri::command]
async fn set_device(state: tauri::State<'_, AppState>, change: DeviceChange) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        let mut led = kb.get_led()?;
        if let Some(m) = change.side_mode {
            led.set_side_mode(m)?;
        }
        if let Some(m) = change.knob_mode {
            led.set_knob_mode(m)?;
        }
        if let Some(m) = change.os_mode {
            led.set_os_mode(m)?;
        }
        if let Some(w) = change.win_lock {
            led.set_win_lock(w);
        }
        if let Some(s) = change.sleep {
            led.set_sleep(s)?;
        }
        kb.set_led(&led)?;
        Ok(Snapshot { device: Some(device_settings(&led)), ..Default::default() })
    })
    .await
}

fn check_layer(layer: u8) -> Result<()> {
    if layer > 1 {
        return derr("layer must be 0 (base) or 1 (Fn)");
    }
    Ok(())
}

/// Write keymap entries on one layer and read the layer back to check them.
fn write_entries(kb: &mut Keyboard, layer: u8, changes: &[(u8, Entry)]) -> Result<Vec<u8>> {
    check_layer(layer)?;
    let mut raw = kb.get_matrix(layer)?;
    for &(i, e) in changes {
        if keymap::key(i).is_none() {
            return derr(format!("no key at matrix index {i}"));
        }
        p::set_matrix_entry(&mut raw, i, e);
    }
    kb.set_matrix(&raw, layer)?;
    let check = kb.get_matrix(layer)?;
    for &(i, e) in changes {
        if p::matrix_entry(&check, i) != e {
            return derr(format!("Verification failed: key {i} reads back {:02x?}, expected {e:02x?}.", p::matrix_entry(&check, i)));
        }
    }
    Ok(check)
}

fn keymap_snapshot(kb: &mut Keyboard, layer: u8, raw: Vec<u8>) -> Result<Snapshot> {
    let other = kb.get_matrix(1 - layer)?;
    let layers = if layer == 0 { vec![entries(&raw), entries(&other)] } else { vec![entries(&other), entries(&raw)] };
    Ok(Snapshot { keymap: Some(layers), ..Default::default() })
}

/// Set keys to raw entries (a key, a key with modifiers, or a modifier) on one layer.
#[tauri::command]
async fn set_keys(state: tauri::State<'_, AppState>, layer: u8, changes: Vec<(u8, Entry)>) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        let raw = write_entries(kb, layer, &changes)?;
        keymap_snapshot(kb, layer, raw)
    })
    .await
}

/// Give keys their factory function back.
#[tauri::command]
async fn reset_keys(state: tauri::State<'_, AppState>, layer: u8, indices: Vec<u8>) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        check_layer(layer)?;
        let factory = p::factory::matrix(layer);
        let changes: Vec<(u8, Entry)> = indices.iter().map(|&i| (i, p::matrix_entry(&factory, i))).collect();
        let raw = write_entries(kb, layer, &changes)?;
        keymap_snapshot(kb, layer, raw)
    })
    .await
}

#[derive(Deserialize)]
struct MacroSave {
    name: String,
    steps: Vec<String>,
    delay: u32,
    /// (layer, matrix index) to assign it to
    key: Option<(u8, u8)>,
    mode: u8,
    count: u8,
}

/// Create or replace a macro by name, and optionally assign it to a key.
#[tauri::command]
async fn save_macro(state: tauri::State<'_, AppState>, m: MacroSave) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        let name = m.name.trim().to_string();
        if name.is_empty() {
            return derr("Give the macro a name.");
        }
        #[cfg(windows)]
        let layout = win::detect_layout();
        #[cfg(not(windows))]
        let layout = macros::Layout::Us;
        let events = macros::parse_steps(&m.steps, m.delay, layout)?;
        let mut list = macros::parse_store(&kb.get_macro_store()?);
        let index = match list.iter().position(|x| x.name == name) {
            Some(i) => {
                list[i] = macros::Macro::new(&name, events);
                i
            }
            None => {
                list.push(macros::Macro::new(&name, events));
                list.len() - 1
            }
        };
        let store = macros::build_store(&list)?;
        kb.set_macro_store(&store)?;
        let mut snap = Snapshot { macros: Some(macros_view(&store)), ..Default::default() };
        if let Some((layer, key)) = m.key {
            let raw = write_entries(kb, layer, &[(key, macros::macro_entry(index as u8, m.mode, m.count))])?;
            snap.keymap = keymap_snapshot(kb, layer, raw)?.keymap;
        }
        Ok(snap)
    })
    .await
}

/// Remove a macro and fix the keymap: keys pointing at later macros move down by one, keys
/// of the deleted one get their factory function back.
#[tauri::command]
async fn delete_macro(state: tauri::State<'_, AppState>, index: u8) -> std::result::Result<Snapshot, String> {
    with_kb(state, move |kb| {
        let mut list = macros::parse_store(&kb.get_macro_store()?);
        if index as usize >= list.len() {
            return derr("No such macro.");
        }
        list.remove(index as usize);
        let store = macros::build_store(&list)?;
        kb.set_macro_store(&store)?;
        let mut layers = Vec::new();
        for layer in 0..4u8 {
            let raw = kb.get_matrix(layer)?;
            let factory = p::factory::matrix(layer);
            let mut new = raw.clone();
            for i in 0..p::MATRIX_LEN as u8 {
                let e = p::matrix_entry(&raw, i);
                if e[0] != macros::MACRO_ENTRY {
                    continue;
                }
                if e[3] == index {
                    p::set_matrix_entry(&mut new, i, p::matrix_entry(&factory, i));
                } else if e[3] > index {
                    new[4 * i as usize + 3] = e[3] - 1;
                }
            }
            if new != raw {
                kb.set_matrix(&new, layer)?;
            }
            if layer < 2 {
                layers.push(entries(&new));
            }
        }
        Ok(Snapshot { macros: Some(macros_view(&store)), keymap: Some(layers), ..Default::default() })
    })
    .await
}

/// Same files and order as `aula backup` / `aula restore`, so backups work with both tools.
const BACKUP: [&str; 7] = [
    "cmd_41.hex", "cmd_41_layer1.hex", "cmd_41_layer2.hex", "cmd_41_layer3.hex", "cmd_49.hex", "cmd_44.hex",
    // the custom colour table has to be sent after the LED block
    "cmd_42.hex",
];

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return derr("backup file is not hex");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| device::DeviceError("backup file is not hex".into())))
        .collect()
}

fn layer_of(name: &str) -> u8 {
    name.strip_prefix("cmd_41_layer").and_then(|s| s.strip_suffix(".hex")).and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn io(e: std::io::Error) -> device::DeviceError {
    device::DeviceError(e.to_string())
}

#[tauri::command]
async fn backup(state: tauri::State<'_, AppState>, folder: String) -> std::result::Result<Vec<String>, String> {
    with_kb(state, move |kb| {
        let dir = std::path::PathBuf::from(&folder);
        std::fs::create_dir_all(&dir).map_err(io)?;
        let mut saved = Vec::new();
        for name in BACKUP {
            let data = match name {
                "cmd_49.hex" => kb.get_palette()?,
                "cmd_44.hex" => kb.get_led()?.0.to_vec(),
                "cmd_42.hex" => kb.get_custom()?,
                _ => kb.get_matrix(layer_of(name))?,
            };
            std::fs::write(dir.join(name), hex(&data)).map_err(io)?;
            saved.push(name.to_string());
        }
        Ok(saved)
    })
    .await
}

#[tauri::command]
async fn restore(state: tauri::State<'_, AppState>, folder: String) -> std::result::Result<Vec<String>, String> {
    with_kb(state, move |kb| {
        let dir = std::path::PathBuf::from(&folder);
        // check every file first, so a broken backup doesn't leave the keyboard half restored
        let mut tables = Vec::new();
        for name in BACKUP {
            let path = dir.join(name);
            if !path.exists() {
                continue;
            }
            let data = unhex(&std::fs::read_to_string(&path).map_err(io)?)?;
            let ok = match name {
                "cmd_49.hex" => data.len() == p::PALETTE_LEN,
                "cmd_44.hex" => p::LedConfig::parse(&data).is_ok(),
                "cmd_42.hex" => data.len() == p::CUSTOM_LEN,
                _ => data.len() == p::MATRIX_BYTES,
            };
            if !ok {
                return derr(format!("{name} in the backup has the wrong size."));
            }
            tables.push((name, data));
        }
        if tables.is_empty() {
            return derr("No backup files (cmd_41.hex …) in that folder.");
        }
        let mut done = Vec::new();
        for (name, data) in tables {
            match name {
                "cmd_49.hex" => kb.set_palette(&data)?,
                "cmd_44.hex" => kb.set_led(&p::LedConfig::parse(&data)?)?,
                "cmd_42.hex" => kb.set_custom(&data)?,
                _ => kb.set_matrix(&data, layer_of(name))?,
            }
            done.push(name.to_string());
        }
        Ok(done)
    })
    .await
}

// ------------------------------------------------------------ audio

/// Start lighting the keyboard to the computer's sound. Frames go to the UI as
/// "audio-frame" events, problems as "audio-error", the end as "audio-stopped".
#[cfg(windows)]
#[tauri::command]
fn audio_start(app: tauri::AppHandle, settings: audio::Settings) -> std::result::Result<(), String> {
    // your choice wins over League lighting for the rest of a match
    app::league_step_aside(&app);
    start_audio(&app, settings)
}

/// Start (or update) the audio lighting; also used to bring it back after a League match.
#[cfg(windows)]
pub(crate) fn start_audio(app: &tauri::AppHandle, settings: audio::Settings) -> std::result::Result<(), String> {
    use tauri::{Emitter, Manager};
    let state = app.state::<AppState>();
    let mut slot = state.audio.lock().map_err(|_| "audio lock poisoned".to_string())?;
    if let Some(h) = slot.as_ref() {
        *h.settings.lock().unwrap() = settings;
        return Ok(());
    }
    if win::vendor_app_running() {
        return Err("The official AULA app (OemDrv.exe) is running. Quit it first (also from the tray).".into());
    }
    // only one thing can stream frames at a time: the audio lighting replaces an animation
    // (stopped without forgetting it, so it comes back with the app)
    if state.anim.lock().map(|a| a.is_some()).unwrap_or(false) {
        app::stop_animation(app, true);
    }
    // the real thing replaces the page's preview
    if let Some(p) = state.audio_preview.lock().map_err(|_| "audio lock poisoned".to_string())?.take() {
        p.stop();
    }
    let app = app.clone();
    let handle = audio::run::start(state.kb.clone(), open_keyboard, settings, true, move |ev| {
        let _ = match ev {
            audio::run::Event::Frame(f) => app.emit("audio-frame", f),
            audio::run::Event::Error(e) => app.emit("audio-error", e),
            audio::run::Event::Stopped => app.emit("audio-stopped", ()),
        };
    })?;
    *slot = Some(handle);
    Ok(())
}

/// Change mode, gain, smoothness or colour while it runs.
#[cfg(windows)]
#[tauri::command]
fn audio_update(state: tauri::State<'_, AppState>, settings: audio::Settings) -> std::result::Result<(), String> {
    if let Some(h) = state.audio.lock().map_err(|_| "audio lock poisoned".to_string())?.as_ref() {
        *h.settings.lock().unwrap() = settings;
    }
    Ok(())
}

/// Stop; the keyboard goes back to its own effect about 2 s later.
#[cfg(windows)]
#[tauri::command]
async fn audio_stop(state: tauri::State<'_, AppState>) -> std::result::Result<(), String> {
    let handle = state.audio.lock().map_err(|_| "audio lock poisoned".to_string())?.take();
    if let Some(h) = handle {
        tauri::async_runtime::spawn_blocking(move || h.stop()).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Start or stop the Audio page's preview (only while the audio lighting itself is off):
/// frames arrive as "audio-frame" like the real thing, but nothing reaches the keyboard.
#[cfg(windows)]
#[tauri::command]
async fn audio_preview(app: tauri::AppHandle, on: bool, settings: audio::Settings) -> std::result::Result<(), String> {
    use tauri::{Emitter, Manager};
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut slot = state.audio_preview.lock().map_err(|_| "audio lock poisoned".to_string())?;
        if !on {
            if let Some(p) = slot.take() {
                p.stop();
            }
            return Ok(());
        }
        if state.audio.lock().map(|a| a.is_some()).unwrap_or(false) {
            return Ok(()); // the real thing is on: its frames are the preview
        }
        if let Some(p) = slot.as_ref() {
            *p.settings.lock().unwrap() = settings;
            return Ok(());
        }
        let app2 = app.clone();
        *slot = Some(audio::run::start(state.kb.clone(), open_keyboard, settings, false, move |ev| {
            if let audio::run::Event::Frame(f) = ev {
                let _ = app2.emit("audio-frame", f);
            }
        })?);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct AudioDevices {
    devices: Vec<String>,
    default: Option<String>,
}

/// Output devices the audio lighting can listen to.
#[cfg(windows)]
#[tauri::command]
async fn audio_devices() -> std::result::Result<AudioDevices, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (devices, default) = audio::run::output_devices();
        AudioDevices { devices, default }
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(windows)]
#[tauri::command]
fn audio_running(state: tauri::State<'_, AppState>) -> bool {
    state.audio.lock().map(|a| a.is_some()).unwrap_or(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // a second launch (e.g. from the Start menu while it runs in the tray) shows this one;
        // two copies would fight over the keyboard
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| app::show_window(app)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![app::MINIMIZED_ARG]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| Ok(app::setup(app.handle())?))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                app::on_close_requested(window, api);
            }
        })
        .invoke_handler(tauri::generate_handler![
            catalog, status, read_all, read_battery, set_effect, reset_palette, set_custom, set_device, set_keys,
            reset_keys, save_macro, delete_macro, backup, restore, audio_start, audio_update, audio_stop,
            audio_running, audio_devices, app::anim_get, app::anim_start, app::anim_update, app::anim_stop,
            app::app_settings_get, app::app_settings_set, app::presets_get, app::anim_preset_save,
            app::perkey_preset_save, app::audio_preset_save, app::preset_delete, app::league_get, app::league_set,
            app::league_preset_save, app::anim_image_import, app::discord_get, app::discord_set,
            app::discord_reconnect, app::playing, app::stop_playing,
            app::anim_preview, audio_preview,
            s75::app::s75_get, s75::app::s75_set_lighting, s75::app::s75_set_bar, s75::app::s75_set_settings,
            s75::app::s75_battery, s75::app::s75_set_perkey, s75::app::s75_read_perkey,
            s75::app::s75_set_key, s75::app::s75_set_macros, s75::app::s75_screen_preview,
            s75::app::s75_upload_screen, s75::app::s75_sync_clock, read_parts, connection_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        unhex(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// The user's own keyboard, backed up with `aula backup` on 2026-10-20.
    #[test]
    fn users_backup_parses_as_aula_info_showed_it() {
        let led = p::LedConfig::parse(&fixture("cmd_44.hex")).unwrap();
        assert_eq!(p::effect(led.effect()).unwrap().name, "stream");
        let s = led.slot(led.effect()).unwrap();
        assert_eq!((s.brightness, s.speed, s.color), (1, 4, 7));
        assert_eq!(led.sleep(), 300);
        assert_eq!((led.side_mode(), led.knob_mode(), led.os_mode(), led.win_lock()), (0, 0, 0, false));
        assert_eq!(effects_view(&led, &fixture("cmd_49.hex")).len(), p::EFFECTS.len());
        assert_eq!(fixture("cmd_41.hex").len(), p::MATRIX_BYTES);
        assert_eq!(fixture("cmd_42.hex").len(), p::CUSTOM_LEN);
    }

    #[test]
    fn backup_names() {
        assert_eq!(layer_of("cmd_41.hex"), 0);
        assert_eq!(layer_of("cmd_41_layer3.hex"), 3);
        assert_eq!(unhex(&hex(&[0, 0xab, 0xff])).unwrap(), vec![0, 0xab, 0xff]);
    }
}
