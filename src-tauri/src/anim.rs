//! Animations in your own colours, streamed from the computer (real-time frames, CMD_LIVE).
//!
//! The F75's firmware effects can't do this: their "multicolour" is a fixed rainbow that
//! ignores the effect palettes (tested 2026-10-21). So the app draws the frames itself, like
//! the audio lighting. The keyboard shows them while the app runs, and about 2 s after the
//! last frame goes back to its own effect.
//!
//! A frame only fits ~11 colours (keys share colour groups), so keys sit in fixed bands along
//! the animation's direction and never change band, which keeps the movement smooth (the
//! approach aula-f75-hub found on hardware).

use crate::audio::{encode, leds, Led};
use crate::protocol::Rgb;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Your colours flow across the board.
    Stream,
    /// The board fades in and out, a new colour each breath.
    Breathing,
    /// Your colours stay in place while a bright crest sweeps over them.
    Wave,
    /// Keys twinkle in your colours over a dim glow.
    Sparkle,
    /// Your colours spread across the board, standing still.
    Gradient,
    /// A GIF or picture played on the keys (see `imagelight`).
    Image,
    /// Flames rising from one edge of the board, in your colours: the first colour licks
    /// at the tips, the last burns at the base, and the hottest core glows almost white.
    Fire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Right,
    Left,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub kind: Kind,
    /// 1-7 colours.
    pub colors: Vec<Rgb>,
    /// 0.1 - 3.0
    pub speed: f32,
    /// 0.05 - 1.0
    pub brightness: f32,
    pub direction: Direction,
    /// For Image: the picture (the app's own copy of it).
    pub image: Option<String>,
    /// For Image: how the picture is laid over the board.
    pub fit: crate::imagelight::Fit,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            kind: Kind::Stream,
            colors: vec![[255, 20, 147], [255, 0, 0], [255, 110, 0]], // pink, red, orange
            speed: 1.0,
            brightness: 1.0,
            direction: Direction::Right,
            image: None,
            fit: crate::imagelight::Fit::Fill,
        }
    }
}

/// Bands along the direction: each one is a colour group, and a frame fits about 11.
const BANDS: usize = 11;

fn mix(a: Rgb, b: Rgb, f: f32) -> Rgb {
    let f = f.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * f).round() as u8)
}

fn scale(c: Rgb, v: f32) -> Rgb {
    let v = v.clamp(0.0, 1.0);
    c.map(|x| (x as f32 * v).round() as u8)
}

/// Colour at position `u` (wraps every 1.0) of the colour list, blended between neighbours.
fn color_at(colors: &[Rgb], u: f32) -> Rgb {
    match colors.len() {
        0 => [0, 0, 0],
        1 => colors[0],
        n => {
            let x = u.rem_euclid(1.0) * n as f32;
            let i = x as usize % n;
            mix(colors[i], colors[(i + 1) % n], x.fract())
        }
    }
}

/// 0..1 position of a key along a direction (where a flow starts = 0).
fn along(l: &Led, d: Direction) -> f32 {
    match d {
        Direction::Right => l.x,
        Direction::Left => 1.0 - l.x,
        Direction::Down => l.y,
        Direction::Up => 1.0 - l.y,
    }
}

fn band_of(l: &Led, d: Direction) -> usize {
    ((along(l, d) * BANDS as f32) as usize).min(BANDS - 1)
}

/// Deterministic per-key randomness (twinkle timing), the same on every run.
fn seed(index: u8, salt: u32) -> f32 {
    let mut x = (index as u32).wrapping_mul(2_654_435_761) ^ salt.wrapping_mul(40_503);
    x ^= x >> 15;
    x = x.wrapping_mul(2_246_822_519);
    x ^= x >> 13;
    (x % 10_000) as f32 / 10_000.0
}

/// Smooth value noise in 0..1: random values on a grid, blended between grid points.
fn noise(x: f32, y: f32) -> f32 {
    let hash = |ix: i32, iy: i32| -> f32 {
        let mut h = (ix as u32).wrapping_mul(374_761_393) ^ (iy as u32).wrapping_mul(668_265_263);
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        ((h ^ (h >> 16)) % 10_000) as f32 / 10_000.0
    };
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let smooth = |f: f32| f * f * (3.0 - 2.0 * f);
    let (sx, sy) = (smooth(fx), smooth(fy));
    let top = hash(ix, iy) + (hash(ix + 1, iy) - hash(ix, iy)) * sx;
    let bottom = hash(ix, iy + 1) + (hash(ix + 1, iy + 1) - hash(ix, iy + 1)) * sx;
    top + (bottom - top) * sy
}

/// Two octaves of noise: big slow shapes with finer flicker on top.
fn fbm(x: f32, y: f32) -> f32 {
    (noise(x, y) * 2.0 + noise(x * 2.3 + 17.0, y * 2.3 - 5.0)) / 3.0
}

/// Heat levels a fire frame is drawn with: one colour group each, so even 7 colours fit.
const FIRE_LEVELS: usize = 8;

/// Fire: how hot (0..1) a key is at time `t`. `base` is the key's distance from the edge the
/// flames rise from (0 = the edge), `across` its position along that edge.
fn fire_heat(base: f32, across: f32, t: f32) -> f32 {
    // each tongue of flame reaches its own, wandering height
    let reach = 0.25 + 0.85 * fbm(across * 5.0, t * 0.9);
    let body = (1.0 - base / reach).clamp(0.0, 1.0);
    // turbulence moving up the flames, stronger towards the tips
    let swirl = fbm(across * 7.0 + 3.0, base * 5.0 - t * 3.2) - 0.5;
    // the whole fire breathes a little
    let breath = 0.92 + 0.08 * (t * 2.3).sin() * (t * 0.7 + across).cos();
    ((body + swirl * (0.35 + 0.5 * base)) * breath).clamp(0.0, 1.0)
}

/// The fire's colour at heat `h`: dark, then your colours from the tips (first) to the base
/// (last), then a near-white core.
fn fire_color(colors: &[Rgb], h: f32) -> Rgb {
    let mut stops: Vec<Rgb> = vec![[0, 0, 0]];
    stops.extend_from_slice(colors);
    let last = *colors.last().unwrap_or(&[255, 120, 0]);
    stops.push(mix(last, [255, 245, 220], 0.75));
    let x = h.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (x as usize).min(stops.len() - 2);
    mix(stops[i], stops[i + 1], x - i as f32)
}

/// The frame at time `t` (seconds) as colour groups.
pub fn render(leds: &[Led], s: &Settings, t: f32) -> Vec<(Rgb, Vec<u8>)> {
    let colors: Vec<Rgb> = if s.colors.is_empty() { vec![[255, 255, 255]] } else { s.colors.clone() };
    let t = t * s.speed.clamp(0.1, 3.0);
    let bright = s.brightness.clamp(0.05, 1.0);
    let mut bands: BTreeMap<usize, (Rgb, Vec<u8>)> = BTreeMap::new();
    let band_color = |b: usize| -> f32 { (b as f32 + 0.5) / BANDS as f32 };
    match s.kind {
        Kind::Stream | Kind::Gradient => {
            let shift = if s.kind == Kind::Stream { t * 0.12 } else { 0.0 };
            for l in leds {
                let b = band_of(l, s.direction);
                let c = color_at(&colors, band_color(b) - shift);
                bands.entry(b).or_insert((scale(c, bright), Vec::new())).1.push(l.index);
            }
        }
        Kind::Wave => {
            for l in leds {
                let b = band_of(l, s.direction);
                let pos = band_color(b);
                let phase = pos - t * 0.35;
                let crest = 0.5 + 0.5 * (2.0 * std::f32::consts::PI * phase).cos();
                let c = mix(scale(color_at(&colors, pos), 0.25 + 0.75 * crest), [255, 255, 255], 0.25 * crest.powi(8));
                bands.entry(b).or_insert((scale(c, bright), Vec::new())).1.push(l.index);
            }
        }
        Kind::Breathing => {
            // one breath every 4 s at speed 1: fade in, fade out, next colour
            let breath = t / 4.0;
            let i = breath.floor() as usize % colors.len();
            let phase = breath.fract();
            let seen = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * phase).cos();
            let v = seen.powf(2.2); // LED power: the eye is far more sensitive when dim
            let c = scale(colors[i], v * bright);
            bands.insert(0, (c, leds.iter().map(|l| l.index).collect()));
        }
        Kind::Image => {} // drawn by render_image, from the decoded picture
        Kind::Fire => {
            let t = t * 0.8;
            for l in leds {
                let base = along(l, s.direction);
                let across = match s.direction {
                    Direction::Up | Direction::Down => l.x * 2.6, // the board is wide: more tongues
                    Direction::Left | Direction::Right => l.y,
                };
                let heat = fire_heat(base, across, t);
                // a few fixed heat levels, so the frame keeps its exact colours
                let level = (heat * FIRE_LEVELS as f32).round() as usize;
                if level == 0 {
                    continue;
                }
                let c = fire_color(&colors, level as f32 / FIRE_LEVELS as f32);
                bands.entry(level).or_insert((scale(c, bright), Vec::new())).1.push(l.index);
            }
        }
        Kind::Sparkle => {
            let base = scale(color_at(&colors, 0.0), 0.12 * bright);
            let mut twinkles: BTreeMap<usize, (Rgb, Vec<u8>)> = BTreeMap::new();
            let mut dim = Vec::new();
            for l in leds {
                let period = 2.0 + 3.0 * seed(l.index, 1);
                let age = (t + seed(l.index, 2) * period).rem_euclid(period);
                let cycle = ((t + seed(l.index, 2) * period) / period).floor() as u32;
                if age < 0.3 {
                    // flash in one of your colours, a different one each time (one group per
                    // colour, so even 7 colours fit the frame)
                    let ci = (seed(l.index, cycle.wrapping_add(7)) * colors.len() as f32) as usize % colors.len();
                    twinkles.entry(ci).or_insert((scale(colors[ci], bright), Vec::new())).1.push(l.index);
                } else {
                    dim.push(l.index);
                }
            }
            bands.insert(0, (base, dim));
            for (k, v) in twinkles {
                bands.insert(100 + k, v);
            }
        }
    }
    // keys with the same colour can share a group
    let mut merged: BTreeMap<Rgb, Vec<u8>> = BTreeMap::new();
    for (c, keys) in bands.into_values() {
        merged.entry(c).or_default().extend(keys);
    }
    merged.into_iter().filter(|(c, _)| *c != [0, 0, 0]).collect()
}

/// An image animation's frame at time `t`: every key its own colour, so on 2.4G and
/// Bluetooth the frame is merged down to fit (see `fit`); over the cable it shows as it is.
pub fn render_image(clip: &crate::imagelight::Clip, s: &Settings, t: f32) -> Vec<(Rgb, Vec<u8>)> {
    let bright = s.brightness.clamp(0.05, 1.0);
    let mut groups: BTreeMap<Rgb, Vec<u8>> = BTreeMap::new();
    for &(index, c) in clip.at(t * s.speed.clamp(0.1, 3.0)) {
        groups.entry(scale(c, bright)).or_default().push(index);
    }
    groups.into_iter().filter(|(c, _)| *c != [0, 0, 0]).collect()
}

fn distance(a: Rgb, b: Rgb) -> u32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).unsigned_abs()).max().unwrap()
}

/// Merge groups until the frame fits the budget: each time the pair that changes least,
/// weighing the colour difference by how many keys change (so a few odd keys merge before
/// big areas do). The merged group takes the keys' average colour.
pub(crate) fn fit(mut groups: Vec<(Rgb, Vec<u8>)>) -> Vec<(Rgb, Vec<u8>)> {
    let bytes = |g: &[(Rgb, Vec<u8>)]| g.iter().map(|(_, k)| 4 + k.len()).sum::<usize>();
    while groups.len() > 1 && bytes(&groups) > crate::protocol::LIVE_BUDGET {
        let mut best = (0, 1, f32::MAX);
        for i in 0..groups.len() {
            for j in i + 1..groups.len() {
                let (a, b) = (groups[i].1.len() as f32, groups[j].1.len() as f32);
                let cost = distance(groups[i].0, groups[j].0) as f32 * a * b / (a + b).max(1.0);
                if cost < best.2 {
                    best = (i, j, cost);
                }
            }
        }
        let (c2, keys) = groups.remove(best.1);
        let (c1, k1) = &mut groups[best.0];
        let (n1, n2) = (k1.len() as u32, keys.len() as u32);
        *c1 = [0, 1, 2].map(|i| ((c1[i] as u32 * n1 + c2[i] as u32 * n2) / (n1 + n2).max(1)) as u8);
        k1.extend(keys);
    }
    groups
}

/// The frame at time `t`, ready to stream.
pub fn frame(leds: &[Led], s: &Settings, t: f32) -> crate::protocol::Result<Vec<u8>> {
    encode(&fit(render(leds, s, t)))
}

/// What the UI gets every frame: the key colours shown, or that it's paused for idleness.
#[derive(Debug, Clone, Serialize)]
pub struct FrameEvent {
    pub keys: Vec<(u8, Rgb)>,
    pub paused: bool,
}

#[cfg(windows)]
pub mod run {
    use super::*;
    use crate::device::Keyboard;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    pub struct Handle {
        pub stop: Arc<AtomicBool>,
        pub settings: Arc<Mutex<Settings>>,
        /// Pause after this many seconds without keyboard or mouse input; 0 = never.
        pub idle_pause: Arc<AtomicU32>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Handle {
        pub fn stop(mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    pub enum Event {
        Frame(FrameEvent),
        Error(String),
    }

    /// Seconds since the last keyboard or mouse input on this computer.
    pub fn idle_seconds() -> u32 {
        use windows_sys::Win32::System::SystemInformation::GetTickCount;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return 0;
        }
        unsafe { GetTickCount() }.wrapping_sub(info.dwTime) / 1000
    }

    pub fn start(
        kb: Arc<Mutex<Option<Keyboard>>>,
        open: fn() -> crate::device::Result<Keyboard>,
        settings: Settings,
        idle_pause: u32,
        emit: impl Fn(Event) + Send + 'static,
    ) -> Handle {
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(settings));
        let idle = Arc::new(AtomicU32::new(idle_pause));
        let (stop2, shared2, idle2) = (stop.clone(), shared.clone(), idle.clone());
        let thread = std::thread::spawn(move || {
            let leds = leds();
            let started = Instant::now();
            let period = Duration::from_secs_f32(1.0 / crate::audio::FPS as f32);
            let mut failures = 0u32;
            let mut was_paused = false;
            // the decoded picture, for the image and fit it was loaded with
            let mut clip: Option<(Option<String>, crate::imagelight::Fit, Option<crate::imagelight::Clip>)> = None;
            while !stop2.load(Ordering::Relaxed) {
                let tick = Instant::now();
                let limit = idle2.load(Ordering::Relaxed);
                // Paused while the computer is idle: the keyboard goes back to its own effect
                // and can sleep. The next key press or mouse move brings it back.
                if limit > 0 && idle_seconds() >= limit {
                    if !was_paused {
                        emit(Event::Frame(FrameEvent { keys: vec![], paused: true }));
                        was_paused = true;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
                was_paused = false;
                let s = shared2.lock().unwrap().clone();
                let t = started.elapsed().as_secs_f32();
                let groups = if s.kind == Kind::Image {
                    if clip.as_ref().is_none_or(|(image, fit, _)| *image != s.image || *fit != s.fit) {
                        let loaded = match &s.image {
                            None => Err("Choose a GIF or picture first.".to_string()),
                            Some(path) => crate::imagelight::load(std::path::Path::new(path), s.fit),
                        };
                        if let Err(e) = &loaded {
                            emit(Event::Error(e.clone()));
                        }
                        clip = Some((s.image.clone(), s.fit, loaded.ok()));
                    }
                    clip.as_ref().and_then(|c| c.2.as_ref()).map_or_else(Vec::new, |c| render_image(c, &s, t))
                } else {
                    render(&leds, &s, t)
                };
                // what the keyboard got (merged to fit over 2.4G and Bluetooth), for the preview
                let groups = match crate::device::send_frame(&kb, open, &groups) {
                    Ok(sent) => {
                        failures = 0;
                        sent
                    }
                    Err(e) => {
                        failures += 1;
                        if failures == 1 || failures % 30 == 0 {
                            emit(Event::Error(e));
                        }
                        std::thread::sleep(Duration::from_millis(500));
                        groups
                    }
                };
                let keys = groups.iter().flat_map(|(c, ks)| ks.iter().map(move |&i| (i, *c))).collect();
                emit(Event::Frame(FrameEvent { keys, paused: false }));
                if let Some(rest) = period.checked_sub(tick.elapsed()) {
                    std::thread::sleep(rest);
                }
            }
        });
        Handle { stop, settings: shared, idle_pause: idle, thread: Some(thread) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds() -> [Kind; 6] {
        [Kind::Stream, Kind::Breathing, Kind::Wave, Kind::Sparkle, Kind::Gradient, Kind::Fire]
    }

    #[test]
    fn every_animation_fits_the_frame_budget() {
        let leds = leds();
        let rainbow7 = vec![[255, 0, 0], [255, 128, 0], [255, 255, 0], [0, 255, 0], [0, 0, 255], [75, 0, 130], [148, 0, 211]];
        for kind in kinds() {
            for colors in [vec![[255, 20, 147]], Settings::default().colors, rainbow7.clone()] {
                for dir in [Direction::Right, Direction::Up] {
                    let s = Settings { kind, colors: colors.clone(), direction: dir, ..Settings::default() };
                    for step in 0..120 {
                        let f = frame(&leds, &s, step as f32 * 0.071).unwrap();
                        assert!(f.len() <= crate::protocol::LIVE_BUDGET, "{kind:?} {} colours: {} bytes", colors.len(), f.len());
                    }
                }
            }
        }
    }

    #[test]
    fn a_colour_per_key_is_merged_to_fit_with_every_key_kept() {
        let leds = leds();
        let groups: Vec<(Rgb, Vec<u8>)> =
            leds.iter().enumerate().map(|(i, l)| ([(i * 3) as u8, 255 - (i * 3) as u8, (i * 7 % 256) as u8], vec![l.index])).collect();
        let merged = fit(groups);
        assert!(encode(&merged).is_ok());
        assert_eq!(merged.iter().map(|g| g.1.len()).sum::<usize>(), 80);
        // two equal halves merge to their average
        let two = fit(vec![([200, 0, 0], (0..60).collect()), ([0, 0, 100], (60..120).collect())]);
        assert_eq!(two, vec![([100, 0, 50], (0..120).collect())]);
    }

    #[test]
    fn fire_burns_hot_at_its_base_and_moves() {
        let leds = leds();
        let s = Settings { kind: Kind::Fire, direction: Direction::Up, ..Settings::default() };
        let space = crate::keymap::key_by_name("space").unwrap().index;
        for step in 0..40 {
            let f = render(&leds, &s, step as f32 * 0.25);
            // the bottom row always burns
            assert!(f.iter().any(|(_, ks)| ks.contains(&space)), "step {step}: the space bar went dark");
            assert!(f.len() <= FIRE_LEVELS);
        }
        assert_ne!(render(&leds, &s, 0.0), render(&leds, &s, 0.5));
        // the base is hotter (brighter) than the top on average
        let total = |row: u8| -> u32 {
            let f = render(&leds, &s, 3.0);
            leds.iter()
                .filter(|l| l.row == row)
                .map(|l| f.iter().find(|(_, ks)| ks.contains(&l.index)).map_or(0, |(c, _)| c.iter().map(|&v| v as u32).sum()))
                .sum()
        };
        assert!(total(5) > total(0));
    }

    #[test]
    fn stream_uses_only_your_colours() {
        let leds = leds();
        let s = Settings::default(); // pink, red, orange
        for step in 0..50 {
            for (c, _) in render(&leds, &s, step as f32 * 0.2) {
                // blends of pink, red and orange: red always full, no blue beyond pink's 147
                assert!(c[0] == 255 && c[2] <= 147, "{c:?} isn't pink/red/orange");
            }
        }
    }

    #[test]
    fn stream_moves_and_gradient_stands_still() {
        let leds = leds();
        let s = Settings::default();
        assert_ne!(render(&leds, &s, 0.0), render(&leds, &s, 1.0));
        let g = Settings { kind: Kind::Gradient, ..s };
        assert_eq!(render(&leds, &g, 0.0), render(&leds, &g, 5.0));
    }

    #[test]
    fn breathing_goes_dark_between_colours_and_cycles_them() {
        let leds = leds();
        let s = Settings { kind: Kind::Breathing, ..Settings::default() };
        assert!(render(&leds, &s, 0.0).is_empty()); // start of a breath: off
        let first = render(&leds, &s, 2.0)[0].0; // middle of breath 1: pink
        let second = render(&leds, &s, 6.0)[0].0; // middle of breath 2: red
        assert_eq!(first, [255, 20, 147]);
        assert_eq!(second, [255, 0, 0]);
        assert_eq!(render(&leds, &s, 2.0)[0].1.len(), 80); // the whole board
    }

    #[test]
    fn every_key_is_lit_in_flowing_modes() {
        let leds = leds();
        for kind in [Kind::Stream, Kind::Wave, Kind::Gradient, Kind::Sparkle] {
            let s = Settings { kind, ..Settings::default() };
            let lit: usize = render(&leds, &s, 1.3).iter().map(|g| g.1.len()).sum();
            assert_eq!(lit, 80, "{kind:?}");
        }
    }
}
