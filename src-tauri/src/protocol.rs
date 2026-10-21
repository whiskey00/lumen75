//! AULA F75 wireless (2.4G dongle / Bluetooth) configuration protocol.
//!
//! Ported from aula-f75-hub (`aula/protocol.py`, MIT, (c) 2026 Baki Akyol), which recovered
//! it from the official app (OemDrv.exe, class CDev3632) and verified it on hardware.
//!
//! Packet (20 bytes, HID report 0x13):
//! ```text
//! [0]    0x13           report id
//! [1]    command        read commands = write command | 0x40
//! [2]    packet count   in replies, bit 7 (0x80) = error
//! [3]    packet index
//! [4]    (sub << 4) | payload length (max 14)
//! [5:19] payload
//! [19]   sum of the first 19 bytes & 0xFF
//! ```

use serde::Serialize;

pub const REPORT_ID: u8 = 0x13;
pub const PACKET_LEN: usize = 20;
pub const CHUNK: usize = 14;

pub const CMD_SET_MATRIX: u8 = 0x01; // keymap, sub = layer (0 base, 1 Fn)
pub const CMD_SET_CUSTOM: u8 = 0x02; // per-key RGB table (custom effect)
pub const CMD_SET_MACRO: u8 = 0x03;
pub const CMD_SET_LED: u8 = 0x04; // effect / brightness / speed / colour and device settings
pub const CMD_SET_PALETTE: u8 = 0x09; // per-effect 7-colour palettes
pub const CMD_PROFILE: u8 = 0x0A; // reply's first byte is probably the battery %
pub const CMD_LIVE: u8 = 0x88; // real-time frame: not acknowledged, not stored (see live_frame)
pub const READ: u8 = 0x40;

/// Sub of CMD_LIVE: per-key colours as colour groups.
pub const LIVE_KEYS: u8 = 1;
/// Longest frame the official app sends.
pub const LIVE_MAX: usize = 200;
/// Longer frames (more than 9 packets) get lost more often (tested upstream).
pub const LIVE_BUDGET: usize = 126;
/// ms between packets: closer together, the keyboard drops frames (tested upstream).
pub const LIVE_GAP_MS: u64 = 5;
/// The same over Bluetooth: about one packet per radio connection interval (not yet
/// tested on hardware; a full frame then takes ~90 ms, so streams run at ~11 frames/s).
pub const LIVE_GAP_MS_BT: u64 = 10;

pub const MATRIX_LEN: usize = 126;
pub const MATRIX_BYTES: usize = MATRIX_LEN * 4; // 504
pub const CUSTOM_LEN: usize = MATRIX_LEN * 3; // 378
pub const LED_LEN: usize = 128;
pub const LED_MAGIC: [u8; 2] = [0x5A, 0xA5];
pub const PALETTE_LEN: usize = 490;
pub const MACRO_STORE_LEN: usize = 512;

const LED_EFFECT: usize = 10;
const LED_WIN_LOCK: usize = 15; // 1 = Windows key disabled (Fn+Win)
const LED_SIDE_MODE: usize = 18; // white bar next to Esc
const LED_SLEEP: usize = 24; // wireless sleep timer in units of 30 s, 0 = never
const LED_KNOB_MODE: usize = 26;
const LED_OS_MODE: usize = 27;
const LED_SLOTS_OFFSET: usize = 58;
pub const LED_SLOTS: u8 = 34;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProtocolError {}

pub type Result<T> = std::result::Result<T, ProtocolError>;

pub fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(ProtocolError(msg.into()))
}

// ------------------------------------------------------------ packets

pub type Packet = [u8; PACKET_LEN];

pub fn checksum(b: &[u8]) -> u8 {
    b[..19].iter().fold(0u8, |a, &x| a.wrapping_add(x))
}

pub fn build_packet(cmd: u8, total: u8, index: u8, sub: u8, chunk: &[u8]) -> Packet {
    assert!(chunk.len() <= CHUNK, "a packet carries at most 14 payload bytes");
    let mut b = [0u8; PACKET_LEN];
    b[0] = REPORT_ID;
    b[1] = cmd;
    b[2] = total;
    b[3] = index;
    b[4] = (sub << 4) | chunk.len() as u8;
    b[5..5 + chunk.len()].copy_from_slice(chunk);
    b[19] = checksum(&b);
    b
}

pub fn split_packets(cmd: u8, sub: u8, data: &[u8]) -> Vec<Packet> {
    if data.is_empty() {
        return vec![build_packet(cmd, 1, 0, sub, &[])];
    }
    let total = data.len().div_ceil(CHUNK) as u8;
    data.chunks(CHUNK)
        .enumerate()
        .map(|(i, c)| build_packet(cmd, total, i as u8, sub, c))
        .collect()
}

pub fn read_request(cmd: u8, sub: u8) -> Packet {
    build_packet(cmd | READ, 1, 0, sub, &[])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub cmd: u8,
    pub total: u8,
    pub index: u8,
    pub error: bool,
    pub data: Vec<u8>,
}

/// Decode a report from the keyboard; None if it isn't a valid config packet.
pub fn parse_packet(r: &[u8]) -> Option<Reply> {
    if r.len() < PACKET_LEN || r[0] != REPORT_ID || checksum(r) != r[19] {
        return None;
    }
    let len = (r[4] & 0x0F) as usize;
    Some(Reply {
        cmd: r[1],
        total: r[2] & 0x7F,
        index: r[3],
        error: r[2] & 0x80 != 0,
        data: r[5..5 + len.min(CHUNK)].to_vec(),
    })
}

// ------------------------------------------------------------ effects

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Effect {
    pub hw: u8,
    pub name: &'static str,
    pub label: &'static str,
    pub speed: bool,
    pub color: bool,
}

const fn fx(hw: u8, name: &'static str, label: &'static str, speed: bool, color: bool) -> Effect {
    Effect { hw, name, label, speed, color }
}

/// Firmware effect numbers as observed on a real F75 (every number 1-34 was tried by
/// aula-f75-hub). The official app's names and numbering don't match the firmware.
pub const EFFECTS: [Effect; 16] = [
    fx(1, "static", "Static", false, true),
    fx(2, "breathing", "Breathing", true, true),
    fx(3, "spectrum", "Spectrum", true, false),
    fx(4, "ripple-row", "Row ripple", true, true),
    fx(5, "raindrops", "Raindrops", true, true),
    fx(6, "wave", "Wave", true, true),
    fx(7, "ripple", "Ripple", true, true),
    fx(8, "twinkle", "Twinkle", true, true),
    fx(10, "snake", "Snake", true, true),
    fx(11, "stream", "Stream", true, true),
    fx(12, "reactive", "Reactive", true, true),
    fx(13, "center-wave", "Center wave", true, true),
    fx(15, "rainbow-wheel", "Rainbow wheel", true, false),
    fx(16, "waterfall", "Waterfall", true, false),
    fx(17, "center-flash", "Center flash", true, true),
    fx(32, "custom", "Per-key", false, false),
];
pub const CUSTOM_EFFECT: u8 = 32;

pub fn effect(hw: u8) -> Option<&'static Effect> {
    EFFECTS.iter().find(|e| e.hw == hw)
}

pub const COLORS: [&str; 8] = ["red", "green", "blue", "yellow", "purple", "cyan", "white", "multi"];
pub const MULTI: u8 = 7;
pub const SIDE_MODES: [&str; 4] = ["off", "fill", "breathing", "static"];
pub const KNOB_MODES: [&str; 2] = ["media", "lighting"];
pub const OS_MODES: [&str; 3] = ["windows", "android", "mac"];
pub const SLEEP_UNIT: u32 = 30;
pub const SLEEP_MAX: u32 = 1200;
/// What the official app offers (seconds); 0 = never.
pub const SLEEP_CHOICES: [u32; 10] = [0, 60, 90, 120, 180, 240, 300, 600, 900, 1200];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct EffectSlot {
    /// 0-9
    pub brightness: u8,
    /// 0-4
    pub speed: u8,
    /// 0-6 palette colour, 7 = multicolour
    pub color: u8,
}

/// The 128-byte lighting and device-settings block (read 0x44, write 0x04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedConfig(pub [u8; LED_LEN]);

impl LedConfig {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        if raw.len() != LED_LEN || raw[LED_LEN - 2..] != LED_MAGIC {
            return err("unexpected LED config block");
        }
        Ok(LedConfig(raw.try_into().unwrap()))
    }

    pub fn effect(&self) -> u8 {
        self.0[LED_EFFECT]
    }
    pub fn set_effect(&mut self, hw: u8) {
        self.0[LED_EFFECT] = hw;
    }

    pub fn side_mode(&self) -> u8 {
        self.0[LED_SIDE_MODE]
    }
    pub fn set_side_mode(&mut self, m: u8) -> Result<()> {
        if m as usize >= SIDE_MODES.len() {
            return err("side light mode must be 0-3");
        }
        self.0[LED_SIDE_MODE] = m;
        Ok(())
    }

    pub fn win_lock(&self) -> bool {
        self.0[LED_WIN_LOCK] != 0
    }
    pub fn set_win_lock(&mut self, on: bool) {
        self.0[LED_WIN_LOCK] = on as u8;
    }

    pub fn os_mode(&self) -> u8 {
        self.0[LED_OS_MODE]
    }
    pub fn set_os_mode(&mut self, m: u8) -> Result<()> {
        if m as usize >= OS_MODES.len() {
            return err("OS mode must be 0-2");
        }
        self.0[LED_OS_MODE] = m;
        Ok(())
    }

    pub fn knob_mode(&self) -> u8 {
        self.0[LED_KNOB_MODE]
    }
    pub fn set_knob_mode(&mut self, m: u8) -> Result<()> {
        if m as usize >= KNOB_MODES.len() {
            return err("knob mode must be 0-1");
        }
        self.0[LED_KNOB_MODE] = m;
        Ok(())
    }

    /// Seconds of idle time before the keyboard sleeps (wireless only); 0 = never.
    pub fn sleep(&self) -> u32 {
        self.0[LED_SLEEP] as u32 * SLEEP_UNIT
    }
    pub fn set_sleep(&mut self, seconds: u32) -> Result<()> {
        if seconds % SLEEP_UNIT != 0 || seconds > SLEEP_MAX {
            return err("sleep time must be a multiple of 30 s, at most 20 min (0 = never)");
        }
        self.0[LED_SLEEP] = (seconds / SLEEP_UNIT) as u8;
        Ok(())
    }

    fn slot_offset(hw: u8) -> Result<usize> {
        if !(1..=LED_SLOTS).contains(&hw) {
            return err(format!("effect {hw} has no settings slot"));
        }
        Ok(LED_SLOTS_OFFSET + 2 * (hw as usize - 1))
    }

    pub fn slot(&self, hw: u8) -> Result<EffectSlot> {
        let o = Self::slot_offset(hw)?;
        Ok(EffectSlot { brightness: self.0[o], speed: self.0[o + 1] >> 4, color: self.0[o + 1] & 0x0F })
    }

    pub fn set_slot(&mut self, hw: u8, s: EffectSlot) -> Result<()> {
        if s.brightness > 9 || s.speed > 4 || s.color > 7 {
            return err("brightness must be 0-9, speed 0-4, colour 0-7");
        }
        let o = Self::slot_offset(hw)?;
        self.0[o] = s.brightness;
        self.0[o + 1] = (s.speed << 4) | s.color;
        Ok(())
    }
}

// ------------------------------------------------------------ per-key colours

pub type Rgb = [u8; 3];

/// Per-key colours (index -> RGB) to the 378-byte planar table: all R, then all G, then all B.
pub fn custom_from_colors(colors: &[(u8, Rgb)]) -> Vec<u8> {
    let mut b = vec![0u8; CUSTOM_LEN];
    for &(i, [r, g, bl]) in colors {
        let i = i as usize;
        if i < MATRIX_LEN {
            b[i] = r;
            b[MATRIX_LEN + i] = g;
            b[2 * MATRIX_LEN + i] = bl;
        }
    }
    b
}

pub fn colors_from_custom(raw: &[u8]) -> Vec<Rgb> {
    (0..MATRIX_LEN).map(|i| [raw[i], raw[MATRIX_LEN + i], raw[2 * MATRIX_LEN + i]]).collect()
}

// ------------------------------------------------------------ real-time frames

/// Colour groups -> a real-time frame for CMD_LIVE / LIVE_KEYS: [R, G, B, count, index…] per
/// group. Keys that aren't listed go dark, so black groups are left out.
pub fn live_frame(groups: &[(Rgb, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for (rgb, keys) in groups {
        if keys.is_empty() || *rgb == [0, 0, 0] {
            continue;
        }
        let mut keys = keys.clone();
        keys.sort_unstable();
        out.extend_from_slice(rgb);
        out.push(keys.len() as u8);
        out.extend(keys);
    }
    if out.is_empty() {
        return Ok(vec![0, 0, 0, 1, 0]); // a single dark key: everything off
    }
    if out.len() > LIVE_MAX {
        return err(format!("real-time frame too long ({} bytes, at most {LIVE_MAX})", out.len()));
    }
    Ok(out)
}

// ------------------------------------------------------------ palettes

/// One palette per effect number 0-22, each holding the 7 colours an effect's colour
/// setting 0-6 picks from, then 7 bytes kept as read. Palette n belongs to effect n.
pub const PALETTE_COUNT: u8 = 23;
pub const PALETTE_COLORS: u8 = 7;
pub const DEFAULT_PALETTE: [Rgb; 7] = [
    [255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [255, 0, 255], [0, 255, 255], [255, 255, 255],
];

fn palette_offset(hw: u8, color: u8) -> Result<usize> {
    if hw >= PALETTE_COUNT {
        return err(format!("effect {hw} has no palette"));
    }
    if color >= PALETTE_COLORS {
        return err("palette colour must be 0-6");
    }
    Ok(hw as usize * PALETTE_COLORS as usize * 3 + color as usize * 3)
}

pub fn palette_color(raw: &[u8], hw: u8, color: u8) -> Result<Rgb> {
    let o = palette_offset(hw, color)?;
    Ok([raw[o], raw[o + 1], raw[o + 2]])
}

pub fn set_palette_color(raw: &mut [u8], hw: u8, color: u8, rgb: Rgb) -> Result<()> {
    let o = palette_offset(hw, color)?;
    raw[o..o + 3].copy_from_slice(&rgb);
    Ok(())
}

pub fn reset_palette(raw: &mut [u8], hw: u8) -> Result<()> {
    for (c, rgb) in DEFAULT_PALETTE.iter().enumerate() {
        set_palette_color(raw, hw, c as u8, *rgb)?;
    }
    Ok(())
}

// ------------------------------------------------------------ keymap entries

pub type Entry = [u8; 4];

pub fn matrix_entry(raw: &[u8], index: u8) -> Entry {
    let i = 4 * index as usize;
    raw[i..i + 4].try_into().unwrap()
}

pub fn set_matrix_entry(raw: &mut [u8], index: u8, entry: Entry) {
    let i = 4 * index as usize;
    raw[i..i + 4].copy_from_slice(&entry);
}

/// Plain key entry: [0, modifier bits, 0, HID usage].
pub fn key_entry(usage: u8, modifiers: u8) -> Entry {
    [0x00, modifiers, 0x00, usage]
}

// ------------------------------------------------------------ wired (USB cable)

// Wired mode (258A:010C) talks through 520-byte feature reports (id 0x06) on its own
// collection instead of 20-byte packets:
//
//   [0] 0x06  [1] command  [2] sub (keymap layer)  [3] 0  [4] 0x01  [5] 0
//   [6..8] length, little-endian  [8..] data
//
// A read sends the request, then fetches the reply, which echoes the header. The read
// commands below were found on hardware (2026-10-21) and return the same tables as the
// wireless reads; the reply is a whole 512-byte page, of which the table is the start.
// 0x88, 0x8b, 0x8e and 0x91 return firmware code: never needed.

pub const WIRED_REPORT_ID: u8 = 0x06;
pub const WIRED_REPORT_LEN: usize = 520;
pub const WIRED_HEADER: usize = 8;
const WIRED_PAGE: usize = 0x200;

pub const WIRED_READ_MATRIX: u8 = 0x83; // = 0x41, sub = layer
pub const WIRED_READ_LED: u8 = 0x84; // = 0x44
pub const WIRED_READ_PALETTE: u8 = 0x8A; // = 0x49

pub type WiredReport = [u8; WIRED_REPORT_LEN];

pub fn wired_request(cmd: u8, sub: u8, len: usize) -> WiredReport {
    let mut b = [0u8; WIRED_REPORT_LEN];
    b[0] = WIRED_REPORT_ID;
    b[1] = cmd;
    b[2] = sub;
    b[4] = 0x01;
    b[6..8].copy_from_slice(&(len as u16).to_le_bytes());
    b
}

/// The request for a table: the LED block is asked for by its own length, the others by page.
pub fn wired_read_request(cmd: u8, sub: u8) -> WiredReport {
    wired_request(cmd, sub, if cmd == WIRED_READ_LED { LED_LEN } else { WIRED_PAGE })
}

/// The first `len` data bytes of a reply to `cmd`/`sub`, or None if it answers something else.
pub fn wired_reply(r: &[u8], cmd: u8, sub: u8, len: usize) -> Option<Vec<u8>> {
    (r.len() >= WIRED_HEADER + len && r[0] == WIRED_REPORT_ID && r[1] == cmd && r[2] == sub)
        .then(|| r[WIRED_HEADER..WIRED_HEADER + len].to_vec())
}

// ------------------------------------------------------------ factory tables

/// Factory table dumps from aula-f75-hub's backup/ (a fresh keyboard).
pub mod factory {
    fn hex(s: &str) -> Vec<u8> {
        let s = s.trim();
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }
    pub fn matrix(layer: u8) -> Vec<u8> {
        hex(match layer {
            0 => include_str!("../factory/cmd_41.hex"),
            1 => include_str!("../factory/cmd_41_layer1.hex"),
            2 => include_str!("../factory/cmd_41_layer2.hex"),
            _ => include_str!("../factory/cmd_41_layer3.hex"),
        })
    }
    pub fn led() -> Vec<u8> {
        hex(include_str!("../factory/cmd_44.hex"))
    }
    pub fn palette() -> Vec<u8> {
        hex(include_str!("../factory/cmd_49.hex"))
    }
    pub fn custom() -> Vec<u8> {
        hex(include_str!("../factory/cmd_42.hex"))
    }
    pub fn macros() -> Vec<u8> {
        hex(include_str!("../factory/cmd_43.hex"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    // expected values from aula-f75-hub's tests, captured from real keyboard traffic
    #[test]
    fn read_request_matches_capture() {
        assert_eq!(read_request(CMD_SET_LED, 0).to_vec(), hex("13 44 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 58"));
        assert_eq!(read_request(CMD_PROFILE, 0)[19], 0x5E);
    }

    #[test]
    fn led_write_packets_match_capture() {
        let pkts = split_packets(CMD_SET_LED, 0, &factory::led());
        assert_eq!(pkts.len(), 10);
        assert_eq!(pkts[0][..5], hex("13 04 0a 00 0e")[..]);
        assert_eq!(pkts[9][..7], hex("13 04 0a 09 02 5a a5")[..]);
        assert!(pkts.iter().all(|p| checksum(p) == p[19]));
    }

    #[test]
    fn parse_reply() {
        let mut r = hex("13 44 0a 09 02 5a a5");
        r.extend([0u8; 12]);
        r.push(0x6b);
        let rep = parse_packet(&r).unwrap();
        assert_eq!((rep.cmd, rep.total, rep.index, rep.data.clone()), (0x44, 10, 9, vec![0x5a, 0xa5]));
        r[19] = 0;
        assert!(parse_packet(&r).is_none());
    }

    #[test]
    fn led_config_fields() {
        let mut led = LedConfig::parse(&factory::led()).unwrap();
        assert_eq!(SIDE_MODES[led.side_mode() as usize], "static");
        assert_eq!(KNOB_MODES[led.knob_mode() as usize], "media");
        assert_eq!(OS_MODES[led.os_mode() as usize], "windows");
        assert!(!led.win_lock());
        assert_eq!(led.sleep(), 60);
        led.set_sleep(300).unwrap();
        assert_eq!(led.0[24], 10);
        assert!(led.set_sleep(45).is_err() && led.set_sleep(1230).is_err());
        led.set_os_mode(2).unwrap();
        led.set_win_lock(true);
        assert_eq!((led.0[27], led.0[15]), (2, 1));
        assert!(led.set_side_mode(4).is_err());
        let mut s = led.slot(1).unwrap();
        s.color = 0;
        led.set_slot(1, s).unwrap();
        assert_eq!(led.slot(1).unwrap().color, 0);
        assert!(led.set_slot(1, EffectSlot { brightness: 10, speed: 0, color: 0 }).is_err());
    }

    #[test]
    fn custom_table_is_planar() {
        // live test upstream: Esc red, A green, W blue
        let raw = custom_from_colors(&[(0, [255, 0, 0]), (9, [0, 255, 0]), (14, [0, 0, 255])]);
        assert_eq!(raw.len(), 378);
        assert_eq!((raw[0], raw[126 + 9], raw[252 + 14]), (255, 255, 255));
        assert_eq!(raw.iter().map(|&b| b as u32).sum::<u32>(), 3 * 255);
        assert_eq!(colors_from_custom(&raw)[9], [0, 255, 0]);
    }

    #[test]
    fn palettes() {
        let mut p = factory::palette();
        assert_eq!(p.len(), PALETTE_LEN);
        assert_eq!(palette_color(&p, 2, 0).unwrap(), [255, 0, 0]);
        set_palette_color(&mut p, 2, 0, [0xff, 0x60, 0]).unwrap();
        assert_eq!(palette_color(&p, 2, 0).unwrap(), [0xff, 0x60, 0]);
        reset_palette(&mut p, 2).unwrap();
        assert_eq!(p, factory::palette());
    }

    #[test]
    fn factory_tables_have_the_right_sizes() {
        for l in 0..4 {
            assert_eq!(factory::matrix(l).len(), MATRIX_BYTES);
        }
        assert_eq!(factory::custom().len(), CUSTOM_LEN);
        assert_eq!(factory::macros().len(), MACRO_STORE_LEN);
        assert_eq!(matrix_entry(&factory::matrix(0), 0), [0, 0, 0, 0x29]); // Esc
    }

    // headers the keyboard echoed back over the cable (2026-10-21)
    #[test]
    fn wired_requests_and_replies() {
        assert_eq!(wired_read_request(WIRED_READ_LED, 0)[..8], hex("06 84 00 00 01 00 80 00")[..]);
        assert_eq!(wired_read_request(WIRED_READ_MATRIX, 1)[..8], hex("06 83 01 00 01 00 00 02")[..]);
        assert_eq!(wired_read_request(WIRED_READ_PALETTE, 0).len(), WIRED_REPORT_LEN);

        let mut r = wired_read_request(WIRED_READ_LED, 0);
        r[WIRED_HEADER..WIRED_HEADER + LED_LEN].copy_from_slice(&factory::led());
        assert_eq!(wired_reply(&r, WIRED_READ_LED, 0, LED_LEN), Some(factory::led()));
        assert_eq!(wired_reply(&r, WIRED_READ_PALETTE, 0, LED_LEN), None);
        assert_eq!(wired_reply(&r, WIRED_READ_LED, 1, LED_LEN), None);
    }
}
