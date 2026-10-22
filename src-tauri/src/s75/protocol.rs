//! AULA S75 Pro configuration protocol, written from the description in open-aula's
//! `web/PROTOCOL.md` (https://github.com/paulcoding810/open-aula, reverse-engineered from the
//! vendor's DeviceDriver.exe). Only the protocol facts are used; no code is copied.
//! Not yet tested on an S75 Pro.
//!
//! USB (0C45:800A): every command is a 64-byte feature report `04 op b2 0 0 0 0 0 count_lo
//! count_hi …`, answered by a feature report read back; `count` 64-byte data packets follow,
//! the last ending in `aa 55`. A write is `18` (begin) → op → data → `02` (end/save) → `f0`
//! (apply).
//!
//! 2.4 GHz dongle (05AC:024F, interface 3): 32-byte output reports `cmd len seq data[28]
//! checksum` (checksum = sum of the first 31 bytes); the dongle echoes `cmd len seq`.

/// Opcodes of the USB command report.
pub mod op {
    pub const BEGIN: u8 = 0x18;
    pub const END: u8 = 0x02;
    pub const APPLY: u8 = 0xf0;
    pub const LIGHT: u8 = 0x13;
    pub const SETTINGS: u8 = 0x17;
    pub const TIME: u8 = 0x28;
    pub const KEYMAP: u8 = 0x11;
    pub const KEYMAP_FN: u8 = 0x27;
    pub const USER_LIGHT: u8 = 0x23;
    pub const USER_LIGHT_READ: u8 = 0xf5;
    pub const MACRO_BEGIN: u8 = 0x19;
    pub const MACRO: u8 = 0x15;
    pub const SCREEN: u8 = 0x72;
}

/// Commands over the dongle.
pub mod radio {
    pub const LIGHT: u8 = 0x05;
    pub const SETTINGS: u8 = 0x07;
    pub const MACROS: u8 = 0x09;
    pub const TIME: u8 = 0x0c;
    pub const KEYMAP: u8 = 0x10;
    pub const KEYMAP_FN: u8 = 0x12;
    pub const USER_LIGHT: u8 = 0x14;
    pub const BATTERY: u8 = 0x20;
}

pub const REPORT: usize = 64;
/// Keymap and per-key colour tables: 4 bytes per key index, 9 packets.
pub const BLOCK: usize = 0x240;
/// The per-key lighting mode.
pub const USER_LIGHT_MODE: u8 = 0x80;
pub const RADIO: usize = 32;
const RADIO_DATA: usize = 28;
/// Screen: 135 × 240 pixels, RGB565 little-endian, after a 256-byte header, in 4096-byte chunks.
pub const SCREEN_W: usize = 135;
pub const SCREEN_H: usize = 240;
pub const SCREEN_HEAD: usize = 256;
pub const SCREEN_CHUNK: usize = 4096;
pub const SCREEN_MAX_FRAMES: usize = 141;

pub type Report = [u8; REPORT];

fn tag(buf: &mut [u8], at: usize) {
    buf[at] = 0xaa;
    buf[at + 1] = 0x55;
}

/// A control command.
pub fn command(op: u8, b2: u8, count: u16) -> Report {
    let mut b = [0u8; REPORT];
    b[0] = 0x04;
    b[1] = op;
    b[2] = b2;
    b[8..10].copy_from_slice(&count.to_le_bytes());
    b
}

/// The built-in lighting effects: (mode, name, which controls apply). The control bits are
/// 1 brightness, 2 speed, 4 left/right, 8 up/down, 0x10 colourful, 0x20 colour.
pub const EFFECTS: [(u8, &str, u8); 20] = [
    (1, "Static", 0x31),
    (2, "Single On", 0x33),
    (3, "Single Off", 0x33),
    (4, "Glittering", 0x33),
    (5, "Fluttering", 0x33),
    (6, "Colourful", 0x03),
    (7, "Breath", 0x33),
    (8, "Spectrum", 0x03),
    (9, "Outward", 0x33),
    (10, "Scrolling", 0x3b),
    (11, "Rolling", 0x37),
    (12, "Rotating", 0x37),
    (13, "Explode", 0x33),
    (14, "Launch", 0x33),
    (15, "Ripples", 0x33),
    (16, "Flowing", 0x37),
    (17, "Pulsating", 0x33),
    (18, "Tilt", 0x37),
    (19, "Shuttle", 0x33),
    (0, "Off", 0x00),
];

/// The side light bar's modes, as the S75 Pro actually shows them (open-aula tested these).
pub const BAR_MODES: [(u8, &str); 5] = [(0x20, "Flowing"), (0x21, "Red"), (0x22, "Blue"), (0x23, "Neon"), (0x24, "Off")];

/// Lighting: an effect and its controls. Brightness and speed 0-5; direction 0 left, 1
/// right, 2 up, 3 down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Lighting {
    pub mode: u8,
    pub rgb: [u8; 3],
    pub colorful: bool,
    pub brightness: u8,
    pub speed: u8,
    pub direction: u8,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting { mode: 7, rgb: [255, 0, 0], colorful: true, brightness: 5, speed: 3, direction: 1 }
    }
}

pub fn light_payload(l: &Lighting) -> Report {
    let mut p = [0u8; REPORT];
    p[0] = l.mode;
    p[1..4].copy_from_slice(&l.rgb);
    p[8] = l.colorful as u8;
    p[9] = l.brightness.min(5);
    p[10] = l.speed.min(5);
    p[11] = l.direction.min(3);
    tag(&mut p, 62);
    p
}

/// The side light bar uses the lighting command with its mode alone.
pub fn bar_payload(mode: u8) -> Report {
    let mut p = [0u8; REPORT];
    p[0] = mode;
    tag(&mut p, 62);
    p
}

/// Settings. Sleep: 0 never, 1 one minute, 2 five, 3 thirty. Response (debounce) 1-5.
/// Fn switch: 0 momentary, 1 toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub win_lock: bool,
    pub alt_f4_lock: bool,
    pub alt_tab_lock: bool,
    pub fn_switch: u8,
    pub sleep: u8,
    pub response: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { win_lock: false, alt_f4_lock: false, alt_tab_lock: false, fn_switch: 0, sleep: 0, response: 1 }
    }
}

pub fn settings_payload(s: &Settings) -> Report {
    let mut p = [0u8; REPORT];
    p[..9].copy_from_slice(&[0, 1, s.win_lock as u8, s.alt_f4_lock as u8, s.alt_tab_lock as u8, s.fn_switch.min(1), s.sleep.min(3), 0, s.response.clamp(1, 5)]);
    tag(&mut p, 62);
    p
}

/// The clock: `weekday` 0 = Sunday.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Time {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub weekday: u8,
}

pub fn time_payload(t: &Time, slot: u8) -> Report {
    let mut p = [0u8; REPORT];
    p[..11].copy_from_slice(&[0, slot, 0x5a, (t.year % 2000) as u8, t.month, t.day, t.hour, t.minute, t.second, 0, t.weekday]);
    tag(&mut p, 62);
    p
}

/// A key's action in the keymap (4 bytes).
pub type Entry = [u8; 4];

/// The key's own function.
pub const DEFAULT: Entry = [0, 0, 0, 0];
pub const DISABLED: Entry = [0x05, 0x03, 0, 0];

/// A key or combination: HID usage with a modifier bitmask. A modifier key alone (usage
/// 0xe0-0xe7) goes in as its bit.
pub fn key_entry(usage: u8, mods: u8) -> Entry {
    if (0xe0..=0xe7).contains(&usage) {
        [0x02, mods | 1 << (usage - 0xe0), 0, 0]
    } else {
        [0x02, mods, usage, 0]
    }
}

/// A consumer-control usage (media keys, browser, calculator, ...).
pub fn media_entry(usage: u16) -> Entry {
    let [lo, hi] = usage.to_le_bytes();
    [0x03, lo, hi, 0]
}

/// Mouse actions: (name, the two bytes after type 01).
pub const MOUSE: [(&str, [u8; 2]); 8] = [
    ("Left click", [0x01, 0x01]),
    ("Right click", [0x01, 0x02]),
    ("Middle click", [0x01, 0x04]),
    ("Double click", [0x01, 0x03]),
    ("Scroll up", [0x03, 0x01]),
    ("Scroll down", [0x03, 0xff]),
    ("Mouse button 4", [0x01, 0x08]),
    ("Mouse button 5", [0x01, 0x10]),
];

pub fn mouse_entry(bytes: [u8; 2]) -> Entry {
    [0x01, bytes[0], bytes[1], 0]
}

/// How a macro plays: 0 once, 1 N times, 2 toggle, 3 while held, 4 click to play.
pub fn macro_entry(index: u8, mode: u8, loops: u8) -> Entry {
    [0x06, index, mode.min(4), loops.max(1)]
}

/// A whole table (keymap or per-key colours): 4 bytes per key index, `aa 55` at the end.
fn table(entries: impl IntoIterator<Item = (u8, Entry)>) -> [u8; BLOCK] {
    let mut buf = [0u8; BLOCK];
    for (idx, e) in entries {
        let at = idx as usize * 4;
        if at + 4 <= BLOCK - 2 {
            buf[at..at + 4].copy_from_slice(&e);
        }
    }
    tag(&mut buf, BLOCK - 2);
    buf
}

/// The keymap of one layer, by key index (keys not listed keep their own function).
pub fn keymap_block(entries: &[(u8, Entry)]) -> [u8; BLOCK] {
    table(entries.iter().copied())
}

/// Per-key colours by light index: `[index r g b]` per key.
pub fn perkey_block(colors: &[(u8, [u8; 3])]) -> [u8; BLOCK] {
    table(colors.iter().map(|&(i, c)| (i, [i, c[0], c[1], c[2]])))
}

/// Per-key colours read back (`f5`): the colour at each light index asked for.
pub fn decode_perkey(buf: &[u8], indexes: &[u8]) -> Vec<(u8, [u8; 3])> {
    indexes
        .iter()
        .filter_map(|&i| {
            let o = i as usize * 4;
            (o + 3 < buf.len()).then(|| (i, [buf[o + 1], buf[o + 2], buf[o + 3]]))
        })
        .collect()
}

/// A macro step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    KeyDown { usage: u8 },
    KeyUp { usage: u8 },
    /// 1 left, 2 right, 4 middle.
    MouseDown { button: u8 },
    MouseUp { button: u8 },
    /// At least 10 ms.
    Delay { ms: u16 },
}

fn step_bytes(s: &Step) -> [u8; 4] {
    match *s {
        Step::KeyDown { usage } => [0, 0, usage, 0xb0],
        Step::KeyUp { usage } => [0, 0, usage, 0x30],
        Step::MouseDown { button } => [0, 0, button, 0x90],
        Step::MouseUp { button } => [0, 0, button, 0x10],
        Step::Delay { ms } => {
            let [lo, hi] = ms.max(10).to_le_bytes();
            [lo, hi, 0, 0x50]
        }
    }
}

const MACRO_TABLE: usize = 400;
const MACRO_MAX: usize = 0xe00;

/// The macro store: a 100-entry table of offsets (`ff ff ff ff` = empty), then each macro as
/// a step count, 6 zero bytes and its 4-byte steps. Returns the buffer and its used length.
fn macro_body(macros: &[Vec<Step>]) -> Result<(Vec<u8>, usize), String> {
    if macros.len() > MACRO_TABLE / 4 {
        return Err(format!("At most {} macros.", MACRO_TABLE / 4));
    }
    let mut buf = vec![0u8; MACRO_MAX];
    let mut off = MACRO_TABLE;
    for (i, steps) in macros.iter().enumerate() {
        if steps.is_empty() {
            buf[i * 4..i * 4 + 4].copy_from_slice(&[0xff; 4]);
            continue;
        }
        if off + 8 + steps.len() * 4 > MACRO_MAX - 64 {
            return Err("The keyboard's macro memory is full: use fewer or shorter macros.".into());
        }
        buf[i * 4..i * 4 + 2].copy_from_slice(&(off as u16).to_le_bytes());
        buf[off..off + 2].copy_from_slice(&(steps.len() as u16).to_le_bytes());
        for (j, s) in steps.iter().enumerate() {
            let o = off + 8 + j * 4;
            buf[o..o + 4].copy_from_slice(&step_bytes(s));
        }
        off += 8 + steps.len() * 4;
    }
    Ok((buf, off))
}

/// The macro store for USB: whole 64-byte packets, `aa 55` at the end. Returns the data and
/// the packet count.
pub fn macro_block(macros: &[Vec<Step>]) -> Result<(Vec<u8>, u16), String> {
    let (buf, len) = macro_body(macros)?;
    let packets = len / REPORT + if len % REPORT == 0 { 1 } else { 2 };
    let mut out = buf[..packets * REPORT].to_vec();
    let n = out.len();
    tag(&mut out, n - 2);
    Ok((out, packets as u16))
}

/// A dongle frame (32 bytes): `cmd len seq data[28] checksum`.
pub fn radio_frame(cmd: u8, len: u8, seq: u8, data: &[u8]) -> [u8; RADIO] {
    let mut b = [0u8; RADIO];
    b[0] = cmd;
    b[1] = len;
    b[2] = seq;
    let n = data.len().min(RADIO_DATA);
    b[3..3 + n].copy_from_slice(&data[..n]);
    b[31] = b[..31].iter().fold(0u8, |s, &x| s.wrapping_add(x));
    b
}

/// A one-frame command over the dongle: the first 14 bytes of the USB payload, `aa 55`
/// after them.
pub fn radio_short(cmd: u8, payload: &Report) -> [u8; RADIO] {
    let mut d = [0u8; RADIO_DATA];
    d[..14].copy_from_slice(&payload[..14]);
    tag(&mut d, 14);
    radio_frame(cmd, 0x10, 0, &d)
}

/// A table over the dongle: 21 frames of 28 bytes (the last carries 16).
pub fn radio_block(cmd: u8, block: &[u8; BLOCK]) -> Vec<[u8; RADIO]> {
    (0..21)
        .map(|i| {
            let from = i * RADIO_DATA;
            let to = (from + RADIO_DATA).min(BLOCK);
            radio_frame(cmd, if i == 20 { 0x10 } else { 0x1c }, i as u8, &block[from..to])
        })
        .collect()
}

/// The macro store over the dongle, in 28-byte frames (the last one says how much it holds).
pub fn radio_macros(macros: &[Vec<Step>]) -> Result<Vec<[u8; RADIO]>, String> {
    let (buf, len) = macro_body(macros)?;
    let chunks = len / RADIO_DATA + 1 + usize::from(len % RADIO_DATA != 0);
    let mut out = vec![0u8; chunks * RADIO_DATA];
    let n = len.min(out.len());
    out[..n].copy_from_slice(&buf[..n]);
    let end = out.len();
    tag(&mut out, end - 2);
    Ok((0..chunks)
        .map(|i| {
            let len = if i == chunks - 1 { (len % RADIO_DATA) as u8 } else { RADIO_DATA as u8 };
            radio_frame(radio::MACROS, len, i as u8, &out[i * RADIO_DATA..(i + 1) * RADIO_DATA])
        })
        .collect())
}

/// A screen image: a frame count and each frame's delay (in 2 ms units) in the header, then
/// the frames as RGB565 little-endian, padded with `ff` to whole 4096-byte chunks.
pub fn screen_image(frames: &[(Vec<u8>, u32)]) -> Result<Vec<u8>, String> {
    if frames.is_empty() || frames.len() > SCREEN_MAX_FRAMES {
        return Err(format!("The screen takes 1 to {SCREEN_MAX_FRAMES} frames."));
    }
    let size = SCREEN_W * SCREEN_H * 2;
    let total = SCREEN_HEAD + size * frames.len();
    let mut buf = vec![0xffu8; total.div_ceil(SCREEN_CHUNK) * SCREEN_CHUNK];
    buf[0] = frames.len() as u8;
    for (i, (pixels, delay_ms)) in frames.iter().enumerate() {
        if pixels.len() != size {
            return Err("A screen frame has the wrong size.".into());
        }
        buf[1 + i] = (delay_ms / 2).clamp(1, 255) as u8;
        buf[SCREEN_HEAD + i * size..SCREEN_HEAD + (i + 1) * size].copy_from_slice(pixels);
    }
    Ok(buf)
}

/// RGB → RGB565, low byte first.
pub fn rgb565(r: u8, g: u8, b: u8) -> [u8; 2] {
    let v = ((r as u16 & 0xf8) << 8) | ((g as u16 & 0xfc) << 3) | (b as u16 >> 3);
    v.to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    // byte layouts as open-aula's PROTOCOL.md describes them
    #[test]
    fn commands_and_payloads() {
        assert_eq!(command(op::KEYMAP, 0, 9)[..10], hex("04 11 00 00 00 00 00 00 09 00")[..]);
        assert_eq!(command(op::SETTINGS, 1, 1)[..3], [0x04, 0x17, 0x01]);
        let l = light_payload(&Lighting { mode: 7, rgb: [1, 2, 3], colorful: true, brightness: 4, speed: 2, direction: 3 });
        assert_eq!(l[..12], hex("07 01 02 03 00 00 00 00 01 04 02 03")[..]);
        assert_eq!(l[62..], [0xaa, 0x55]);
        let s = settings_payload(&Settings { win_lock: true, sleep: 2, response: 3, ..Settings::default() });
        assert_eq!(s[..9], hex("00 01 01 00 00 00 02 00 03")[..]);
        let t = time_payload(&Time { year: 2026, month: 10, day: 22, hour: 9, minute: 5, second: 30, weekday: 4 }, 1);
        assert_eq!(t[..11], hex("00 01 5a 1a 0a 16 09 05 1e 00 04")[..]);
    }

    #[test]
    fn keymap_entries() {
        assert_eq!(key_entry(0x04, 0x01), [0x02, 0x01, 0x04, 0]); // Ctrl+A
        assert_eq!(key_entry(0xe1, 0), [0x02, 0x02, 0, 0]); // left Shift alone
        assert_eq!(media_entry(0x00cd), [0x03, 0xcd, 0x00, 0]); // play/pause
        assert_eq!(mouse_entry(MOUSE[1].1), [0x01, 0x01, 0x02, 0]);
        assert_eq!(macro_entry(2, 1, 5), [0x06, 2, 1, 5]);
        let b = keymap_block(&[(1, DISABLED)]);
        assert_eq!(b[4..8], DISABLED);
        assert_eq!(b[BLOCK - 2..], [0xaa, 0x55]);
    }

    #[test]
    fn perkey_round_trip() {
        let b = perkey_block(&[(38, [10, 20, 30])]);
        assert_eq!(b[152..156], [38, 10, 20, 30]);
        assert_eq!(decode_perkey(&b, &[38]), vec![(38, [10, 20, 30])]);
    }

    #[test]
    fn macros_layout() {
        let m = vec![vec![], vec![Step::KeyDown { usage: 4 }, Step::Delay { ms: 5 }, Step::KeyUp { usage: 4 }]];
        let (data, packets) = macro_block(&m).unwrap();
        assert_eq!(data[0..4], [0xff; 4]); // the first is empty
        assert_eq!(data[4..6], 400u16.to_le_bytes()); // the second starts after the table
        assert_eq!(data[400..402], [3, 0]); // three steps
        assert_eq!(data[408..420], hex("00 00 04 b0 0a 00 00 50 00 00 04 30")[..]); // delay raised to 10 ms
        // 420 bytes used: 6 whole packets, a part one and one more, as the vendor driver counts
        assert_eq!((packets, data.len()), (8, 8 * 64));
        assert_eq!(data[data.len() - 2..], [0xaa, 0x55]);
        let frames = radio_macros(&m).unwrap();
        assert_eq!(frames.len(), 420 / 28 + 1); // 15 whole chunks + 1
        assert_eq!(frames.last().unwrap()[1], 0); // 420 % 28 = 0
    }

    #[test]
    fn dongle_frames() {
        let f = radio_frame(radio::BATTERY, 1, 0, &[]);
        assert_eq!(f[..3], [0x20, 0x01, 0x00]);
        assert_eq!(f[31], 0x21);
        let s = radio_short(radio::LIGHT, &light_payload(&Lighting::default()));
        assert_eq!((s[0], s[1], s[2]), (0x05, 0x10, 0));
        assert_eq!(s[3 + 14..3 + 16], [0xaa, 0x55]);
        let b = radio_block(radio::KEYMAP, &keymap_block(&[]));
        assert_eq!(b.len(), 21);
        assert_eq!((b[0][1], b[20][1], b[20][2]), (0x1c, 0x10, 20));
        assert_eq!(b[20][3 + 14..3 + 16], [0xaa, 0x55]); // 576 = 20 × 28 + 16: the tag ends the last frame
    }

    #[test]
    fn screen_images() {
        let frame = vec![0u8; SCREEN_W * SCREEN_H * 2];
        let img = screen_image(&[(frame.clone(), 100), (frame, 1)]).unwrap();
        assert_eq!((img[0], img[1], img[2]), (2, 50, 1));
        assert_eq!(img.len() % SCREEN_CHUNK, 0);
        assert_eq!(img.len(), (SCREEN_HEAD + 2 * 64_800).div_ceil(4096) * 4096);
        assert_eq!(rgb565(255, 0, 0), [0x00, 0xf8]);
        assert_eq!(rgb565(0, 0, 255), [0x1f, 0x00]);
        assert!(screen_image(&[]).is_err());
    }
}
