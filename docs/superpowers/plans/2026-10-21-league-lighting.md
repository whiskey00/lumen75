# League of Legends Lighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** While a League match runs, the F75 shows customisable bars, a dead state and event effects from Riot's Live Client Data API, then hands the keyboard back to whatever was on before.

**Architecture:** A watcher thread polls `https://127.0.0.1:2999/liveclientdata/allgamedata`. Pure code turns each poll into a game snapshot plus your events, and a pure lifecycle state machine decides when a match starts, ends, or should step aside. While a match owns the keyboard, a renderer turns the snapshot, the setup and the active effects into colour groups, streamed with the existing `device::send_frame`. Taking over and handing back join the existing rule that only one thing streams at a time (`app.rs` animations, `lib.rs` audio).

**Tech Stack:** Rust (Tauri 2), `ureq` 2 + `rustls` 0.23 (new, for the local HTTPS endpoint), serde_json; React + TypeScript + shadcn/ui.

**Spec:** `docs/superpowers/specs/2026-10-21-league-lighting-design.md`

## Global Constraints

- Data source: **only** Riot's Live Client Data API on `127.0.0.1:2999`. No Razer/Logitech emulation, no memory reading, nothing that could touch Vanguard.
- Certificate trust limited to the local endpoint: verify against Riot's root certificate (pinned); never disable checking globally.
- Cable and dongle only. On Bluetooth it doesn't take over and the Games page says why in one plain line.
- Only one thing streams at a time; League joins animations and audio under that rule.
- Typing must not get worse during a match (frames go through the existing stream path; no key guard in it).
- After a match the keyboard goes back to the last thing that was on; never left in game lighting.
- Old `settings.json` files must still load (`AppConfig` is `#[serde(default)]`).
- UI: shadcn/ui, forced dark theme, plain short sentences.
- Before every commit: `cargo test` in `src-tauri` and `npx tsc --noEmit` at the repo root. Commit messages without attribution lines.

## Review Focus

1. **Riot ID names:** `activePlayer.riotId` is `Name#TAG`, while events may name you by `riotIdGameName` or the old `summonerName`. Your kills and deaths must still be recognised. Test: `events_match_you_by_any_of_your_names` (Task 2).
2. **Joining mid-match or starting the app during one:** old kills and objectives must not replay. Test: `first_poll_sets_a_baseline_without_effects` (Task 2).
3. **Leaving or crashing without `GameEnd`:** the endpoint just vanishes; the keyboard must still be handed back. Test: `endpoint_gone_for_three_seconds_ends_the_match` (Task 4).
4. **Spectating or watching a replay:** no active player, so it must never take over. Test: `spectator_data_is_not_a_match` (Task 2).
5. **You start an animation or audio mid-match:** League steps aside until the next match and nothing is restored at this match's end. Test: `stepping_aside_lasts_until_the_next_match` (Task 4).

---

## File structure

- Create `src-tauri/src/league.rs`: game data from the API: `parse`, `Snapshot`, `Team`, the `Tracker` that turns raw events into your `GameEvent`s, and the HTTPS client (`fetch`).
- Create `src-tauri/src/gamelight.rs`: the customisable setup (`Setup` and its parts, `Setup::default`) and `render`.
- Create `src-tauri/src/match_watch.rs`: the pure lifecycle state machine (`Watch`) and the Windows watcher thread (`run`).
- Create `src-tauri/certs/riotgames.pem`: Riot's root certificate.
- Create `src-tauri/tests/fixtures/league/*.json`: recorded/handwritten API replies.
- Modify `src-tauri/src/anim.rs`: make `fit` `pub(crate)`.
- Modify `src-tauri/src/app.rs`: config fields, presets, takeover/hand-back, commands, quit.
- Modify `src-tauri/src/lib.rs`: modules, `AppState.league`, register commands; audio start steps League aside.
- Modify `src-tauri/Cargo.toml`: `ureq`, `rustls`, `rustls-pemfile`.
- Create `src/pages/games.tsx`; modify `src/lib/api.ts`, `src/App.tsx`.
- Modify `README.md`, `CLAUDE.md`.

---

### Task 1: HTTPS client for the local League endpoint

**Files:**
- Create: `src-tauri/certs/riotgames.pem`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/league.rs` (client part only for now), register `mod league;` in `src-tauri/src/lib.rs`
- Test: in `league.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub fn fetch() -> Result<Option<String>, String>`: the body of `GET https://127.0.0.1:2999/liveclientdata/allgamedata`, `Ok(None)` when nothing listens (connection refused or timed out: no match), `Err` for anything else (bad certificate, HTTP error other than 404). Timeout 500 ms.

- [ ] **Step 1:** Download Riot's root certificate from `https://static.developer.riotgames.com/docs/lol/riotgames.pem` into `src-tauri/certs/riotgames.pem`. Check it starts with `-----BEGIN CERTIFICATE-----`. If the URL doesn't serve a certificate, stop and ask the owner.
- [ ] **Step 2: Write the failing test**

```rust
#[test]
fn riot_root_certificate_loads() {
    assert_eq!(riot_roots().len(), 1);
}
```

- [ ] **Step 3:** Run `cargo test --lib riot_root_certificate_loads` → FAIL (`riot_roots` not defined).
- [ ] **Step 4:** Add to `Cargo.toml` `[dependencies]`: `ureq = { version = "2", default-features = false, features = ["tls"] }`, `rustls = "0.23"`, `rustls-pemfile = "2"` (pick versions that build together). Implement `fn riot_roots() -> rustls::RootCertStore` (from `include_bytes!("../certs/riotgames.pem")`) and `fetch()`. The client uses a rustls `ClientConfig` whose verifier wraps `WebPkiServerVerifier` built from `riot_roots()` and accepts only the `NotValidForName` error (the certificate isn't issued for `127.0.0.1`; we only ever connect there). Build the agent once (`std::sync::OnceLock`).
- [ ] **Step 5:** Run `cargo test --lib riot_root_certificate_loads` → PASS; `cargo build` with no warnings.
- [ ] **Step 6:** Commit: `git add src-tauri/certs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/league.rs src-tauri/src/lib.rs && git commit -m "League lighting: client for the local Live Client Data endpoint"`

### Task 2: Game data: snapshot and your events

**Files:**
- Modify: `src-tauri/src/league.rs`
- Create: `src-tauri/tests/fixtures/league/` with `in_game.json` (you alive, mid game, a few events), `dead.json` (you dead, `respawnTimer` 23.5), `spectator.json` (no `activePlayer` / an `error` field instead), `riot_id.json` (your `riotId` `"Kai#EUW"`, `riotIdGameName` `"Kai"`, events naming you `"Kai"`), `game_end.json` (ends with `GameEnd`, `Result` `"Win"`). Shape: `allgamedata` as documented by Riot: `activePlayer` (`riotId`, `riotIdGameName`, `summonerName`, `level`, `currentGold`, `championStats` {`currentHealth`, `maxHealth`, `resourceValue`, `resourceMax`, `resourceType`}), `allPlayers[]` (`riotId`, `riotIdGameName`, `summonerName`, `team` `"ORDER"`/`"CHAOS"`, `isDead`, `respawnTimer`), `events.Events[]` (`EventID`, `EventName`, `EventTime`, plus per-event fields), `gameData` (`gameMode`, `gameTime`).
- Test: `league.rs` tests

**Interfaces:**
- Produces (all `pub`, serde `Serialize` where noted for the UI):

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)] pub enum Team { Order, Chaos }
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub team: Team,
    pub health: f32, pub max_health: f32,
    pub resource: f32, pub resource_max: f32,
    pub resource_kind: String,     // "MANA", "ENERGY", "NONE", ...
    pub level: u32, pub gold: f32,
    pub dead: bool, pub respawn: f32, // seconds left
}
pub struct RawEvent { pub id: u64, pub name: String, pub fields: serde_json::Value }
pub enum Parsed { NotPlaying, Playing(Snapshot, Vec<RawEvent>, Players) }
pub struct Players { /* your names; every player's names -> team */ }
pub fn parse(json: &str) -> Result<Parsed, String>;

// gamelight::Event is defined in Task 3; Tracker returns it.
pub struct GameEvent { pub event: crate::gamelight::Event, pub streak: u8, pub dragon: Option<String> }
pub struct Tracker { /* last EventID, last level, respawn total */ }
impl Tracker {
    pub fn new() -> Self;
    /// Your events since the last call. The first call only sets the baseline.
    pub fn update(&mut self, snap: &Snapshot, events: &[RawEvent], players: &Players) -> Vec<GameEvent>;
    /// Respawn bar fraction 0..1 (1 just died, 0 back alive): remaining / longest seen this death.
    pub fn respawn_fraction(&self, snap: &Snapshot) -> f32;
}
```

Task 3 defines `gamelight::Event`; write Task 2's code against the variant names below and add the enum in `gamelight.rs` now if Task 3 isn't done (Task 3 then extends that file).

Classification (who counts as "you" / "your team"):
- `ChampionKill`: `KillerName` is you → `Kill`; `VictimName` is you → `Death`; you in `Assisters` → `Assist`.
- `FirstBlood`: `Recipient` is you → `FirstBlood`.
- `Multikill`: `KillerName` is you → `Multikill` with `streak = KillStreak` (2-5).
- `Ace`: `AcingTeam` is your team → `Ace`.
- `DragonKill` / `HeraldKill` / `BaronKill`: `KillerName` is a player on your team → `Dragon` (with `dragon = DragonType`) / `Herald` / `Baron`.
- `TurretKilled` / `InhibKilled`: the structure name (`TurretKilled` / `InhibKilled` field) contains `_T1_` (Order's) or `_T2_` (Chaos's); it's yours to celebrate when the structure is the *other* team's → `Turret` / `Inhibitor`.
- `GameEnd`: `Result` `"Win"` → `Win`, `"Lose"` → `Lose`.
- Level increase between calls → `LevelUp` (one per call even if several levels).
- "You" matches any of your `riotId`, `riotIdGameName`, `summonerName` (case-insensitive, ignoring a `#TAG` suffix).

- [ ] **Step 1: Write the failing tests** (`fixture(name)` reads `tests/fixtures/league/<name>`):

```rust
#[test] fn in_game_data_parses_into_a_snapshot()      // in_game.json: team, health/max, resource kind, level, gold, dead=false
#[test] fn spectator_data_is_not_a_match()            // spectator.json -> Parsed::NotPlaying
#[test] fn first_poll_sets_a_baseline_without_effects() // Tracker::new().update(in_game...) is empty
#[test] fn new_events_after_the_baseline_are_yours()  // baseline, then the same data plus a ChampionKill by you (id+1) -> [Kill]; repeating the call -> []
#[test] fn events_match_you_by_any_of_your_names()    // riot_id.json: kill by "Kai" -> Kill; victim "kai#euw" -> Death
#[test] fn objectives_and_structures_count_for_your_team_only() // dragon by teammate -> Dragon(dragon=Some("Fire")); by enemy -> nothing; Turret_T2_... killed while you're ORDER -> Turret; Turret_T1_... -> nothing
#[test] fn multikill_carries_its_streak()             // KillStreak 5 -> Multikill streak 5
#[test] fn game_end_reports_win_or_lose()             // game_end.json after baseline at an earlier EventID -> [Win]
#[test] fn level_up_and_respawn_fraction()            // level 6->7 -> [LevelUp]; dead.json respawn 23.5 first seen -> fraction 1.0, later 11.75 -> 0.5
```

- [ ] **Step 2:** Run `cargo test --lib league::` → FAIL (types not defined).
- [ ] **Step 3:** Implement the interfaces above in `league.rs` (serde_json `Value` access; missing optional fields default, missing `activePlayer.championStats` → `NotPlaying`).
- [ ] **Step 4:** Run `cargo test --lib league::` → PASS.
- [ ] **Step 5:** Commit: `git add src-tauri/src/league.rs src-tauri/src/gamelight.rs src-tauri/tests/fixtures/league && git commit -m "League lighting: game snapshot and your events from the live data"`

### Task 3: The setup and the renderer

**Files:**
- Create/extend: `src-tauri/src/gamelight.rs`, register `pub mod gamelight;` in `lib.rs`
- Modify: `src-tauri/src/anim.rs` (`fn fit` → `pub(crate) fn fit`)
- Test: `gamelight.rs` tests

**Interfaces:**
- Consumes: `league::{Snapshot, Team, GameEvent}`, `audio::{leds, Led}`, `protocol::Rgb`.
- Produces (serde `Serialize, Deserialize`, `rename_all = "lowercase"` enums, structs `#[serde(default)]`; the same shapes are mirrored in TypeScript in Task 5):

```rust
pub enum Value { Health, Resource, Gold, Level, Respawn }
pub enum Style { Flash, Pulse, Wave }
pub enum Event { Kill, Death, Assist, FirstBlood, Multikill, Ace, Dragon, Herald, Baron, Turret, Inhibitor, LevelUp, Win, Lose } // serde: "kill", "firstblood", "levelup", ...
pub enum BackgroundKind { Off, Solid, Team }
pub struct Background { pub kind: BackgroundKind, pub color: Rgb }
pub struct Bar { pub value: Value, pub keys: Vec<u8>, pub full: Rgb, pub empty: Rgb, pub reverse: bool, pub max: u32 /* gold only */ }
pub struct DeadState { pub on: bool, pub dim: Rgb, pub countdown_keys: Vec<u8>, pub countdown: Rgb }
pub struct EventEffect { pub event: Event, pub on: bool, pub style: Style, pub color: Option<Rgb> /* None for Dragon = the drake's colour */, pub ms: u32 }
pub struct Setup { pub background: Background, pub bars: Vec<Bar>, pub dead: DeadState, pub events: Vec<EventEffect> }
impl Default for Setup;   // the spec's default, below

pub struct Active { pub effect: EventEffect, pub color: Rgb, pub start: f32, pub repeats: u8 }
pub fn activate(setup: &Setup, ev: &GameEvent, now: f32) -> Option<Active>;   // None when that event is off
pub fn render(setup: &Setup, snap: &Snapshot, respawn_fraction: f32, active: &mut Vec<Active>, leds: &[Led], now: f32) -> Vec<(Rgb, Vec<u8>)>;
```

Values pinned by the spec and this plan:
- Default setup: background `Team`; bar health on the number row `1`-`0` (matrix indexes looked up by key name in `keymap::KEYS`), full `[0,255,0]`, empty `[255,0,0]`; bar resource on `f1`-`f12`, full = resource colour (MANA `[0,120,255]`, ENERGY `[255,220,0]`, others `[255,255,255]`, chosen at render time when the bar's `full` is `[0,0,0]`), empty `[0,0,0]`; dead state on, dim `[40,40,40]`, countdown on the number row in `[255,255,255]`; events on: kill, death, multikill, ace, dragon, herald, baron, turret, inhibitor, win, lose; off: assist, firstblood, levelup. Durations 600 ms (flash), 1200 ms (pulse, wave), 3000 ms for win/lose.
- Team colours: Order `[0,90,255]`, Chaos `[255,40,40]`. Drake colours by `DragonType`: Fire `[255,60,0]`, Water/Ocean `[0,140,255]`, Earth/Mountain `[150,90,30]`, Air/Cloud `[220,240,255]`, Hextech `[0,230,230]`, Chemtech `[60,255,60]`, Elder `[170,60,255]`; unknown → `[255,255,255]`.
- Bars: fraction = health/max_health, resource/resource_max, gold/max, (level−1)/17, `respawn_fraction`; keys in list order (reversed when `reverse`); a key is lit with `empty→full` blended by how much of it the fraction covers; unlit keys show `empty`. Resource bar keys go to the background when `resource_kind == "NONE"`.
- Layers bottom-up: background → bars → dead state (replaces both while `snap.dead`: all keys `dim`, countdown bar over `countdown_keys`) → active effects (flash: whole board in the colour fading out; pulse: whole board brightness `sin` over the duration; wave: a band moving left to right by `Led.x`). Multikill plays `streak − 1` times back to back. A new effect replaces the one playing (finished ones are dropped from `active`).
- Output: colour groups (keys with identical colours together, black keys omitted).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn default_setup_is_the_specs_default()        // background Team, 2 bars (health number row, resource f-row), dead.on, events on/off as listed
#[test] fn health_bar_fills_its_keys_in_order()        // health 50% over 10 keys: first 5 full colour, last 5 empty; reverse flips it
#[test] fn team_background_uses_the_team_colour()      // Order -> [0,90,255] on keys outside the bars
#[test] fn dead_state_replaces_bars_with_a_countdown() // dead, fraction 0.5 on 10 countdown keys: 5 white, rest dim
#[test] fn champions_without_resource_leave_those_keys_to_the_background()
#[test] fn effects_play_for_their_duration_then_end()  // flash at t=0 colours every key; at t>ms it's gone and removed from active
#[test] fn dragon_effect_takes_the_drakes_colour()      // activate Dragon(dragon=Some("Fire")) with color None -> [255,60,0]
#[test] fn turned_off_events_do_nothing()               // activate(assist) with default setup -> None
```

- [ ] **Step 2:** Run `cargo test --lib gamelight::` → FAIL.
- [ ] **Step 3:** Implement the interfaces and values above.
- [ ] **Step 4:** Run `cargo test --lib gamelight::` → PASS.
- [ ] **Step 5:** Commit: `git add src-tauri/src/gamelight.rs src-tauri/src/anim.rs src-tauri/src/lib.rs && git commit -m "League lighting: customisable setup and renderer"`

### Task 4: Match lifecycle, watcher thread and handover

**Files:**
- Create: `src-tauri/src/match_watch.rs` (register `pub mod match_watch;` in `lib.rs`)
- Modify: `src-tauri/src/app.rs`, `src-tauri/src/lib.rs`
- Test: `match_watch.rs` tests; `app.rs` (or `lib.rs`) test for old settings

**Interfaces:**
- Consumes: `league::{fetch, parse, Parsed, Tracker}`, `gamelight::{Setup, activate, render, Active}`, `anim::fit`, `device::send_frame`, `audio::{leds, FPS}`.
- Produces:

```rust
// match_watch.rs (pure, all platforms)
pub enum Poll { Nothing, Playing }              // from fetch+parse: no endpoint/NotPlaying, or a live match
pub enum Change { Started, Ended }
pub struct Watch { /* state: Idle | Live{since_gone} | SteppedAside */ }
impl Watch {
    pub fn new() -> Self;
    pub fn poll(&mut self, p: Poll, now_ms: u64) -> Option<Change>;  // Ended after GONE_MS (3000) without Playing, or immediately on game_end()
    pub fn game_end(&mut self) -> Option<Change>;
    pub fn step_aside(&mut self);                 // stays aside until a Nothing->Playing transition after an Ended
    pub fn owns_keyboard(&self) -> bool;          // Live and not stepped aside
}
pub const SEARCH_MS: u64 = 1000; pub const POLL_MS: u64 = 300; pub const GONE_MS: u64 = 3000;

// Windows thread (cfg(windows)), like anim::run / audio::run
pub mod run {
    pub struct Handle { pub setup: Arc<Mutex<Setup>>, pub stop: Arc<AtomicBool>, pub step_aside: Arc<AtomicBool>, thread: ... }
    impl Handle { pub fn stop(self); }
    pub enum Event { Started, Ended, Frame(Vec<(u8, Rgb)>), Status(String), Error(String) }
    pub fn start(kb, open: fn() -> Result<Keyboard>, setup: Setup, on: impl Fn(Event) + Send + 'static) -> Handle;
}
```

`run` loop: poll every `SEARCH_MS` while idle and `POLL_MS` while live; feed `Watch`; when the `Tracker` reports `Win` or `Lose`, also call `watch.game_end()`; on `Started` emit `Event::Started` and wait for the app to answer by leaving `step_aside` false (the app decides takeover, below); while `owns_keyboard()`, stream frames at `audio::FPS` (render → `anim::fit` → `send_frame`), errors handled like `anim::run` (report the first and every 30th, wait 500 ms); on `Ended` play the win/lose effect to its end, then emit `Event::Ended`.

App side (`app.rs`), as the spec's handover:
- `AppConfig` gains `league_on: bool` (default false), `league_setup: gamelight::Setup`, `league_presets: Vec<LeaguePreset { name, setup }>`; `AppState` gains `league: Mutex<Option<match_watch::run::Handle>>` and `league_resume: Mutex<Option<Resume>>` where `enum Resume { Animation, Audio(audio::Settings) }`.
- On `Started`: open the keyboard (through `state.kb` / `open_keyboard`); if it isn't found or `kb.mode == Mode::Bluetooth`, call `step_aside` and emit `league-status` with "League lighting needs the USB cable or the 2.4G dongle." Otherwise remember what streams (`anim` running → `Resume::Animation` via `stop_animation(app, false)`; audio running → `Resume::Audio(settings)`, stop it and emit `audio-stopped`), then let the watcher stream.
- On `Ended`: take `league_resume`; `Animation` → `start_animation`; `Audio(s)` → start audio with `s` (factor the body of `lib.rs` `audio_start` into a function both use); `None` → nothing (the keyboard returns to its own effect).
- `start_animation` and audio start, when called while League owns the keyboard: set the handle's `step_aside`, and clear `league_resume` (your choice wins; nothing is restored).
- `setup()`: start the watcher when `league_on`. `quit()`: stop it.
- Commands (register in `lib.rs`): `league_get() -> LeagueState { on, setup, live: bool, status: Option<String> }`, `league_set(on: Option<bool>, setup: Option<Setup>) -> LeagueState` (saves; starts/stops the watcher; updates a running setup), `league_preset_save(name, setup) -> Presets`; `presets_get`/`Presets` gain `league`; `preset_delete` accepts kind `"league"`. Events to the UI: `league-frame` (keys), `league-state` (`LeagueState`), `league-error`.

- [ ] **Step 1: Write the failing tests** (`match_watch.rs`):

```rust
#[test] fn playing_starts_a_match_once()                     // Nothing, Playing -> Started; Playing again -> None
#[test] fn endpoint_gone_for_three_seconds_ends_the_match()  // Playing at 0, Nothing at 1000/2000 -> None, Nothing at 3000 -> Ended
#[test] fn game_end_ends_at_once()
#[test] fn stepping_aside_lasts_until_the_next_match()       // Started, step_aside -> owns_keyboard false; still false while Playing; Ended; next Started -> owns_keyboard true
#[test] fn old_settings_without_league_still_load()          // app.rs: serde_json::from_str::<AppConfig>("{}") -> league_on false, league_setup == Setup::default()
```

- [ ] **Step 2:** Run `cargo test --lib match_watch:: old_settings` → FAIL.
- [ ] **Step 3:** Implement `Watch`, then `run`, then the app side and commands as above.
- [ ] **Step 4:** Run `cargo test` in `src-tauri` → all PASS, `cargo build` without warnings.
- [ ] **Step 5:** Commit: `git add src-tauri/src && git commit -m "League lighting: match watcher, takeover and hand-back"`

### Task 5: Games page

**Files:**
- Create: `src/pages/games.tsx`
- Modify: `src/lib/api.ts`, `src/App.tsx`

**Interfaces:**
- Consumes: the Task 4 commands and events; `Keyboard`, `PresetBar`, `ColourInput` components.
- Produces: TypeScript mirrors of `Setup` and its parts (`Value`, `Style`, `GameEventKind`, `Background`, `Bar`, `DeadState`, `EventEffect`), `LeagueState`, and `api.leagueGet()`, `api.leagueSet({ on?, setup? })`, `api.leaguePresetSave(name, setup)`; `Presets.league`; `presetDelete` kind `"league"`.

Page contents (plain short sentences, existing card style):
- A "League of Legends lighting" switch, and one status line: "Waiting for a match.", "Lighting your match.", or the `status` text (for example the Bluetooth message).
- Live keyboard preview from `league-frame`, like the Animations page.
- Background: off / solid colour / team colour.
- Bars: add, remove; per bar the value, keys ("Number row", "F1-F12", or "Pick keys" painting on the keyboard picture, in click order), full and empty colours, direction, and a maximum for gold.
- Dead state: on/off, dim colour, countdown keys and colour.
- Events: one row per event with on/off, style, colour (Dragon: "Drake's colour" or a picked one), duration.
- `PresetBar` for League presets (saves "this League setup: background, bars, dead state and effects").
- Changes save and apply at once (`leagueSet({ setup })`), debounced like the Animations page (120 ms).
- Add `{ id: "games", label: "Games", icon: Gamepad2 }` to `PAGES` in `App.tsx` (not wireless-only).

- [ ] **Step 1:** Add the types and api functions; run `npx tsc --noEmit` → PASS.
- [ ] **Step 2:** Build `games.tsx` and the nav entry; run `npx tsc --noEmit` → PASS.
- [ ] **Step 3:** Commit: `git add src && git commit -m "League lighting: Games page"`

### Task 6: Docs and a real match

**Files:**
- Modify: `README.md` (features: League lighting, cable/dongle only, official data only), `CLAUDE.md` (status: League lighting, not yet verified in a match; where the code lives)

- [ ] **Step 1:** Update both files; commit `git commit -am "League lighting: docs"` (stage only these two files).
- [ ] **Step 2:** Owner check in a real match (practice tool is fine), on the cable and on the dongle: the switch on; an animation running before the match; the match takes over; health and mana bars move; dying shows the countdown; a kill and a dragon flash; leaving the game brings the animation back; on Bluetooth the status line says why it doesn't run. Record what was verified in `CLAUDE.md` and commit after the owner confirms.
