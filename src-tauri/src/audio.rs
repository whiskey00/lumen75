//! Lighting that follows the computer's sound, like the official app's "Audio dance".
//!
//! What's playing is captured with WASAPI loopback (cpal), split into frequency bands with an
//! FFT, turned into per-key colours by a mode, and streamed to the keyboard as real-time
//! frames (CMD_LIVE). Nothing is stored on the keyboard: about 2 s after the last frame it goes
//! back to its own effect.

use crate::keymap;
use crate::protocol::{self as p, Rgb};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Frames per second we aim for; over 2.4G about 14 get through (9 packets × 5 ms each).
pub const FPS: u32 = 14;
const FFT_SIZE: usize = 2048;
const MIN_HZ: f32 = 40.0;
const MAX_HZ: f32 = 16_000.0;
/// Levels are relative to the loudest band of the last few seconds (automatic level), over
/// this many dB, so quiet and loud playback react alike.
const RANGE_DB: f32 = 36.0;
/// Below this the sound counts as silence (no automatic boost of noise).
const SILENCE_DB: f32 = -75.0;
/// How fast the reference falls back after a loud part (dB per frame).
const REF_FALL_DB: f32 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Bars rising from the bottom, bass on the left.
    Spectrum,
    /// Bars from the middle outwards, bass in the middle.
    Mirror,
    /// Rings spreading from the centre with the loudness.
    Ripple,
    /// The whole board pulses with the bass.
    Pulse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Mode,
    /// 0.2 - 4.0: how strongly the lights react.
    pub gain: f32,
    /// 0 - 9: how slowly the levels fall back.
    pub smoothness: u8,
    /// Rainbow colours instead of your `colors`.
    pub colorful: bool,
    /// One colour: what older settings saved, used when `colors` is empty.
    pub color: Rgb,
    /// Your colours (1-7), spread over the bands from the first to the last.
    #[serde(default)]
    pub colors: Vec<Rgb>,
    /// Output device to listen to, by name; None = the Windows default. Apps such as
    /// SteelSeries Sonar play music on a device other than the default.
    #[serde(default)]
    pub device: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Spectrum,
            gain: 1.0,
            smoothness: 4,
            colorful: true,
            color: [0, 255, 0],
            colors: Vec::new(),
            device: None,
        }
    }
}

// ------------------------------------------------------------ key geometry

/// A key with an LED: matrix index and centre, x and y in 0..1.
#[derive(Debug, Clone, Copy)]
pub struct Led {
    pub index: u8,
    pub x: f32,
    pub y: f32,
    /// 0 (top row) - 5 (bottom row)
    pub row: u8,
}

pub fn leds() -> Vec<Led> {
    let keys: Vec<_> = keymap::KEYS.iter().filter(|k| k.rect[2] > 0).collect();
    let cx = |k: &keymap::Key| (k.rect[0] + k.rect[2]) as f32 / 2.0;
    let cy = |k: &keymap::Key| (k.rect[1] + k.rect[3]) as f32 / 2.0;
    let (x0, x1) = keys.iter().map(|k| cx(k)).fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let (y0, y1) = keys.iter().map(|k| cy(k)).fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
    keys.iter()
        .map(|k| {
            let y = (cy(k) - y0) / (y1 - y0);
            Led { index: k.index, x: (cx(k) - x0) / (x1 - x0), y, row: (y * 5.0).round() as u8 }
        })
        .collect()
}

// ------------------------------------------------------------ analysis

/// Sound -> smoothed band levels (0..1).
pub struct Analyzer {
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    sample_rate: f32,
    levels: Vec<f32>,
    /// Loudest band of the last few seconds (dB): the automatic level.
    reference_db: f32,
}

impl Analyzer {
    pub fn new(sample_rate: u32, bands: usize) -> Self {
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
            .collect();
        Analyzer {
            fft: rustfft::FftPlanner::new().plan_fft_forward(FFT_SIZE),
            window,
            sample_rate: sample_rate as f32,
            levels: vec![0.0; bands],
            reference_db: SILENCE_DB,
        }
    }

    /// `samples`: the latest mono samples (the last FFT_SIZE are used).
    pub fn update(&mut self, samples: &[f32], s: &Settings) -> &[f32] {
        use rustfft::num_complex::Complex;
        let tail = &samples[samples.len().saturating_sub(FFT_SIZE)..];
        let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
        for (i, v) in tail.iter().enumerate() {
            let at = FFT_SIZE - tail.len() + i;
            buf[at] = Complex::new(v * self.window[at], 0.0);
        }
        self.fft.process(&mut buf);
        let bin_hz = self.sample_rate / FFT_SIZE as f32;
        let n = self.levels.len();
        let db: Vec<f32> = (0..n)
            .map(|b| {
                let lo = MIN_HZ * (MAX_HZ / MIN_HZ).powf(b as f32 / n as f32);
                let hi = MIN_HZ * (MAX_HZ / MIN_HZ).powf((b + 1) as f32 / n as f32);
                let i0 = (lo / bin_hz) as usize;
                let i1 = ((hi / bin_hz) as usize).min(FFT_SIZE / 2).max(i0 + 1); // at least one bin
                let peak = buf[i0..i1].iter().map(|c| c.norm()).fold(0.0f32, f32::max);
                // amplitude relative to a full-scale sine (window halves it), higher bands tilted up
                let amp = peak / (FFT_SIZE as f32 / 4.0) * (1.0 + b as f32 / n as f32);
                20.0 * amp.max(1e-9).log10()
            })
            .collect();
        // automatic level: jump up to a louder part at once, fall back slowly
        let loudest = db.iter().copied().fold(f32::MIN, f32::max);
        self.reference_db = loudest.max(self.reference_db - REF_FALL_DB).max(SILENCE_DB);
        let silent = loudest < SILENCE_DB;
        // a level falls by this much per frame: 0 = at once, 9 = slowly
        let fall = 0.45 - 0.045 * s.smoothness.min(9) as f32;
        for (level, d) in self.levels.iter_mut().zip(db) {
            let target = if silent {
                0.0
            } else {
                (((d - (self.reference_db - RANGE_DB)) / RANGE_DB) * s.gain).clamp(0.0, 1.0)
            };
            *level = if target > *level { target } else { (*level - fall).max(target) };
        }
        &self.levels
    }
}

// ------------------------------------------------------------ rendering

fn hsv(h: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u8 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [(r * v * 255.0) as u8, (g * v * 255.0) as u8, (b * v * 255.0) as u8]
}

fn scale(c: Rgb, v: f32) -> Rgb {
    let v = v.clamp(0.0, 1.0);
    [(c[0] as f32 * v) as u8, (c[1] as f32 * v) as u8, (c[2] as f32 * v) as u8]
}

/// How many bands each mode shows (each band is a colour group, and a frame only fits ~11).
pub fn bands(mode: Mode) -> usize {
    match mode {
        Mode::Spectrum => 11,
        Mode::Mirror => 6,
        Mode::Ripple => 8,
        Mode::Pulse => 4,
    }
}

/// Colour at `f` (0 = first colour, 1 = last) of a list, blended between neighbours.
fn spread(colors: &[Rgb], f: f32) -> Rgb {
    match colors.len() {
        0 => [255, 255, 255],
        1 => colors[0],
        n => {
            let x = f.clamp(0.0, 1.0) * (n - 1) as f32;
            let i = (x as usize).min(n - 2);
            let k = x - i as f32;
            [0, 1, 2].map(|c| (colors[i][c] as f32 + (colors[i + 1][c] as f32 - colors[i][c] as f32) * k).round() as u8)
        }
    }
}

/// Band levels -> colour groups. Each band's keys share one colour, so frames stay small and
/// keys never jump between groups (which would flicker).
pub fn render(leds: &[Led], levels: &[f32], s: &Settings, t: f32) -> Vec<(Rgb, Vec<u8>)> {
    let n = levels.len().max(1);
    let mine: &[Rgb] = if s.colors.is_empty() { std::slice::from_ref(&s.color) } else { &s.colors };
    let band_color = |band: usize, v: f32| -> Rgb {
        if s.colorful {
            hsv(band as f32 / n as f32 * 0.8 + t * 0.02, v)
        } else {
            scale(spread(mine, band as f32 / (n - 1).max(1) as f32), v)
        }
    };
    let mut groups: BTreeMap<usize, (Rgb, Vec<u8>)> = BTreeMap::new();
    let mut put = |band: usize, color: Rgb, index: u8| {
        groups.entry(band).or_insert_with(|| (color, Vec::new())).1.push(index);
    };
    match s.mode {
        Mode::Spectrum | Mode::Mirror => {
            for l in leds {
                let band = if s.mode == Mode::Spectrum {
                    ((l.x * n as f32) as usize).min(n - 1)
                } else {
                    (((l.x - 0.5).abs() * 2.0 * n as f32) as usize).min(n - 1)
                };
                // bar height in rows from the bottom (row 5); a little level lights the bottom row
                let height = (levels[band] * 6.0).ceil() as u8;
                if 5 - l.row < height {
                    put(band, band_color(band, 1.0), l.index);
                }
            }
        }
        Mode::Ripple => {
            let loud = levels.iter().take(n / 2).copied().fold(0.0f32, f32::max);
            for l in leds {
                let d = ((l.x - 0.5).powi(2) + (0.45 * (l.y - 0.5)).powi(2)).sqrt() / 0.55;
                let ring = ((d * n as f32) as usize).min(n - 1);
                if (ring as f32) < loud * n as f32 {
                    put(ring, band_color(ring, 1.0 - 0.5 * ring as f32 / n as f32), l.index);
                }
            }
        }
        Mode::Pulse => {
            let bass = levels[0].max(levels.get(1).copied().unwrap_or(0.0));
            let v = bass * bass;
            // the pulse slowly moves through your colours (and back), like it does the rainbow
            let color = if s.colorful {
                hsv(t * 0.05, v)
            } else {
                let u = (t * 0.05).rem_euclid(2.0);
                scale(spread(mine, if u > 1.0 { 2.0 - u } else { u }), v)
            };
            for l in leds {
                put(0, color, l.index);
            }
        }
    }
    groups.into_values().filter(|(c, _)| *c != [0, 0, 0]).collect()
}

/// Groups -> the frame, within the size that streams reliably.
pub fn encode(groups: &[(Rgb, Vec<u8>)]) -> p::Result<Vec<u8>> {
    let frame = p::live_frame(groups)?;
    if frame.len() > p::LIVE_BUDGET {
        return p::err(format!("audio frame too long ({} bytes)", frame.len()));
    }
    Ok(frame)
}

// ------------------------------------------------------------ running it

/// What the UI gets every frame: the band levels and the key colours shown.
#[derive(Debug, Clone, Serialize)]
pub struct FrameEvent {
    pub levels: Vec<f32>,
    pub keys: Vec<(u8, Rgb)>,
    /// No sound arrived from the device for a second.
    pub silent: bool,
    /// The device listened to.
    pub device: String,
}

#[cfg(windows)]
pub mod run {
    use super::*;
    use crate::device::Keyboard;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    pub struct Handle {
        pub stop: Arc<AtomicBool>,
        pub settings: Arc<Mutex<Settings>>,
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
        Stopped,
    }

    /// What the capture callback fills: the latest mono samples and when they came.
    #[derive(Default)]
    struct Input {
        samples: VecDeque<f32>,
        last: Option<Instant>,
    }

    /// Output devices that can be listened to, and the Windows default.
    pub fn output_devices() -> (Vec<String>, Option<String>) {
        let host = cpal::default_host();
        let default = host.default_output_device().and_then(|d| d.name().ok());
        let names = host.output_devices().map(|ds| ds.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
        (names, default)
    }

    /// Start capturing and streaming on a thread of its own (cpal streams can't move between
    /// threads). `open` gives the keyboard; `emit` gets frames and errors for the UI.
    pub fn start(
        kb: Arc<Mutex<Option<Keyboard>>>,
        open: fn() -> crate::device::Result<Keyboard>,
        settings: Settings,
        emit: impl Fn(Event) + Send + 'static,
    ) -> Result<Handle, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(settings));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let (stop2, shared2) = (stop.clone(), shared.clone());
        let thread = std::thread::spawn(move || {
            let input: Arc<Mutex<Input>> = Arc::default();
            let mut device = shared2.lock().unwrap().device.clone();
            let (mut stream, mut rate, mut device_name) = match capture(device.as_deref(), input.clone()) {
                Ok(v) => {
                    let _ = ready_tx.send(Ok(()));
                    v
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let leds = leds();
            let mut mode = shared2.lock().unwrap().mode;
            let mut analyzer = Analyzer::new(rate, bands(mode));
            let started = Instant::now();
            let period = Duration::from_secs_f32(1.0 / FPS as f32);
            let mut failures = 0u32;
            while !stop2.load(Ordering::Relaxed) {
                let tick = Instant::now();
                let s = shared2.lock().unwrap().clone();
                if s.device != device {
                    device = s.device.clone();
                    *input.lock().unwrap() = Input::default();
                    // the old stream is dropped when the new one replaces it
                    match capture(device.as_deref(), input.clone()).or_else(|e| {
                        emit(Event::Error(e));
                        capture(None, input.clone()) // fall back to the default device
                    }) {
                        Ok((st, r, name)) => {
                            (stream, rate, device_name) = (st, r, name);
                            analyzer = Analyzer::new(rate, bands(mode));
                        }
                        Err(e) => {
                            emit(Event::Error(e));
                            break;
                        }
                    }
                }
                if s.mode != mode {
                    mode = s.mode;
                    analyzer = Analyzer::new(rate, bands(mode));
                }
                // Loopback stops calling back while nothing plays, so old samples must not
                // keep the lights up: anything older than a moment counts as silence.
                let (buf, silent) = {
                    let i = input.lock().unwrap();
                    let age = i.last.map(|t| t.elapsed());
                    let fresh = age.is_some_and(|a| a < Duration::from_millis(200));
                    let buf: Vec<f32> = if fresh { i.samples.iter().copied().collect() } else { Vec::new() };
                    (buf, !age.is_some_and(|a| a < Duration::from_secs(1)))
                };
                let levels = analyzer.update(&buf, &s).to_vec();
                let groups = render(&leds, &levels, &s, started.elapsed().as_secs_f32());
                let sent = crate::device::send_frame(&kb, open, &groups);
                match sent {
                    Ok(()) => failures = 0,
                    Err(e) => {
                        failures += 1;
                        if failures == 1 || failures % 30 == 0 {
                            emit(Event::Error(e));
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
                let keys = groups.iter().flat_map(|(c, ks)| ks.iter().map(move |&i| (i, *c))).collect();
                emit(Event::Frame(FrameEvent { levels, keys, silent, device: device_name.clone() }));
                if let Some(rest) = period.checked_sub(tick.elapsed()) {
                    std::thread::sleep(rest);
                }
            }
            drop(stream);
            emit(Event::Stopped);
        });
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Handle { stop, settings: shared, thread: Some(thread) }),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                Err("The audio capture didn't start.".into())
            }
        }
    }

    /// Loopback capture of an output device (by name; None = the default) into `input`
    /// (mono, newest last). Gives the stream, its sample rate and the device's name.
    fn capture(name: Option<&str>, input: Arc<Mutex<Input>>) -> Result<(cpal::Stream, u32, String), String> {
        let host = cpal::default_host();
        let device = match name {
            Some(n) => host
                .output_devices()
                .map_err(|e| format!("Audio devices: {e}"))?
                .find(|d| d.name().is_ok_and(|x| x == n))
                .ok_or_else(|| format!("The audio device “{n}” isn't there any more."))?,
            None => host.default_output_device().ok_or("No audio output device found.")?,
        };
        let device_name = device.name().unwrap_or_default();
        let config = device.default_output_config().map_err(|e| format!("Audio output: {e}"))?;
        let channels = config.channels() as usize;
        let rate = config.sample_rate().0;
        let push = move |mono: &mut dyn Iterator<Item = f32>| {
            let mut i = input.lock().unwrap();
            i.samples.extend(mono);
            let excess = i.samples.len().saturating_sub(FFT_SIZE * 2);
            i.samples.drain(..excess);
            i.last = Some(Instant::now());
        };
        let err = |e| eprintln!("audio stream error: {e}");
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| push(&mut data.chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32)),
                err,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    push(&mut data.chunks(channels).map(|f| f.iter().map(|&v| v as f32 / 32768.0).sum::<f32>() / channels as f32))
                },
                err,
                None,
            ),
            other => return Err(format!("Unsupported audio format {other:?}.")),
        }
        .map_err(|e| format!("Can't capture the computer's sound: {e}"))?;
        stream.play().map_err(|e| format!("Can't start the audio capture: {e}"))?;
        Ok((stream, rate, device_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, rate: u32, amp: f32) -> Vec<f32> {
        (0..FFT_SIZE).map(|i| amp * (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin()).collect()
    }

    #[test]
    fn leds_cover_the_board() {
        let l = leds();
        assert_eq!(l.len(), 80); // every key but the knob
        assert!(l.iter().all(|k| (0.0..=1.0).contains(&k.x) && (0.0..=1.0).contains(&k.y) && k.row <= 5));
        assert_eq!(l.iter().find(|k| k.index == 0).unwrap().row, 0); // Esc on top
        assert_eq!(l.iter().find(|k| k.index == 35).unwrap().row, 5); // Space at the bottom
    }

    #[test]
    fn a_bass_tone_lights_the_low_bands() {
        let s = Settings { gain: 1.0, ..Default::default() };
        let mut a = Analyzer::new(48_000, 11);
        let levels = a.update(&sine(60.0, 48_000, 0.5), &s).to_vec();
        assert!(levels[0].max(levels[1]) > 0.9, "{levels:?}"); // 60 Hz sits between bands 0 and 1
        assert!(levels[3..].iter().all(|&v| v < 0.2), "{levels:?}");
        // silence: levels fall back over a few frames
        let mut last = 1.0;
        for _ in 0..10 {
            last = a.update(&vec![0.0; FFT_SIZE], &s)[..2].iter().copied().fold(0.0, f32::max);
        }
        assert!(last < 0.05);
    }

    #[test]
    fn quiet_playback_reacts_like_loud_playback() {
        let s = Settings::default();
        let loud = Analyzer::new(48_000, 11).update(&sine(60.0, 48_000, 0.5), &s).to_vec();
        let quiet = Analyzer::new(48_000, 11).update(&sine(60.0, 48_000, 0.01), &s).to_vec();
        assert!((loud[0] - quiet[0]).abs() < 0.05, "{loud:?} vs {quiet:?}");
        // but near-silence isn't boosted into light
        let hiss = Analyzer::new(48_000, 11).update(&sine(60.0, 48_000, 0.00005), &s).to_vec();
        assert!(hiss.iter().all(|&v| v == 0.0), "{hiss:?}");
    }

    #[test]
    fn every_mode_fits_the_frame_budget_at_any_level() {
        let leds = leds();
        for mode in [Mode::Spectrum, Mode::Mirror, Mode::Ripple, Mode::Pulse] {
            for colorful in [true, false] {
                for level in [0.0, 0.3, 0.7, 1.0] {
                    let s = Settings { mode, colorful, ..Settings::default() };
                    let groups = render(&leds, &vec![level; bands(mode)], &s, 3.0);
                    let frame = encode(&groups).unwrap();
                    assert!(frame.len() <= p::LIVE_BUDGET, "{mode:?} {level}: {} bytes", frame.len());
                    if level == 1.0 {
                        let lit: usize = groups.iter().map(|g| g.1.len()).sum();
                        assert!(lit >= 60, "{mode:?} lights only {lit} keys at full level");
                    }
                }
            }
        }
    }

    #[test]
    fn your_colours_spread_over_the_bands() {
        let s = Settings {
            colorful: false,
            colors: vec![[255, 20, 147], [255, 0, 0], [255, 110, 0]],
            ..Settings::default()
        };
        let groups = render(&leds(), &[1.0; 11], &s, 0.0);
        let colours: Vec<Rgb> = groups.iter().map(|g| g.0).collect();
        assert!(colours.contains(&[255, 20, 147]), "first colour on the bass side: {colours:?}");
        assert!(colours.contains(&[255, 110, 0]), "last colour on the treble side: {colours:?}");
        assert!(colours.iter().all(|c| c[0] == 255 && c[2] <= 147), "only blends of your colours: {colours:?}");
        // settings saved before colour lists: the single colour is used
        let old = Settings { colorful: false, color: [0, 0, 255], ..Settings::default() };
        assert!(render(&leds(), &[1.0; 11], &old, 0.0).iter().all(|g| g.0 == [0, 0, 255]));
    }

    #[test]
    fn silence_is_a_dark_frame() {
        let s = Settings::default();
        let groups = render(&leds(), &[0.0; 11], &s, 0.0);
        assert!(groups.is_empty());
        assert_eq!(encode(&groups).unwrap(), vec![0, 0, 0, 1, 0]);
    }
}
