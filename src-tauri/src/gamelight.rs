//! League lighting: what the keyboard shows during a match (the customisable setup) and how
//! a game snapshot plus the playing effects turn into a frame.

use crate::audio::Led;
use crate::league::{GameEvent, Snapshot, Team};
use crate::protocol::Rgb;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// The moments an effect can play for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Event {
    Kill,
    Death,
    Assist,
    FirstBlood,
    Multikill,
    Ace,
    Dragon,
    Herald,
    Baron,
    Turret,
    Inhibitor,
    LevelUp,
    Win,
    Lose,
}

/// What a bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Value {
    Health,
    /// Mana, energy, fury, ...
    Resource,
    Gold,
    /// 1-18
    Level,
    /// Respawn countdown (only while dead).
    Respawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// The whole board in the colour, fading out.
    Flash,
    /// The whole board swells to the colour and back.
    Pulse,
    /// A band of the colour sweeps left to right.
    Wave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundKind {
    Off,
    Solid,
    /// Blue side or red side.
    Team,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Background {
    pub kind: BackgroundKind,
    /// For Solid.
    pub color: Rgb,
}

impl Default for Background {
    fn default() -> Self {
        Background { kind: BackgroundKind::Team, color: [255, 255, 255] }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bar {
    pub value: Value,
    /// Matrix indexes, filled in this order.
    pub keys: Vec<u8>,
    /// Black for a resource bar = the resource's own colour.
    pub full: Rgb,
    pub empty: Rgb,
    pub reverse: bool,
    /// The full amount, for gold.
    pub max: u32,
}

impl Default for Bar {
    fn default() -> Self {
        Bar { value: Value::Health, keys: Vec::new(), full: [0, 255, 0], empty: [255, 0, 0], reverse: false, max: 3000 }
    }
}

/// While you're dead: everything dim, with the respawn countdown as a bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeadState {
    pub on: bool,
    pub dim: Rgb,
    pub countdown_keys: Vec<u8>,
    pub countdown: Rgb,
}

impl Default for DeadState {
    fn default() -> Self {
        DeadState { on: true, dim: [40, 40, 40], countdown_keys: keys(NUMBER_ROW), countdown: [255, 255, 255] }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventEffect {
    pub event: Event,
    pub on: bool,
    pub style: Style,
    /// None for a dragon: the drake's own colour.
    pub color: Option<Rgb>,
    pub ms: u32,
}

impl Default for EventEffect {
    fn default() -> Self {
        EventEffect { event: Event::Kill, on: true, style: Style::Flash, color: Some([255, 255, 255]), ms: 600 }
    }
}

/// Everything the keyboard shows during a match: one preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Setup {
    pub background: Background,
    pub bars: Vec<Bar>,
    pub dead: DeadState,
    pub events: Vec<EventEffect>,
}

const NUMBER_ROW: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
const F_ROW: [&str; 12] = ["f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"];

/// Matrix indexes of keys by name, in that order.
fn keys(names: impl IntoIterator<Item = &'static str>) -> Vec<u8> {
    names.into_iter().filter_map(|n| crate::keymap::KEYS.iter().find(|k| k.name == n).map(|k| k.index)).collect()
}

impl Default for Setup {
    fn default() -> Self {
        use Event::*;
        let fx = |event, on, style, color: Option<Rgb>| {
            let ms = match (event, style) {
                (Win | Lose, _) => 3000,
                (_, Style::Flash) => 600,
                _ => 1200,
            };
            EventEffect { event, on, style, color, ms }
        };
        Setup {
            background: Background::default(),
            bars: vec![
                Bar { value: Value::Health, keys: keys(NUMBER_ROW), ..Bar::default() },
                Bar { value: Value::Resource, keys: keys(F_ROW), full: [0, 0, 0], empty: [0, 0, 0], ..Bar::default() },
            ],
            dead: DeadState::default(),
            events: vec![
                fx(Kill, true, Style::Flash, Some([255, 215, 0])),
                fx(Death, true, Style::Pulse, Some([255, 0, 0])),
                fx(Assist, false, Style::Flash, Some([0, 255, 128])),
                fx(FirstBlood, false, Style::Flash, Some([200, 0, 0])),
                fx(Multikill, true, Style::Flash, Some([255, 100, 0])),
                fx(Ace, true, Style::Wave, Some([255, 255, 255])),
                fx(Dragon, true, Style::Wave, None),
                fx(Herald, true, Style::Wave, Some([160, 0, 255])),
                fx(Baron, true, Style::Wave, Some([160, 0, 255])),
                fx(Turret, true, Style::Flash, Some([255, 255, 255])),
                fx(Inhibitor, true, Style::Flash, Some([255, 255, 255])),
                fx(LevelUp, false, Style::Flash, Some([0, 255, 255])),
                fx(Win, true, Style::Wave, Some([255, 215, 0])),
                fx(Lose, true, Style::Pulse, Some([255, 0, 0])),
            ],
        }
    }
}

fn team_color(t: Team) -> Rgb {
    match t {
        Team::Order => [0, 90, 255],
        Team::Chaos => [255, 40, 40],
    }
}

/// The drake's colour, by the API's `DragonType`.
fn dragon_color(kind: &str) -> Rgb {
    match kind.to_ascii_lowercase().as_str() {
        "fire" => [255, 60, 0],
        "water" => [0, 140, 255],
        "earth" => [150, 90, 30],
        "air" => [220, 240, 255],
        "hextech" => [0, 230, 230],
        "chemtech" => [60, 255, 60],
        "elder" => [170, 60, 255],
        _ => [255, 255, 255],
    }
}

fn resource_color(kind: &str) -> Rgb {
    match kind {
        "MANA" => [0, 120, 255],
        "ENERGY" => [255, 220, 0],
        _ => [255, 255, 255],
    }
}

fn mix(a: Rgb, b: Rgb, f: f32) -> Rgb {
    let f = f.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * f).round() as u8)
}

fn ratio(a: f32, b: f32) -> f32 {
    if b > 0.0 {
        (a / b).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// An effect that's playing.
#[derive(Debug, Clone)]
pub struct Active {
    pub effect: EventEffect,
    pub color: Rgb,
    /// Seconds, on the same clock as `render`'s `now`.
    pub start: f32,
    pub repeats: u8,
}

impl Active {
    /// Seconds it plays for, repeats included.
    pub fn ms_total(&self) -> f32 {
        self.effect.ms as f32 / 1000.0 * self.repeats as f32
    }
    fn end(&self) -> f32 {
        self.start + self.ms_total()
    }
}

/// The effect a moment plays, or None when that event is turned off.
pub fn activate(setup: &Setup, ev: &GameEvent, now: f32) -> Option<Active> {
    let effect = setup.events.iter().find(|e| e.event == ev.event && e.on)?.clone();
    let color = effect.color.unwrap_or_else(|| ev.dragon.as_deref().map_or([255, 255, 255], dragon_color));
    // a multi-kill plays once more for each kill past the first (penta: 4 times)
    let repeats = if ev.event == Event::Multikill { ev.streak.saturating_sub(1).max(1) } else { 1 };
    Some(Active { effect, color, start: now, repeats })
}

/// Paint `bar` over `colors` with `fraction` of its keys lit, the boundary key blended.
fn paint_bar(colors: &mut HashMap<u8, Rgb>, keys: &[u8], reverse: bool, fraction: f32, full: Rgb, empty: Rgb) {
    let n = keys.len() as f32;
    let ordered: Vec<u8> = if reverse { keys.iter().rev().copied().collect() } else { keys.to_vec() };
    for (i, k) in ordered.into_iter().enumerate() {
        if colors.contains_key(&k) {
            colors.insert(k, mix(empty, full, fraction * n - i as f32));
        }
    }
}

/// The frame for `now` (seconds): background, bars, dead state, then the playing effect on
/// top. Finished effects are dropped from `active`; a newer effect replaces an older one.
pub fn render(
    setup: &Setup,
    snap: &Snapshot,
    respawn_fraction: f32,
    active: &mut Vec<Active>,
    leds: &[Led],
    now: f32,
) -> Vec<(Rgb, Vec<u8>)> {
    let base = match setup.background.kind {
        BackgroundKind::Off => [0, 0, 0],
        BackgroundKind::Solid => setup.background.color,
        BackgroundKind::Team => team_color(snap.team),
    };
    let mut colors: HashMap<u8, Rgb> = leds.iter().map(|l| (l.index, base)).collect();
    if snap.dead && setup.dead.on {
        colors.values_mut().for_each(|c| *c = setup.dead.dim);
        let d = &setup.dead;
        paint_bar(&mut colors, &d.countdown_keys, false, respawn_fraction, d.countdown, d.dim);
    } else {
        for bar in &setup.bars {
            let (fraction, full) = match bar.value {
                Value::Health => (ratio(snap.health, snap.max_health), bar.full),
                Value::Resource if snap.resource_kind == "NONE" => continue,
                Value::Resource => (
                    ratio(snap.resource, snap.resource_max),
                    if bar.full == [0, 0, 0] { resource_color(&snap.resource_kind) } else { bar.full },
                ),
                Value::Gold => (ratio(snap.gold, bar.max as f32), bar.full),
                Value::Level => (ratio(snap.level.saturating_sub(1) as f32, 17.0), bar.full),
                Value::Respawn => (respawn_fraction, bar.full),
            };
            paint_bar(&mut colors, &bar.keys, bar.reverse, fraction, full, bar.empty);
        }
    }

    active.retain(|a| now < a.end());
    if active.len() > 1 {
        active.drain(..active.len() - 1);
    }
    if let Some(a) = active.last() {
        let len = a.effect.ms.max(1) as f32 / 1000.0;
        let p = ((now - a.start) / len).fract().clamp(0.0, 1.0);
        for l in leds {
            let strength = match a.effect.style {
                Style::Flash => 1.0 - p,
                Style::Pulse => (std::f32::consts::PI * p).sin(),
                Style::Wave => (1.0 - (l.x - (p * 1.4 - 0.2)).abs() / 0.2).max(0.0),
            };
            if let Some(c) = colors.get_mut(&l.index) {
                *c = mix(*c, a.color, strength);
            }
        }
    }

    let mut groups: BTreeMap<Rgb, Vec<u8>> = BTreeMap::new();
    for (k, c) in colors {
        if c != [0, 0, 0] {
            groups.entry(c).or_default().push(k);
        }
    }
    groups.into_iter().map(|(c, mut ks)| { ks.sort_unstable(); (c, ks) }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::leds;

    const NUMBER_ROW: [u8; 10] = [7, 13, 19, 25, 31, 37, 43, 49, 55, 61];
    const ESC: u8 = 0;
    const F1: u8 = 12;

    fn snap() -> Snapshot {
        Snapshot {
            team: Team::Order,
            health: 600.0,
            max_health: 1200.0,
            resource: 150.0,
            resource_max: 300.0,
            resource_kind: "MANA".into(),
            level: 6,
            gold: 1000.0,
            dead: false,
            respawn: 0.0,
        }
    }

    fn colour(groups: &[(Rgb, Vec<u8>)], key: u8) -> Rgb {
        groups.iter().find(|(_, k)| k.contains(&key)).map_or([0, 0, 0], |(c, _)| *c)
    }

    fn frame(setup: &Setup, s: &Snapshot, fraction: f32) -> Vec<(Rgb, Vec<u8>)> {
        render(setup, s, fraction, &mut Vec::new(), &leds(), 0.0)
    }

    fn health_only() -> Setup {
        Setup {
            background: Background { kind: BackgroundKind::Off, color: [0, 0, 0] },
            bars: vec![Bar { value: Value::Health, keys: NUMBER_ROW.to_vec(), full: [0, 255, 0], empty: [255, 0, 0], reverse: false, max: 0 }],
            dead: DeadState { on: false, ..Setup::default().dead },
            events: Vec::new(),
        }
    }

    fn game_event(event: Event) -> GameEvent {
        GameEvent { event, streak: 0, dragon: None }
    }

    #[test]
    fn default_setup_is_the_specs_default() {
        let s = Setup::default();
        assert_eq!(s.background.kind, BackgroundKind::Team);
        assert_eq!(s.bars.len(), 2);
        assert_eq!((s.bars[0].value, s.bars[0].keys.clone()), (Value::Health, NUMBER_ROW.to_vec()));
        assert_eq!((s.bars[0].full, s.bars[0].empty), ([0, 255, 0], [255, 0, 0]));
        assert_eq!((s.bars[1].value, s.bars[1].keys[0], s.bars[1].keys.len()), (Value::Resource, F1, 12));
        assert!(s.dead.on);
        let on: Vec<Event> = s.events.iter().filter(|e| e.on).map(|e| e.event).collect();
        use Event::*;
        assert_eq!(on, [Kill, Death, Multikill, Ace, Dragon, Herald, Baron, Turret, Inhibitor, Win, Lose]);
        assert_eq!(s.events.len(), 14);
    }

    #[test]
    fn health_bar_fills_its_keys_in_order() {
        let mut setup = health_only();
        let f = frame(&setup, &snap(), 0.0); // half health over 10 keys
        assert_eq!(NUMBER_ROW.map(|k| colour(&f, k))[..5], [[0, 255, 0]; 5]);
        assert_eq!(NUMBER_ROW.map(|k| colour(&f, k))[5..], [[255, 0, 0]; 5]);
        setup.bars[0].reverse = true;
        let f = frame(&setup, &snap(), 0.0);
        assert_eq!(colour(&f, NUMBER_ROW[0]), [255, 0, 0]);
        assert_eq!(colour(&f, NUMBER_ROW[9]), [0, 255, 0]);
    }

    #[test]
    fn team_background_uses_the_team_colour() {
        let f = frame(&Setup::default(), &snap(), 0.0);
        assert_eq!(colour(&f, ESC), [0, 90, 255]);
        let f = frame(&Setup::default(), &Snapshot { team: Team::Chaos, ..snap() }, 0.0);
        assert_eq!(colour(&f, ESC), [255, 40, 40]);
    }

    #[test]
    fn dead_state_replaces_bars_with_a_countdown() {
        let s = Snapshot { dead: true, respawn: 10.0, ..snap() };
        let f = frame(&Setup::default(), &s, 0.5);
        assert_eq!(NUMBER_ROW.map(|k| colour(&f, k))[..5], [[255, 255, 255]; 5]);
        assert_eq!(NUMBER_ROW.map(|k| colour(&f, k))[5..], [[40, 40, 40]; 5]);
        assert_eq!(colour(&f, ESC), [40, 40, 40]);
        assert_eq!(colour(&f, F1), [40, 40, 40]);
    }

    #[test]
    fn champions_without_resource_leave_those_keys_to_the_background() {
        let s = Snapshot { resource_kind: "NONE".into(), resource: 0.0, resource_max: 0.0, ..snap() };
        assert_eq!(colour(&frame(&Setup::default(), &s, 0.0), F1), [0, 90, 255]);
    }

    #[test]
    fn effects_play_for_their_duration_then_end() {
        let setup = Setup::default();
        let kill = activate(&setup, &game_event(Event::Kill), 0.0).unwrap();
        let color = kill.color;
        let mut active = vec![kill];
        let f = render(&setup, &snap(), 0.0, &mut active, &leds(), 0.0);
        assert_eq!(colour(&f, ESC), color);
        let f = render(&setup, &snap(), 0.0, &mut active, &leds(), 0.7);
        assert_eq!(colour(&f, ESC), [0, 90, 255]);
        assert!(active.is_empty());
    }

    #[test]
    fn dragon_effect_takes_the_drakes_colour() {
        let e = GameEvent { event: Event::Dragon, streak: 0, dragon: Some("Fire".into()) };
        assert_eq!(activate(&Setup::default(), &e, 0.0).unwrap().color, [255, 60, 0]);
    }

    #[test]
    fn turned_off_events_do_nothing() {
        assert!(activate(&Setup::default(), &game_event(Event::Assist), 0.0).is_none());
    }
}
