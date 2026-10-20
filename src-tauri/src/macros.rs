//! Macro storage (command 0x03 / 0x43). Ported from aula-f75-hub (`aula/macros.py`, MIT),
//! which took the format from the official app's StMacro_To_HdMacro and checked it against
//! the keyboard's factory macro.
//!
//! Store (first 512-byte block):
//!   header  one [address u16, size u16] entry per macro (little endian); the first address
//!           equals the header size, so it gives the count
//!   macro   [name length in bytes] + name + events (4 bytes each)
//! Event:
//!   byte 0  (kind << 4) | delay bits 19..16, plus 0x80 for a release
//!           kind: 0 key (HID usage), 1 modifier (0xE0-0xE7), 2 mouse button, 3/4 mouse X/Y, 5 wheel
//!   byte 1-2 delay bits 15..0 (ms until the next event)
//!   byte 3  code
//! Keymap entry of a macro key: [0x03, repeat mode, repeat count, macro index].

use crate::keymap;
use crate::protocol::{err, Entry, Result, MACRO_STORE_LEN};
use serde::Serialize;
use std::collections::HashMap;

/// The firmware writes 5A A5 over bytes 126-127 of the store, so no macro may cover them.
const RESERVED: (usize, usize) = (126, 128);
const MAX_DELAY: u32 = 0xFFFFF;
pub const KIND_KEY: u8 = 0;
pub const KIND_MOD: u8 = 1;
pub const KIND_MOUSE: u8 = 2;

pub const REPEAT_TIMES: u8 = 1;
pub const REPEAT_HOLD: u8 = 2;
pub const REPEAT_UNTIL_KEY: u8 = 4;
pub const MACRO_ENTRY: u8 = 0x03;

pub const MOD_CODES: [(&str, u8); 8] = [
    ("lctrl", 0xE0), ("lshift", 0xE1), ("lalt", 0xE2), ("lwin", 0xE3),
    ("rctrl", 0xE4), ("rshift", 0xE5), ("ralt", 0xE6), ("rwin", 0xE7),
];
const MOD_ALIASES: [(&str, &str); 6] = [
    ("ctrl", "lctrl"), ("shift", "lshift"), ("alt", "lalt"), ("win", "lwin"), ("super", "lwin"), ("altgr", "ralt"),
];

fn mod_code(name: &str) -> Option<u8> {
    MOD_CODES.iter().find(|(n, _)| *n == name).map(|t| t.1)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub kind: u8,
    pub code: u8,
    pub down: bool,
    /// ms until the next event
    pub delay: u32,
}

impl Event {
    pub fn encode(&self) -> [u8; 4] {
        let d = self.delay.min(MAX_DELAY);
        let mut b0 = (self.kind << 4) | ((d >> 16) & 0x0F) as u8;
        if !self.down && matches!(self.kind, KIND_KEY | KIND_MOD | KIND_MOUSE) {
            b0 |= 0x80;
        }
        [b0, (d >> 8) as u8, d as u8, self.code]
    }

    pub fn decode(b: &[u8]) -> Self {
        Event {
            kind: (b[0] >> 4) & 0x07,
            code: b[3],
            down: b[0] & 0x80 == 0,
            delay: (((b[0] & 0x0F) as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32,
        }
    }

    pub fn describe(&self) -> String {
        let name = match self.kind {
            KIND_MOD => MOD_CODES.iter().find(|t| t.1 == self.code).map(|t| t.0.to_string()),
            KIND_KEY => keymap::hid_name(self.code).map(str::to_string),
            _ => Some(format!("kind{}:0x{:02x}", self.kind, self.code)),
        }
        .unwrap_or_else(|| format!("0x{:02x}", self.code));
        format!("{name}{}{}ms", if self.down { "↓" } else { "↑" }, self.delay)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Macro {
    pub name: String,
    pub events: Vec<Event>,
    /// Name bytes as read from the keyboard, kept unchanged on rewrite.
    pub raw_name: Option<Vec<u8>>,
}

impl Macro {
    pub fn new(name: &str, events: Vec<Event>) -> Self {
        Macro { name: name.to_string(), events, raw_name: None }
    }

    fn name_bytes(&self) -> Vec<u8> {
        if let Some(raw) = &self.raw_name {
            if decode_name(raw) == self.name {
                return raw.clone();
            }
        }
        let mut b: Vec<u8> = self.name.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        b.truncate(60);
        b
    }

    pub fn encode(&self) -> Vec<u8> {
        let n = self.name_bytes();
        let mut out = vec![n.len() as u8];
        out.extend(n);
        for e in &self.events {
            out.extend(e.encode());
        }
        out
    }
}

fn decode_name(raw: &[u8]) -> String {
    if !raw.is_empty() && raw.iter().all(|&c| (0x20..0x7F).contains(&c)) {
        // the factory macro uses plain ASCII
        return raw.iter().map(|&c| c as char).collect();
    }
    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

pub fn parse_store(raw: &[u8]) -> Vec<Macro> {
    let first = u16::from_le_bytes([raw[0], raw[1]]) as usize;
    if first == 0 || first % 4 != 0 || first > raw.len() {
        return vec![];
    }
    let mut out = Vec::new();
    for i in 0..first / 4 {
        let addr = u16::from_le_bytes([raw[4 * i], raw[4 * i + 1]]) as usize;
        let size = u16::from_le_bytes([raw[4 * i + 2], raw[4 * i + 3]]) as usize;
        let body = &raw[addr.min(raw.len())..(addr + size).min(raw.len())];
        if body.is_empty() {
            break;
        }
        let nlen = body[0] as usize;
        let name = body[1..(1 + nlen).min(body.len())].to_vec();
        let events = body[(1 + nlen).min(body.len())..].chunks_exact(4).map(Event::decode).collect();
        out.push(Macro { name: decode_name(&name), events, raw_name: Some(name) });
    }
    out
}

/// Lay the macros out by address, keeping them clear of the bytes the firmware overwrites
/// (a macro that would cross them starts after them).
pub fn build_store(macros: &[Macro]) -> Result<Vec<u8>> {
    let mut data = vec![0u8; 4 * macros.len()];
    if data.len() > RESERVED.0 {
        return err("too many macros");
    }
    for (i, m) in macros.iter().enumerate() {
        let body = m.encode();
        let mut addr = data.len();
        if addr < RESERVED.1 && addr + body.len() > RESERVED.0 {
            addr = RESERVED.1;
            data.resize(addr, 0);
        }
        data[4 * i..4 * i + 2].copy_from_slice(&(addr as u16).to_le_bytes());
        data[4 * i + 2..4 * i + 4].copy_from_slice(&(body.len() as u16).to_le_bytes());
        data.extend(body);
    }
    if data.len() > MACRO_STORE_LEN {
        return err(format!("macros need {} bytes, the keyboard holds {MACRO_STORE_LEN}", data.len()));
    }
    data.resize(MACRO_STORE_LEN, 0);
    Ok(data)
}

pub fn macro_entry(index: u8, mode: u8, count: u8) -> Entry {
    let count = if mode == REPEAT_TIMES { count.max(1) } else { 1 };
    [MACRO_ENTRY, mode, count, index]
}

// ------------------------------------------------------------ steps

/// The computer's keyboard layout, which turns key positions into characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Us,
    TurkishQ,
}

/// key name -> characters on that key (plain, with Shift, with AltGr)
fn layout_keys(layout: Layout) -> Vec<(&'static str, &'static str)> {
    let mut us: Vec<(&'static str, &'static str)> = vec![
        ("1", "1!"), ("2", "2@"), ("3", "3#"), ("4", "4$"), ("5", "5%"), ("6", "6^"), ("7", "7&"),
        ("8", "8*"), ("9", "9("), ("0", "0)"), ("minus", "-_"), ("equal", "=+"), ("lbracket", "[{"),
        ("rbracket", "]}"), ("backslash", "\\|"), ("semicolon", ";:"), ("quote", "'\""), ("grave", "`~"),
        ("comma", ",<"), ("period", ".>"), ("slash", "/?"), ("space", " "), ("enter", "\n"), ("tab", "\t"),
        ("a", "aA"), ("b", "bB"), ("c", "cC"), ("d", "dD"), ("e", "eE"), ("f", "fF"), ("g", "gG"),
        ("h", "hH"), ("i", "iI"), ("j", "jJ"), ("k", "kK"), ("l", "lL"), ("m", "mM"), ("n", "nN"),
        ("o", "oO"), ("p", "pP"), ("q", "qQ"), ("r", "rR"), ("s", "sS"), ("t", "tT"), ("u", "uU"),
        ("v", "vV"), ("w", "wW"), ("x", "xX"), ("y", "yY"), ("z", "zZ"),
    ];
    if layout == Layout::TurkishQ {
        let tr: [(&'static str, &'static str); 31] = [
            ("grave", "\"é<"), ("1", "1!>"), ("2", "2'£"), ("3", "3^#"), ("4", "4+$"), ("5", "5%½"),
            ("6", "6&¾"), ("7", "7/{"), ("8", "8(["), ("9", "9)]"), ("0", "0=}"), ("minus", "*?\\"),
            ("equal", "-_|"), ("q", "qQ@"), ("e", "eE€"), ("t", "tT₺"), ("u", "uUû"), ("i", "ıIî"),
            ("o", "oOô"), ("a", "aAâ"), ("s", "sSß"), ("lbracket", "ğĞ"), ("rbracket", "üÜ~"),
            ("semicolon", "şŞ"), ("quote", "iİ"), ("backslash", ",;`"), ("iso_backslash", "<>|"),
            ("comma", "öÖ"), ("period", "çÇ"), ("slash", ".:"), ("space", " "),
        ];
        for (k, v) in tr {
            match us.iter_mut().find(|e| e.0 == k) {
                Some(e) => e.1 = v,
                None => us.push((k, v)),
            }
        }
    }
    us
}

/// character -> (key name, modifiers); the simplest way to type it wins.
fn char_table(layout: Layout) -> HashMap<char, (&'static str, &'static [&'static str])> {
    const LEVELS: [&[&str]; 3] = [&[], &["lshift"], &["ralt"]];
    let keys = layout_keys(layout);
    let mut t = HashMap::new();
    for (level, mods) in LEVELS.iter().enumerate() {
        for (key, chars) in &keys {
            if let Some(c) = chars.chars().nth(level) {
                t.entry(c).or_insert((*key, *mods));
            }
        }
    }
    t
}

fn step_key(name: &str) -> Result<(u8, u8)> {
    let name = MOD_ALIASES.iter().find(|(a, _)| *a == name).map(|t| t.1).unwrap_or(name);
    if let Some(c) = mod_code(name) {
        return Ok((KIND_MOD, c));
    }
    if let Some(u) = keymap::hid_usage(name) {
        return Ok((KIND_KEY, u));
    }
    err(format!("unknown key in macro step: {name:?}"))
}

fn tap(keys: &[(u8, u8)], delay: u32) -> Vec<Event> {
    let mut v: Vec<Event> = keys.iter().map(|&(kind, code)| Event { kind, code, down: true, delay }).collect();
    v.extend(keys.iter().rev().map(|&(kind, code)| Event { kind, code, down: false, delay }));
    v
}

/// Turn steps like `ctrl+c`, `wait:100`, `text:hello`, `down:shift` into events.
/// Text steps are typed with the keys that give those characters on `layout`.
pub fn parse_steps(steps: &[String], delay: u32, layout: Layout) -> Result<Vec<Event>> {
    let chars = char_table(layout);
    let mut events: Vec<Event> = Vec::new();
    for raw in steps {
        let step = raw.trim();
        if step.is_empty() {
            continue;
        }
        if let Some(ms) = step.strip_prefix("wait:") {
            let ms: u32 = ms.trim().parse().map_err(|_| crate::protocol::ProtocolError(format!("bad wait: {step:?}")))?;
            match events.last_mut() {
                Some(e) => e.delay += ms,
                None => return err("a macro can't start with a wait"),
            }
        } else if let Some(text) = raw.trim_start().strip_prefix("text:") {
            for ch in text.chars() {
                let Some((key, mods)) = chars.get(&ch) else {
                    return err(format!("text steps can't type {ch:?} with this keyboard layout"));
                };
                let mut keys: Vec<(u8, u8)> = mods.iter().map(|m| (KIND_MOD, mod_code(m).unwrap())).collect();
                keys.push((KIND_KEY, keymap::hid_usage(key).unwrap()));
                events.extend(tap(&keys, delay));
            }
        } else if let Some((action @ ("down" | "up"), name)) = step.split_once(':') {
            let (kind, code) = step_key(&name.trim().to_ascii_lowercase())?;
            events.push(Event { kind, code, down: action == "down", delay });
        } else {
            let keys = step.to_ascii_lowercase().split('+').map(|p| step_key(p.trim())).collect::<Result<Vec<_>>>()?;
            events.extend(tap(&keys, delay));
        }
    }
    if events.is_empty() {
        return err("a macro needs at least one step");
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::factory;

    fn steps(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn factory_macro_round_trips() {
        let raw = factory::macros();
        let ms = parse_store(&raw);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].name, "M1");
        let d: Vec<String> = ms[0].events.iter().map(Event::describe).collect();
        assert_eq!(d, ["lctrl↓10ms", "grave↓10ms", "lctrl↑10ms", "grave↑10ms"]);
        assert_eq!(build_store(&ms).unwrap(), raw);
    }

    #[test]
    fn steps_to_events() {
        let ev = parse_steps(&steps(&["ctrl+c", "wait:100", "text:Hi"]), 15, Layout::Us).unwrap();
        let d: Vec<String> = ev[..4].iter().map(Event::describe).collect();
        assert_eq!(d, ["lctrl↓15ms", "c↓15ms", "c↑15ms", "lctrl↑115ms"]);
        assert_eq!(ev[4].describe(), "lshift↓15ms");
        assert_eq!(ev.len(), 4 + 4 + 2);
        assert!(parse_steps(&steps(&["text:ç"]), 20, Layout::Us).is_err());
        assert!(parse_steps(&steps(&["text:ç"]), 20, Layout::TurkishQ).is_ok());
        assert!(parse_steps(&steps(&["wait:5"]), 20, Layout::Us).is_err());
    }

    #[test]
    fn store_layout_and_entry() {
        let a = Macro::new("a", parse_steps(&steps(&["x"]), 20, Layout::Us).unwrap());
        let b = Macro::new("b", parse_steps(&steps(&["y"]), 20, Layout::Us).unwrap());
        let store = build_store(&[a, b]).unwrap();
        assert_eq!(store[..4], [8, 0, 11, 0]);
        assert_eq!(parse_store(&store).len(), 2);
        assert_eq!(macro_entry(1, REPEAT_TIMES, 3), [3, 1, 3, 1]);
    }

    #[test]
    fn store_skips_reserved_bytes() {
        let a = Macro::new("a", parse_steps(&steps(&["text:aaaaaaaaaaaaaa"]), 20, Layout::Us).unwrap());
        let b = Macro::new("b", parse_steps(&steps(&["x"]), 20, Layout::Us).unwrap());
        let store = build_store(&[a, b]).unwrap();
        assert_eq!(u16::from_le_bytes([store[4], store[5]]), 128);
    }

    #[test]
    fn utf16_names() {
        let m = Macro::new("Mërhaba", parse_steps(&steps(&["x"]), 20, Layout::Us).unwrap());
        let back = parse_store(&build_store(&[m]).unwrap());
        assert_eq!(back[0].name, "Mërhaba");
    }
}
