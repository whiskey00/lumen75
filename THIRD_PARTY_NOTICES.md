# Third-party notices

This project's own code is under the MIT License (see `LICENSE`). Parts of it come from, or
were informed by, the projects below.

## aula-f75-hub (code and data, MIT)

<https://github.com/bakiakyol/aula-f75-hub>

These parts are ported to Rust from aula-f75-hub's Python code:

- the packet format and the commands (`src-tauri/src/protocol.rs`)
- reads, writes and retries (`src-tauri/src/device.rs`)
- the Windows HID access (`src-tauri/src/win.rs`)
- the macro format (`src-tauri/src/macros.rs`)
- the key layout and HID usages (`src-tauri/src/keymap.rs`)

The factory table dumps in `src-tauri/factory/` are copied from its `backup/` folder. Those
parts stay under its licence:

```
MIT License

Copyright (c) 2026 Baki Akyol

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## aula-f75-linux-drivers (protocol reference only)

<https://github.com/cloudglides/aula-f75-linux-drivers>

The wired real-time frame header (`wired_live_frame` in `protocol.rs`) matches what this
project sends, and its notes on which effect numbers are safe over the cable were followed.
No code is copied from it.

## Riot Games (League lighting)

The League lighting reads Riot's Live Client Data API, which League of Legends serves on your
own computer during a match. `src-tauri/certs/riotgames.pem` is Riot's public root certificate,
as published in Riot's developer documentation, used only to check that local connection.

Lumen75 isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games
or anyone officially involved in producing or managing Riot Games properties. Riot Games, and
all associated properties are trademarks or registered trademarks of Riot Games, Inc.

## Aurora (ideas only, MIT)

<https://github.com/Aurora-RGB/Aurora>

How Aurora handles League's live data (for example treating an error reply as "no match")
informed the League lighting. No code is copied from it.

## AULA

AULA and F75 are trademarks of their owner. This project isn't affiliated with, endorsed
by or supported by AULA. It contains no AULA software or artwork. The keyboard protocol was
worked out so that this app can work with the keyboard. The key positions in `keymap.rs` are
coordinates from the official app's layout file.

## Libraries

The app is built on open-source libraries (Tauri, React, shadcn/ui, Radix, cpal, rustfft and
others), each under its own licence. See `package.json`, `src-tauri/Cargo.toml` and their
lock files. The components in `src/components/ui/` are generated from shadcn/ui (MIT).
