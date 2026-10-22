//! The S75 Pro's keys: our own 75% grid (see `keymap::k`), with the key index each one has in
//! the S75 Pro's keymap and per-key tables (the same number for both). The indexes are the
//! ones open-aula found in the vendor driver; the drawing is ours. Its screen sits right of
//! Enter and isn't a key.

use crate::keymap::{k, Key};

pub const KEYS: [Key; 80] = [
    // function row, Del at the right
    k(1, "esc", "Esc", 0, 0, 4),
    k(2, "f1", "F1", 5, 0, 4),
    k(3, "f2", "F2", 9, 0, 4),
    k(4, "f3", "F3", 13, 0, 4),
    k(5, "f4", "F4", 17, 0, 4),
    k(6, "f5", "F5", 22, 0, 4),
    k(7, "f6", "F6", 26, 0, 4),
    k(8, "f7", "F7", 30, 0, 4),
    k(9, "f8", "F8", 34, 0, 4),
    k(10, "f9", "F9", 39, 0, 4),
    k(11, "f10", "F10", 43, 0, 4),
    k(12, "f11", "F11", 47, 0, 4),
    k(13, "f12", "F12", 51, 0, 4),
    k(119, "delete", "Del", 61, 0, 4),
    // number row, Home at the right
    k(19, "grave", "`", 0, 1, 4),
    k(20, "1", "1", 4, 1, 4),
    k(21, "2", "2", 8, 1, 4),
    k(22, "3", "3", 12, 1, 4),
    k(23, "4", "4", 16, 1, 4),
    k(24, "5", "5", 20, 1, 4),
    k(25, "6", "6", 24, 1, 4),
    k(26, "7", "7", 28, 1, 4),
    k(27, "8", "8", 32, 1, 4),
    k(28, "9", "9", 36, 1, 4),
    k(29, "0", "0", 40, 1, 4),
    k(30, "minus", "-", 44, 1, 4),
    k(31, "equal", "=", 48, 1, 4),
    k(103, "backspace", "Backspace", 52, 1, 8),
    k(117, "home", "Home", 61, 1, 4),
    // QWERTY row, End at the right
    k(37, "tab", "Tab", 0, 2, 6),
    k(38, "q", "Q", 6, 2, 4),
    k(39, "w", "W", 10, 2, 4),
    k(40, "e", "E", 14, 2, 4),
    k(41, "r", "R", 18, 2, 4),
    k(42, "t", "T", 22, 2, 4),
    k(43, "y", "Y", 26, 2, 4),
    k(44, "u", "U", 30, 2, 4),
    k(45, "i", "I", 34, 2, 4),
    k(46, "o", "O", 38, 2, 4),
    k(47, "p", "P", 42, 2, 4),
    k(48, "lbracket", "[", 46, 2, 4),
    k(49, "rbracket", "]", 50, 2, 4),
    k(67, "backslash", "\\", 54, 2, 6),
    k(120, "end", "End", 61, 2, 4),
    // home row
    k(55, "capslock", "Caps", 0, 3, 7),
    k(56, "a", "A", 7, 3, 4),
    k(57, "s", "S", 11, 3, 4),
    k(58, "d", "D", 15, 3, 4),
    k(59, "f", "F", 19, 3, 4),
    k(60, "g", "G", 23, 3, 4),
    k(61, "h", "H", 27, 3, 4),
    k(62, "j", "J", 31, 3, 4),
    k(63, "k", "K", 35, 3, 4),
    k(64, "l", "L", 39, 3, 4),
    k(65, "semicolon", ";", 43, 3, 4),
    k(66, "quote", "'", 47, 3, 4),
    k(85, "enter", "Enter", 51, 3, 9),
    // bottom letter row
    k(73, "lshift", "Shift", 0, 4, 9),
    k(74, "z", "Z", 9, 4, 4),
    k(75, "x", "X", 13, 4, 4),
    k(76, "c", "C", 17, 4, 4),
    k(77, "v", "V", 21, 4, 4),
    k(78, "b", "B", 25, 4, 4),
    k(79, "n", "N", 29, 4, 4),
    k(80, "m", "M", 33, 4, 4),
    k(81, "comma", ",", 37, 4, 4),
    k(82, "period", ".", 41, 4, 4),
    k(83, "slash", "/", 45, 4, 4),
    k(84, "rshift", "Shift", 49, 4, 7),
    k(101, "up", "↑", 57, 4, 4),
    // space row
    k(91, "lctrl", "Ctrl", 0, 5, 5),
    k(92, "lwin", "Win", 5, 5, 5),
    k(93, "lalt", "Alt", 10, 5, 5),
    k(94, "space", "Space", 15, 5, 25),
    k(95, "ralt", "Alt", 40, 5, 4),
    k(96, "fn", "Fn", 44, 5, 4),
    k(98, "rctrl", "Ctrl", 48, 5, 4),
    k(99, "left", "←", 53, 5, 4),
    k(100, "down", "↓", 57, 5, 4),
    k(102, "right", "→", 61, 5, 4),
];

pub fn key_by_name(name: &str) -> Option<&'static Key> {
    let n = name.trim().to_ascii_lowercase();
    KEYS.iter().find(|k| k.name == n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_are_unique_and_fit_the_tables() {
        let mut seen = std::collections::HashSet::new();
        for k in &KEYS {
            assert!(seen.insert(k.index), "{} twice", k.index);
            assert!((k.index as usize) * 4 + 4 <= super::super::protocol::BLOCK - 2, "{} beyond the table", k.name);
        }
        assert_eq!(key_by_name("Q").unwrap().index, 38);
        assert_eq!(key_by_name("space").unwrap().index, 94);
    }
}
