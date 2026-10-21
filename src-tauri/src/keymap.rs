//! AULA F75 key layout. Ported from aula-f75-hub (`aula/keymap.py`, MIT).
//!
//! Every key has a matrix index (0-125). The same index addresses the key in the keymap
//! table (command 0x01/0x41, 4 bytes per key) and in the per-key RGB table (0x02/0x42).
//! Positions are drawn on a plain 75% grid (see `k`), measured in key units, so the drawing
//! is our own. The keyboard holds RAlt at 53 and Fn at 59.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Key {
    pub index: u8,
    pub name: &'static str,
    pub label: &'static str,
    /// x1, y1, x2, y2 on the drawing (about 660 x 290 px); all zero = not drawn (the knob).
    pub rect: [u16; 4],
}

/// The drawing's grid: a quarter of a key (u) per step, 37 px per u, from (30, 30). The
/// function row sits a quarter u above the rest, as on the keyboard.
const ORIGIN: u16 = 30;
const U: u16 = 37;

/// A key at column `col` and width `w` (both in quarter u) on row `row` (0 = function row).
const fn k(index: u8, name: &'static str, label: &'static str, col: u16, row: u16, w: u16) -> Key {
    let x = ORIGIN + col * U / 4;
    let y = ORIGIN + if row == 0 { 0 } else { (5 + 4 * (row - 1)) * U / 4 };
    let x2 = ORIGIN + (col + w) * U / 4;
    Key { index, name, label, rect: [x + 6, y + 5, x2 - 6, y + U - 5] }
}

/// The knob has an index but isn't drawn.
const fn knob(index: u8) -> Key {
    Key { index, name: "knob", label: "Knob", rect: [0, 0, 0, 0] }
}

pub const KEYS: [Key; 81] = [
    k(0, "esc", "Esc", 0, 0, 4),
    k(12, "f1", "F1", 5, 0, 4),
    k(18, "f2", "F2", 9, 0, 4),
    k(24, "f3", "F3", 13, 0, 4),
    k(30, "f4", "F4", 17, 0, 4),
    k(36, "f5", "F5", 22, 0, 4),
    k(42, "f6", "F6", 26, 0, 4),
    k(48, "f7", "F7", 30, 0, 4),
    k(54, "f8", "F8", 34, 0, 4),
    k(60, "f9", "F9", 39, 0, 4),
    k(66, "f10", "F10", 43, 0, 4),
    k(72, "f11", "F11", 47, 0, 4),
    k(78, "f12", "F12", 51, 0, 4),
    k(1, "grave", "`", 0, 1, 4),
    k(7, "1", "1", 4, 1, 4),
    k(13, "2", "2", 8, 1, 4),
    k(19, "3", "3", 12, 1, 4),
    k(25, "4", "4", 16, 1, 4),
    k(31, "5", "5", 20, 1, 4),
    k(37, "6", "6", 24, 1, 4),
    k(43, "7", "7", 28, 1, 4),
    k(49, "8", "8", 32, 1, 4),
    k(55, "9", "9", 36, 1, 4),
    k(61, "0", "0", 40, 1, 4),
    k(67, "minus", "-", 44, 1, 4),
    k(73, "equal", "=", 48, 1, 4),
    k(79, "backspace", "Backspace", 52, 1, 8),
    k(85, "delete", "Del", 61, 1, 4),
    k(2, "tab", "Tab", 0, 2, 6),
    k(8, "q", "Q", 6, 2, 4),
    k(14, "w", "W", 10, 2, 4),
    k(20, "e", "E", 14, 2, 4),
    k(26, "r", "R", 18, 2, 4),
    k(32, "t", "T", 22, 2, 4),
    k(38, "y", "Y", 26, 2, 4),
    k(44, "u", "U", 30, 2, 4),
    k(50, "i", "I", 34, 2, 4),
    k(56, "o", "O", 38, 2, 4),
    k(62, "p", "P", 42, 2, 4),
    k(68, "lbracket", "[", 46, 2, 4),
    k(74, "rbracket", "]", 50, 2, 4),
    k(80, "backslash", "\\", 54, 2, 6),
    k(86, "pgup", "PgUp", 61, 2, 4),
    k(3, "capslock", "Caps", 0, 3, 7),
    k(9, "a", "A", 7, 3, 4),
    k(15, "s", "S", 11, 3, 4),
    k(21, "d", "D", 15, 3, 4),
    k(27, "f", "F", 19, 3, 4),
    k(33, "g", "G", 23, 3, 4),
    k(39, "h", "H", 27, 3, 4),
    k(45, "j", "J", 31, 3, 4),
    k(51, "k", "K", 35, 3, 4),
    k(57, "l", "L", 39, 3, 4),
    k(63, "semicolon", ";", 43, 3, 4),
    k(69, "quote", "'", 47, 3, 4),
    k(81, "enter", "Enter", 51, 3, 9),
    k(87, "pgdn", "PgDn", 61, 3, 4),
    k(4, "lshift", "Shift", 0, 4, 9),
    k(10, "z", "Z", 9, 4, 4),
    k(16, "x", "X", 13, 4, 4),
    k(22, "c", "C", 17, 4, 4),
    k(28, "v", "V", 21, 4, 4),
    k(34, "b", "B", 25, 4, 4),
    k(40, "n", "N", 29, 4, 4),
    k(46, "m", "M", 33, 4, 4),
    k(52, "comma", ",", 37, 4, 4),
    k(58, "period", ".", 41, 4, 4),
    k(64, "slash", "/", 45, 4, 4),
    k(70, "rshift", "Shift", 49, 4, 7),
    k(82, "up", "↑", 57, 4, 4),
    k(88, "end", "End", 61, 4, 4),
    k(5, "lctrl", "Ctrl", 0, 5, 5),
    k(11, "lwin", "Win", 5, 5, 5),
    k(17, "lalt", "Alt", 10, 5, 5),
    k(35, "space", "Space", 15, 5, 25),
    k(53, "ralt", "Alt", 40, 5, 4),
    k(59, "fn", "Fn", 44, 5, 4),
    k(77, "left", "←", 53, 5, 4),
    k(83, "down", "↓", 57, 5, 4),
    k(89, "right", "→", 61, 5, 4),
    knob(84),
];

pub fn key(index: u8) -> Option<&'static Key> {
    KEYS.iter().find(|k| k.index == index)
}

pub fn key_by_name(name: &str) -> Option<&'static Key> {
    let n = name.trim().to_ascii_lowercase();
    KEYS.iter().find(|k| k.name == n)
}

/// USB HID usage codes that a key can be remapped to (name, label, usage).
pub const HID: &[(&str, &str, u8)] = &[
    ("a", "A", 0x04), ("b", "B", 0x05), ("c", "C", 0x06), ("d", "D", 0x07), ("e", "E", 0x08),
    ("f", "F", 0x09), ("g", "G", 0x0A), ("h", "H", 0x0B), ("i", "I", 0x0C), ("j", "J", 0x0D),
    ("k", "K", 0x0E), ("l", "L", 0x0F), ("m", "M", 0x10), ("n", "N", 0x11), ("o", "O", 0x12),
    ("p", "P", 0x13), ("q", "Q", 0x14), ("r", "R", 0x15), ("s", "S", 0x16), ("t", "T", 0x17),
    ("u", "U", 0x18), ("v", "V", 0x19), ("w", "W", 0x1A), ("x", "X", 0x1B), ("y", "Y", 0x1C),
    ("z", "Z", 0x1D),
    ("1", "1", 0x1E), ("2", "2", 0x1F), ("3", "3", 0x20), ("4", "4", 0x21), ("5", "5", 0x22),
    ("6", "6", 0x23), ("7", "7", 0x24), ("8", "8", 0x25), ("9", "9", 0x26), ("0", "0", 0x27),
    ("enter", "Enter", 0x28), ("esc", "Esc", 0x29), ("backspace", "Backspace", 0x2A),
    ("tab", "Tab", 0x2B), ("space", "Space", 0x2C), ("minus", "-", 0x2D), ("equal", "=", 0x2E),
    ("lbracket", "[", 0x2F), ("rbracket", "]", 0x30), ("backslash", "\\", 0x31),
    ("iso_hash", "ISO #", 0x32), ("semicolon", ";", 0x33), ("quote", "'", 0x34),
    ("grave", "`", 0x35), ("comma", ",", 0x36), ("period", ".", 0x37), ("slash", "/", 0x38),
    ("capslock", "Caps Lock", 0x39),
    ("f1", "F1", 0x3A), ("f2", "F2", 0x3B), ("f3", "F3", 0x3C), ("f4", "F4", 0x3D),
    ("f5", "F5", 0x3E), ("f6", "F6", 0x3F), ("f7", "F7", 0x40), ("f8", "F8", 0x41),
    ("f9", "F9", 0x42), ("f10", "F10", 0x43), ("f11", "F11", 0x44), ("f12", "F12", 0x45),
    ("printscreen", "Print Screen", 0x46), ("scrolllock", "Scroll Lock", 0x47),
    ("pause", "Pause", 0x48), ("insert", "Insert", 0x49), ("home", "Home", 0x4A),
    ("pgup", "Page Up", 0x4B), ("delete", "Delete", 0x4C), ("end", "End", 0x4D),
    ("pgdn", "Page Down", 0x4E), ("right", "→", 0x4F), ("left", "←", 0x50), ("down", "↓", 0x51),
    ("up", "↑", 0x52), ("iso_backslash", "ISO \\", 0x64), ("menu", "Menu", 0x65),
    ("f13", "F13", 0x68), ("f14", "F14", 0x69), ("f15", "F15", 0x6A), ("f16", "F16", 0x6B),
    ("f17", "F17", 0x6C), ("f18", "F18", 0x6D), ("f19", "F19", 0x6E), ("f20", "F20", 0x6F),
    ("f21", "F21", 0x70), ("f22", "F22", 0x71), ("f23", "F23", 0x72), ("f24", "F24", 0x73),
];

/// Modifier keys: stored as bits in the second byte of a keymap entry (name, label, bit).
pub const MODIFIERS: &[(&str, &str, u8)] = &[
    ("lctrl", "Left Ctrl", 0x01), ("lshift", "Left Shift", 0x02), ("lalt", "Left Alt", 0x04),
    ("lwin", "Left Win", 0x08), ("rctrl", "Right Ctrl", 0x10), ("rshift", "Right Shift", 0x20),
    ("ralt", "Right Alt", 0x40), ("rwin", "Right Win", 0x80),
];

pub fn hid_usage(name: &str) -> Option<u8> {
    HID.iter().find(|(n, _, _)| *n == name).map(|t| t.2)
}

pub fn hid_name(usage: u8) -> Option<&'static str> {
    HID.iter().find(|t| t.2 == usage).map(|t| t.0)
}
