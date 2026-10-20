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
- **Audio**: the lights follow whatever the computer plays (Spectrum, Mirror, Ripple, Pulse),
  with gain, smoothness, colourful or one colour, and a choice of output device. Audio is
  captured with WASAPI loopback, so no microphone is needed. Frames stream with the real-time
  command `0x88`, about 14 a second over 2.4G. Nothing is stored on the keyboard.
- **Animations**: Stream, Breathing, Wave, Sparkle and Gradient in your own colours (1-7,
  with presets), plus speed, brightness and direction. The F75's own multicolour effects use a
  fixed rainbow that ignores the colour palettes (tested), so these stream from the app like the
  audio lighting. They pause when the computer is idle, so the keyboard can sleep.
- **Tray and Start with Windows**: closing the window keeps the app in the tray (Open,
  Animation on/off, Quit). Autostart starts it hidden and resumes your last animation.
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
- `audio.rs`: loopback capture (cpal), FFT bands with automatic level, the modes, and colour
  groups that fit the 126-byte frame budget.
- `anim.rs`: the animations as functions of time, in fixed bands so frames stay small and
  smooth; the streaming thread with idle pause.
- `app.rs`: saved settings, tray, autostart, single instance.
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

The icon is `app-icon.svg`. Regenerate every size with `npx tauri icon app-icon.svg`.

## Credits

The protocol, effect numbers, key layout and macro format come from
[aula-f75-hub](https://github.com/bakiakyol/aula-f75-hub) by Baki Akyol (MIT). Its Python code
is ported to Rust here, and its factory table dumps are bundled in `src-tauri/factory/` (see
`NOTICE.md` there). Not affiliated with AULA.
