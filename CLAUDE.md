# Working notes for AULA Studio

README.md covers features, layout and building. This file covers what's in progress and how
work is done here.

## Status (2026-10-21)

- Verified on hardware (2.4G dongle, Windows 11): lighting, per-key, keys, macros, device
  settings, backup/restore, audio lighting, animations, tray, autostart, presets.
- **Bluetooth (3554:FA07 LE / FA08 classic): detection and reads verified on hardware (Windows 10).
  Reads are slow** (see "Bluetooth reads" below). Writes, animations and audio over Bluetooth
  are not tested yet. Original test plan:
  1. Pair the F75 in Windows Bluetooth settings, switch it to Bluetooth, run `npm run tauri dev`.
  2. The header badge should say "Bluetooth" and everything should read (slower than 2.4G).
  3. Change a lighting effect (a write) and check it sticks after Refresh.
  4. Run an animation and audio lighting. Over Bluetooth, live frames use a 10 ms packet gap
     (`LIVE_GAP_MS_BT` in `protocol.rs`), which is a guess. If frames flicker or drop, try a
     bigger gap. If they're smooth, try smaller.
  5. If the keyboard isn't found at all, print the HID paths (`find_devices()` in `win.rs`).
     `maybe_aula()` expects the Bluetooth paths to contain `vid&023554` (LE) or `vid&00023554`
     (classic).
- **Wired (258A:010C): reads (lighting, palettes, keymap), lighting-block writes** (Lighting
  and Device pages) **and the live stream** (Animations, Audio), verified on hardware. Palette
  and keymap writes and per-key colours are next, one at a time.
  See "Wired protocol" below.
- **League lighting: built, not yet verified in a real match** (branch `league-lighting`). Code:
  `league.rs` (local HTTPS client pinned to Riot's root `certs/riotgames.pem`, parsing, your
  events), `gamelight.rs` (setup and renderer), `match_watch.rs` (match lifecycle, watcher
  thread), handover in `app.rs`, page `src/pages/games.tsx`. Design and plan in
  `docs/superpowers/`. Debug builds print each kill event's names (`[league] kill: ...`) to check
  Riot ID matching.
- Battery reading is experimental (first byte of the 0x4A reply).
- No git remote yet: the owner will add a GitHub repo and push.

## Protocol reference

- The packet format, commands and tables are in the comments in `src-tauri/src/protocol.rs`.
- The upstream project is aula-f75-hub (MIT, https://github.com/bakiakyol/aula-f75-hub),
  see its `docs/protocol.md`. Bluetooth there was verified on Linux only.
- Over Bluetooth, keyboard-to-PC packets get lost a lot (roughly every other one in a burst).
  Reads retry and collect parts by index. Writes resend unacknowledged packets.
- The firmware's "multicolour" uses a fixed rainbow, so custom-colour animations are streamed
  live (0x88, at most 126 bytes per frame, no ack, the keyboard falls back to its own effect after
  about 2 s).

## Bluetooth reads (measured 2026-10-21, Windows 10)

- The F75 has two Bluetooth radios that Windows lists separately: "AULA-F75 5.0 KB" is LE
  (`{00001812}`, FA07) and "AULA-F75 3.0 KB" is classic (`{00001124}`, FA08). Pair the 5.0 one.
- Probe, battery and writes work on both (writes are acknowledged per packet).
- Multi-packet replies lose most packets: the keyboard sends faster than the link carries.
  - Classic: 4-5 of 10 arrive, and a plain retry lost the same ones every time. Small profile
    requests before each retry shift the phase (in `read()`), then about half the reads worked.
  - LE: 4-7 packets per attempt, at random. With `read()` retrying while new packets still come
    in (stall limit 25, at most 60 attempts), 18 of 18 full reads worked, 4-40 s each. Reading
    everything takes about 2 minutes.
- Upstream's macOS port gave up on Bluetooth reads; its Linux setup uses LE (FA07).

## Wired protocol (found on hardware 2026-10-21)

- Config channel: collection `MI_01&COL06`, vendor page FF00, 520-byte feature report 0x06.
  A read is `HidD_SetFeature(request)` then `HidD_GetFeature`; the reply echoes the header.
- Read commands: 0x84 = lighting block (same 128 bytes as wireless 0x44), 0x8A = palettes
  (= 0x49), 0x83 sub 0/1 = keymap layers (= 0x41). 0x88/0x8B/0x8E/0x91 return firmware code.
- In wired mode keys 53/59 read as Fn/RCtrl (wireless dumps have RAlt/Fn). Show what's read.
- Write 0x04 = lighting block: applies at once, not acknowledged (a get echoes only the
  header), so `set_led` reads it back. Effect numbers are the same as wireless.
- Live stream: command 0x08, header `06 08 00 00 01 00 7A 01` (from cloudglides; never vary it,
  0x08 also writes memory in the official app), then RGB per key at its matrix index (checked:
  Esc, Q, A, Space, Enter, Up lit right). Not stored; needs a steady stream. One frame and a
  4 s stream left lighting, palette and keymap unchanged.
- Per-key colours and macros: not found yet. The per-key effect (32) is blocked on the cable.
  Palette/keymap writes: not tried yet.
- References: cloudglides/aula-f75-linux-drivers (MIT; its "colour table" 0x8A is the palette,
  and its effect numbers are F87's). FaizAsta/Aula-F75-software has no source or licence.

## How to work here

- Commit after each feature the owner has verified on the keyboard.
- Run `cargo test` in `src-tauri` and `npx tsc --noEmit` before committing.
- Installers: `npx tauri build` builds them into `src-tauri/target/release/bundle/{nsis,msi}`.
  Quit the running app from the tray before reinstalling.
- Close the official AULA app (OemDrv.exe) first: it uses the same channel.
- The UI uses shadcn/ui (radix-nova preset) with a forced dark theme. Write UI text in plain,
  short sentences.
