# League of Legends lighting: design

Date: 2026-10-21. Status: draft for the owner's approval.

## Intent

While the owner plays League of Legends, the F75 shows what's happening in the match: bars for
values like health and mana, short effects for moments like kills, objectives and the result,
and a distinct look while dead. It's built from pieces the owner chooses and can save as presets,
not a fixed design. When the match ends, the keyboard goes back to whatever was on before.

This is the first game. The structure should let other games with official data be added later,
but only League is in scope now.

## Goals

- React to a live League match using **only Riot's official Live Client Data API**.
- Fully customisable: which bars, which keys, which colours, which events and how they look.
- Named presets, like animations and per-key layouts today, and a good ready-made default.
- Take over automatically when a match starts and hand back when it ends.
- Work over the **USB cable** and the **2.4G dongle**.

## Non-goals

- Other games (later, separately).
- Ability or summoner-spell cooldowns: the API doesn't provide them.
- Bluetooth: too much lag for live lighting (measured earlier in this project).
- Lighting outside a live match (client, lobby, champ select): the API only exists in a match.

## Anti-goals (failure even if it technically works)

- Pretending to be Razer Synapse or Logitech G Hub to receive League's built-in lighting, or
  reading game memory, injecting, or anything else that could touch Vanguard (anti-cheat).
- Typing feeling worse during a match. The stream must not delay key presses.
- The keyboard being left in game lighting, or with the wrong thing running, after a match.
- Turning off certificate checking for anything other than the local League endpoint.

## Where the data comes from (official)

During a match, League serves `https://127.0.0.1:2999/liveclientdata/…` on the player's PC
(self-signed certificate, Riot root certificate published in Riot's developer docs). It doesn't
exist outside a match. Useful endpoints:

- `activeplayer`: `championStats.currentHealth`, `maxHealth`, `resourceValue`, `resourceMax`,
  `resourceType` (MANA, ENERGY, NONE, …), `level`, `currentGold`.
- `playerlist`: each player's team (ORDER/CHAOS), `isDead`, `respawnTimer`, scores.
- `eventdata`: events with an increasing `EventID`: `GameStart`, `ChampionKill` (killer, victim,
  assisters), `FirstBlood`, `Multikill` (`KillStreak` 2-5), `Ace`, `DragonKill` (`DragonType`,
  `Stolen`), `HeraldKill`, `BaronKill`, `TurretKilled`, `InhibKilled`, `GameEnd` (`Result`).
- `activeplayername` to find yourself in the player list and in kill events.

## How it behaves

### Turning it on

A new **Games** page has a "League of Legends lighting" switch (saved in settings). While it's
on and the app is running, it checks for a match about once a second. The check is cheap and
quiet when no match is running (connection refused, no error shown).

### When a match starts and ends

- **Start:** when the endpoint first answers with match data, League lighting takes over the
  keyboard. Whatever was streaming stops, but is remembered: an animation, audio lighting (with
  its current settings), or nothing.
- **End:** on `GameEnd`, or when the endpoint stops answering for a few seconds (left the game,
  crash), the result effect (if any) plays, then the remembered thing comes back. If nothing was
  running, streaming simply stops and the keyboard returns to its own effect.
- **The owner starts something else during a match** (an animation or audio lighting from the
  app or the tray): that choice wins. League lighting steps aside until the next match, and
  nothing is restored at the end of this one.
- **Connection:** on the cable and the dongle it runs. If the keyboard is on Bluetooth, or not
  found, it doesn't take over, and the Games page says why in one plain line.

### What's on the keyboard

Layered from the bottom up. The top layer wins on a key.

1. **Background:** for keys no bar uses. A solid colour, the player's team colour (blue side or
   red side), or off.
2. **Bars:** zero or more. Each has:
   - a value: health, resource (mana/energy/…), gold, level (1-18), or respawn countdown;
   - keys: number row, F1-F12, or any keys painted on the keyboard picture (in order);
   - colours: one for full and one for empty, blended in between;
   - a direction: left to right, or right to left.
   For values without a natural maximum (gold) the bar has a maximum the owner sets.
3. **Dead state:** while the player is dead, this replaces the background and bars: by default
   everything dims to grey with the respawn countdown as a bar on the number row. Customisable
   like the rest, or off.
4. **Event effects:** on top of everything, for a moment. For each event: on/off, an effect
   (flash, pulse or wave), a colour, and a duration. Events:
   your kill, your death, your assist, first blood, multi-kill (double, triple, quadra, penta,
   stronger for higher streaks), ace (your team's), dragon (your team's, a colour per drake type
   by default), Herald, Baron, turret taken, inhibitor taken, your level up, win, lose.
   "Your team's" vs the enemy's is decided from the player list. Events that happen while
   another effect plays queue up briefly or replace it (builder's call, see below).

### Presets

The whole setup (background, bars, dead state, events) is one preset. Presets can be saved by
name, picked, renamed by saving over, and deleted, the same way animation and per-key presets
work. The app ships with a default: health on the number row (green to red), resource on
F1-F12 in the resource's colour, team colour background, the grey dead state, and effects on
for kills, deaths, multi-kills, objectives, win and lose.

## Constraints

- **Existing streaming handover:** only one thing streams at a time (animation and audio already
  stop each other, `app.rs` / `lib.rs`). League lighting joins that rule as a third streamer,
  using the same `send_frame(kb, open, groups)` path, so cable and dongle both work with no
  transport code of its own.
- **Frame format:** frames are colour groups `(Rgb, keys)` by matrix index. The cable shows
  every key's own colour; the dongle has a 126-byte frame budget, so colours have to be grouped
  (bars in a few shades). `anim.rs` already has `fit()` for this.
- **Frame rate:** steady, like animations (14 frames/s). Over the cable the keyboard falls back
  to its own effect if frames stop for roughly half a second; over the dongle after about 2 s.
- **Polling:** match data every 250-500 ms is plenty; events are read by `EventID` so none are
  missed or repeated, and only events newer than the takeover moment trigger effects (so joining
  mid-match doesn't replay old kills).
- **HTTPS:** the app has no HTTP client yet. Certificate trust must be limited to the local
  endpoint, ideally by pinning Riot's root certificate.
- **UI:** shadcn/ui, forced dark theme, plain short sentences (CLAUDE.md). The key picker reuses
  the existing keyboard picture component.
- **Settings:** in `settings.json` like the rest (switch, presets, current setup).
- **Tests:** `cargo test` and `npx tsc --noEmit` before committing. The data-to-frame logic
  (bars, dead state, event effects, team detection, event de-duplication) must be testable
  without a running game, from recorded JSON.

## Decided by the owner

- League first; only the official Live Client Data API.
- Cable and dongle only.
- Fully customisable, built from bars, event effects and a background, with presets.
- Takes over at match start; at the end, goes back to the last thing that was on.

## My calls (the owner can change these)

- The list of bar values, events and effect styles (flash, pulse, wave), taken from what the API
  provides.
- The dead state as its own layer.
- The owner starting something else mid-match wins for the rest of that match.
- A Games page with one switch per game, rather than putting it under Animations.
- Checking for a match about once a second; polling match data every 250-500 ms.
- The switch is off by default. It works with the window closed to the tray.
- It runs in every mode with an active player (ranked, normal, ARAM, practice tool, custom),
  but not when spectating or watching a replay (no active player), so those never take over.
- No idle pause during a match (the animations' idle pause doesn't apply).
- A champion without a resource (resource type NONE) leaves the resource bar's keys to the
  background.
- The Games page shows a live preview of the keyboard, like the Animations page.

## Left to the builder

- Which HTTP/TLS crate, and how Riot's root certificate is pinned.
- Exact effect animations and timings, and whether overlapping events queue or replace.
- How bars are quantised into colour groups on the dongle.
- The settings JSON shape, as long as old settings files still load.
- Layout of the Games page and the effect editor, within the existing UI style.
