//! AULA F75 key layout. Ported from aula-f75-hub (`aula/keymap.py`, MIT).
//!
//! Every key has a matrix index (0-125). The same index addresses the key in the keymap
//! table (command 0x01/0x41, 4 bytes per key) and in the per-key RGB table (0x02/0x42).
//! Positions come from the official app's KB.ini (x1, y1, x2, y2 in pixels). KB.ini labels
//! 53 as "FN" and 59 as "RCtrl", but the keyboard holds RAlt at 53 and Fn at 59.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Key {
    pub index: u8,
    pub name: &'static str,
    pub label: &'static str,
    /// x1, y1, x2, y2 in the official app's picture; all zero = not drawn (the knob).
    pub rect: [u16; 4],
}

const fn k(index: u8, name: &'static str, label: &'static str, rect: [u16; 4]) -> Key {
    Key { index, name, label, rect }
}

pub const KEYS: [Key; 81] = [
    k(0, "esc", "Esc", [30, 30, 55, 57]),
    k(12, "f1", "F1", [107, 30, 132, 57]),
    k(18, "f2", "F2", [144, 30, 169, 57]),
    k(24, "f3", "F3", [183, 30, 208, 57]),
    k(30, "f4", "F4", [222, 30, 247, 57]),
    k(36, "f5", "F5", [269, 30, 294, 57]),
    k(42, "f6", "F6", [305, 30, 330, 57]),
    k(48, "f7", "F7", [343, 30, 368, 57]),
    k(54, "f8", "F8", [388, 30, 413, 57]),
    k(60, "f9", "F9", [439, 30, 464, 57]),
    k(66, "f10", "F10", [477, 30, 502, 57]),
    k(72, "f11", "F11", [516, 30, 541, 57]),
    k(78, "f12", "F12", [555, 30, 580, 57]),
    k(1, "grave", "`", [31, 81, 56, 108]),
    k(7, "1", "1", [70, 81, 95, 108]),
    k(13, "2", "2", [108, 81, 133, 108]),
    k(19, "3", "3", [147, 81, 172, 108]),
    k(25, "4", "4", [184, 81, 209, 108]),
    k(31, "5", "5", [223, 81, 248, 108]),
    k(37, "6", "6", [262, 81, 287, 108]),
    k(43, "7", "7", [300, 81, 325, 108]),
    k(49, "8", "8", [338, 81, 363, 108]),
    k(55, "9", "9", [377, 81, 402, 108]),
    k(61, "0", "0", [416, 81, 441, 108]),
    k(67, "minus", "-", [453, 81, 478, 108]),
    k(73, "equal", "=", [491, 81, 516, 108]),
    k(79, "backspace", "Backspace", [531, 81, 591, 108]),
    k(85, "delete", "Del", [604, 81, 629, 108]),
    k(2, "tab", "Tab", [34, 120, 78, 147]),
    k(8, "q", "Q", [89, 119, 114, 146]),
    k(14, "w", "W", [127, 119, 152, 146]),
    k(20, "e", "E", [165, 119, 190, 146]),
    k(26, "r", "R", [204, 119, 229, 146]),
    k(32, "t", "T", [242, 119, 267, 146]),
    k(38, "y", "Y", [280, 119, 305, 146]),
    k(44, "u", "U", [318, 119, 343, 146]),
    k(50, "i", "I", [356, 119, 381, 146]),
    k(56, "o", "O", [395, 119, 420, 146]),
    k(62, "p", "P", [433, 119, 458, 146]),
    k(68, "lbracket", "[", [471, 119, 496, 146]),
    k(74, "rbracket", "]", [509, 119, 534, 146]),
    k(80, "backslash", "\\", [549, 119, 593, 146]),
    k(86, "pgup", "PgUp", [605, 120, 630, 147]),
    k(3, "capslock", "Caps", [33, 158, 85, 185]),
    k(9, "a", "A", [98, 158, 123, 185]),
    k(15, "s", "S", [136, 158, 161, 185]),
    k(21, "d", "D", [174, 158, 199, 185]),
    k(27, "f", "F", [213, 158, 238, 185]),
    k(33, "g", "G", [252, 158, 277, 185]),
    k(39, "h", "H", [290, 158, 315, 185]),
    k(45, "j", "J", [327, 158, 352, 185]),
    k(51, "k", "K", [366, 158, 391, 185]),
    k(57, "l", "L", [405, 158, 430, 185]),
    k(63, "semicolon", ";", [443, 158, 468, 185]),
    k(69, "quote", "'", [481, 158, 506, 185]),
    k(81, "enter", "Enter", [521, 158, 591, 185]),
    k(87, "pgdn", "PgDn", [604, 159, 629, 186]),
    k(4, "lshift", "Shift", [33, 195, 105, 222]),
    k(10, "z", "Z", [116, 195, 141, 222]),
    k(16, "x", "X", [155, 195, 180, 222]),
    k(22, "c", "C", [193, 195, 218, 222]),
    k(28, "v", "V", [232, 195, 257, 222]),
    k(34, "b", "B", [270, 195, 295, 222]),
    k(40, "n", "N", [309, 195, 334, 222]),
    k(46, "m", "M", [347, 195, 372, 222]),
    k(52, "comma", ",", [384, 195, 409, 222]),
    k(58, "period", ".", [424, 195, 449, 222]),
    k(64, "slash", "/", [463, 195, 488, 222]),
    k(70, "rshift", "Shift", [502, 195, 558, 222]),
    k(82, "up", "↑", [567, 195, 592, 222]),
    k(88, "end", "End", [605, 195, 630, 222]),
    k(5, "lctrl", "Ctrl", [34, 234, 69, 261]),
    k(11, "lwin", "Win", [79, 234, 114, 261]),
    k(17, "lalt", "Alt", [122, 234, 157, 261]),
    k(35, "space", "Space", [174, 234, 400, 261]),
    k(53, "ralt", "Alt", [414, 234, 449, 261]),
    k(59, "fn", "Fn", [463, 234, 497, 261]),
    k(77, "left", "←", [529, 234, 554, 261]),
    k(83, "down", "↓", [568, 234, 593, 261]),
    k(89, "right", "→", [605, 234, 630, 261]),
    k(84, "knob", "Knob", [0, 0, 0, 0]),
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
