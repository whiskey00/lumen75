# Lumen75

An unofficial desktop app for the **AULA F75** keyboard: lighting, per-key RGB, key remapping, macros and
device settings, without the official software. Tauri 2 + React + TypeScript + shadcn/ui, Windows.

Works over the **2.4G dongle** (`3554:FA09`) and **Bluetooth** (`3554:FA07` over LE, `3554:FA08` over classic Bluetooth, which is how Windows pairs it). Over the
**USB cable** (`258A:010C`) it reads lighting, palettes and the keymap, changes lighting and device settings, and runs
animations and audio lighting so far; custom colours, key remapping, per-key colours and macros
still need the dongle or Bluetooth.

**League of Legends lighting** (Games page): while you play a match, the keyboard shows bars
(health, mana, gold, level, respawn), a dead state and effects for kills, objectives and the
result, all customisable and saved as presets. It uses only the match data League shares on your
PC (Riot's Live Client Data API, so no cooldowns), runs over the USB cable and the 2.4G dongle,
and brings back your animation or audio lighting when the match ends.

Pick the connection in the header: **Auto** (whichever is there, the cable first), **USB cable**,
**2.4G dongle** or **Bluetooth**. The choice is saved and used from the next start.

## Features

- **Lighting**: the 16 firmware effects with brightness (0-9), speed (0-4) and colour. Any RGB
  colour, written into the effect's palette, and factory colours back with one click.
- **Per-key**: paint keys on the drawn keyboard (groups such as WASD, arrows and F-keys), then
  apply. This switches the keyboard to the per-key effect. Save layouts as your own presets.
- **Keys**: remap any key on the base or Fn layer to a key, a key combo or a modifier, run a
  macro from it, disable it, or reset it to its factory function. Every change is read back and
  checked.
- **Macros**: steps such as `text:Hello`, `ctrl+c`, `wait:100` and `down:shift`, stored on the
  keyboard and assigned to a key (run N times, while held, or until a key is pressed). Text is
  typed for the computer's layout (US or Turkish Q).
- **Audio**: the lights follow whatever the computer plays (Spectrum, Mirror, Ripple, Pulse),
  with gain, smoothness, rainbow or your own colours (1-7), your own saved presets, and a choice
  of output device. Audio is
  captured with WASAPI loopback, so no microphone is needed. Frames stream with the real-time
  command `0x88`, about 14 a second over 2.4G. Nothing is stored on the keyboard.
- **Animations**: Stream, Breathing, Wave, Sparkle and Gradient in your own colours (1-7,
  with presets), plus speed, brightness and direction. Save whole animations as your own presets. The F75's own multicolour effects use a
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

```bash
cd src-tauri && cargo test
```

The tests check packets against values captured from a real keyboard, round-trip the factory
macro byte for byte, and parse a real backup.

The icon is `app-icon.svg`; regenerate every size with `npx tauri icon app-icon.svg`. The tray uses
`tray-icon.svg` (just the letters, readable at 16-24 px), rendered to `src-tauri/icons/tray*.png`.

Installers: `npm run tauri build` puts an MSI and an NSIS setup in `src-tauri/target/release/bundle/`.

## Credits

The protocol, effect numbers, key layout and macro format come from
[aula-f75-hub](https://github.com/bakiakyol/aula-f75-hub) by Baki Akyol (MIT). Its Python code
is ported to Rust here, and its factory table dumps are bundled in `src-tauri/factory/`. The
wired real-time header follows
[aula-f75-linux-drivers](https://github.com/cloudglides/aula-f75-linux-drivers). Details are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Built with AI assistance (Claude), then tested by hand on a real F75.

## Licence

MIT, see [LICENSE](LICENSE). The parts from aula-f75-hub stay under its MIT licence.

## Disclaimer

Lumen75 isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games
or anyone officially involved in producing or managing Riot Games properties. Riot Games, and
all associated properties are trademarks or registered trademarks of Riot Games, Inc.


This is an unofficial, independent project. It isn't affiliated with, endorsed by or
supported by AULA. AULA and F75 are trademarks of their owner and are used here only to say
which keyboard this app works with. Use it at your own risk: it writes to the keyboard's
settings, so make a backup first (Device → Backup).
