# AULA Studio

A desktop app for the **AULA F75** keyboard: lighting, per-key RGB, key remapping, macros and
device settings, without the official software. Tauri 2 + React + TypeScript + shadcn/ui, Windows.

Works over the **2.4G dongle** (`3554:FA09`) and **Bluetooth** (`3554:FA07`). Wired mode
(`258A:010C`) uses a different transport and isn't supported yet.

## Features

- **Lighting**: the 16 firmware effects with brightness (0-9), speed (0-4) and colour. Any RGB
  colour, written into the effect's palette, and factory colours back with one click.
- **Per-key**: paint keys on the drawn keyboard (groups such as WASD, arrows and F-keys), then
  apply. This switches the keyboard to the per-key effect.
- **Keys**: remap any key on the base or Fn layer to a key, a key combo or a modifier, run a
  macro from it, disable it, or reset it to its factory function. Every change is read back and
  checked.
- **Macros**: steps such as `text:Hello`, `ctrl+c`, `wait:100` and `down:shift`, stored on the
  keyboard and assigned to a key (run N times, while held, or until a key is pressed). Text is
  typed for the computer's layout (US or Turkish Q).
- **Device**: side light, knob mode, OS mode, Windows-key lock and sleep timer. Battery level
  (experimental).
- **Backup / restore**: same hex files as `aula backup` / `aula restore`, so backups work with
  both tools.

Close the official AULA app first. It uses the same channel, and the app warns when it's running.

## How it works

`src-tauri/src/`:

- `protocol.rs`: 20-byte packets (report `0x13`, 14-byte chunks, checksum), the 128-byte LED
  and settings block, palettes, the per-key table, keymap entries.
- `device.rs`: reads (reassembled by packet index, retried) and writes (every packet echoed as an
  ack, retried), with a pause after writes. On Windows the dongle sometimes ignored a command
  sent straight after a write.
- `win.rs`: finds the configuration collection (usage page `0xFF02`), overlapped I/O, and only
  talks to the keyboard while no key is held. Otherwise the keyboard can drop a key release.
- `macros.rs`, `keymap.rs`: macro store format, step parser, key layout and HID usages.

## Develop

```bash
npm install
npm run tauri dev
```

The dev server uses port 1430, so it can run next to RAKK Studio (1420).

```bash
cd src-tauri && cargo test
```

The tests check packets against values captured from a real keyboard, round-trip the factory
macro byte for byte, and parse a real backup.

## Credits

The protocol, effect numbers, key layout and macro format come from
[aula-f75-hub](https://github.com/bakiakyol/aula-f75-hub) by Baki Akyol (MIT). Its Python code
is ported to Rust here, and its factory table dumps are bundled in `src-tauri/factory/` (see
`NOTICE.md` there). Not affiliated with AULA.
