//! Discord lighting: voice and notification events from the Discord app on this PC, through
//! its local RPC (the named pipe `discord-ipc-N`). You create your own Discord application on
//! the Developer Portal and give Lumen75 its Client ID and Secret; Discord then asks you once
//! to approve it. RPC needs no approval from Discord for the app's owner (and its testers).
//!
//! What it shows is an overlay: indicators that stay while something is true (muted,
//! deafened, speaking, ...) and short effects (a notification, someone joining your channel).
//! It's painted over whatever else streams (animations, audio, League lighting); when nothing
//! does, it streams on its own while it has something to show, and otherwise leaves the
//! keyboard to its own effect.

use crate::audio::Led;
use crate::gamelight::Style;
use crate::protocol::Rgb;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ------------------------------------------------------------ what it shows

/// Something that stays lit while it's true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Indicator {
    /// You're in a voice channel.
    InVoice,
    /// Someone else in your channel is talking.
    OthersSpeaking,
    /// You're talking.
    Speaking,
    Muted,
    Deafened,
}

/// A moment that plays a short effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    /// A Discord notification (a DM or a mention).
    Notification,
    /// You joined a voice channel.
    JoinedVoice,
    /// You left it.
    LeftVoice,
    /// Someone joined your voice channel.
    UserJoined,
    /// Someone left it.
    UserLeft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorSetup {
    pub indicator: Indicator,
    pub on: bool,
    /// Matrix indexes.
    pub keys: Vec<u8>,
    pub color: Rgb,
}

impl Default for IndicatorSetup {
    fn default() -> Self {
        IndicatorSetup { indicator: Indicator::Muted, on: false, keys: Vec::new(), color: [255, 0, 0] }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectSetup {
    pub event: Event,
    pub on: bool,
    pub style: Style,
    pub color: Rgb,
    pub ms: u32,
}

impl Default for EffectSetup {
    fn default() -> Self {
        EffectSetup { event: Event::Notification, on: false, style: Style::Flash, color: [88, 101, 242], ms: 600 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundKind {
    /// The rest of the board stays dark while Discord streams on its own.
    Off,
    Solid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Background {
    pub kind: BackgroundKind,
    pub color: Rgb,
}

impl Default for Background {
    fn default() -> Self {
        Background { kind: BackgroundKind::Off, color: [20, 20, 30] }
    }
}

/// Everything Discord lighting shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Setup {
    /// Behind the indicators when nothing else streams.
    pub background: Background,
    pub indicators: Vec<IndicatorSetup>,
    pub effects: Vec<EffectSetup>,
}

fn keys(names: &[&str]) -> Vec<u8> {
    names.iter().filter_map(|n| crate::keymap::key_by_name(n).map(|k| k.index)).collect()
}

const F_ROW: [&str; 12] = ["f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"];

impl Default for Setup {
    fn default() -> Self {
        let ind = |indicator, on, names: &[&str], color| IndicatorSetup { indicator, on, keys: keys(names), color };
        let fx = |event, on, style, color, ms| EffectSetup { event, on, style, color, ms };
        Setup {
            background: Background::default(),
            // later ones paint over earlier ones: deafened wins over muted
            indicators: vec![
                ind(Indicator::InVoice, false, &["esc"], [0, 200, 80]),
                ind(Indicator::OthersSpeaking, false, &["space"], [0, 140, 255]),
                ind(Indicator::Speaking, true, &["space"], [0, 255, 80]),
                ind(Indicator::Muted, true, &F_ROW, [255, 0, 0]),
                ind(Indicator::Deafened, true, &F_ROW, [255, 90, 0]),
            ],
            effects: vec![
                fx(Event::Notification, true, Style::Flash, [88, 101, 242], 700),
                fx(Event::JoinedVoice, true, Style::Wave, [0, 255, 80], 1200),
                fx(Event::LeftVoice, true, Style::Wave, [255, 0, 0], 1200),
                fx(Event::UserJoined, true, Style::Flash, [0, 255, 120], 500),
                fx(Event::UserLeft, false, Style::Flash, [255, 80, 0], 500),
            ],
        }
    }
}

/// What Discord says right now, for the app's page.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Voice {
    pub in_voice: bool,
    pub muted: bool,
    pub deafened: bool,
    pub speaking: bool,
    pub others_speaking: usize,
}

/// Discord's state as far as lighting cares, and the effect that's playing.
#[derive(Debug, Default)]
pub struct Overlay {
    pub voice: Voice,
    /// Your user id, from AUTHENTICATE.
    pub me: Option<String>,
    pub channel: Option<String>,
    speakers: HashSet<String>,
    active: Option<(EffectSetup, f32)>,
}

fn mix(a: Rgb, b: Rgb, f: f32) -> Rgb {
    let f = f.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * f).round() as u8)
}

impl Overlay {
    /// Play the effect for `event`, if it's on (a newer effect replaces the playing one).
    pub fn trigger(&mut self, event: Event, setup: &Setup, now: f32) {
        if let Some(e) = setup.effects.iter().find(|e| e.event == event && e.on) {
            self.active = Some((e.clone(), now));
        }
    }

    fn is(&self, i: Indicator) -> bool {
        let v = &self.voice;
        match i {
            Indicator::InVoice => v.in_voice,
            Indicator::OthersSpeaking => v.in_voice && v.others_speaking > 0,
            Indicator::Speaking => v.in_voice && v.speaking && !v.muted,
            Indicator::Muted => v.muted && !v.deafened,
            Indicator::Deafened => v.deafened,
        }
    }

    /// The keys to paint now, in order, as (key, colour, strength 0-1). Empty when there's
    /// nothing to show.
    pub fn paint(&mut self, setup: &Setup, leds: &[Led], now: f32) -> Vec<(u8, Rgb, f32)> {
        let mut out = Vec::new();
        for ind in setup.indicators.iter().filter(|i| i.on && self.is(i.indicator)) {
            out.extend(ind.keys.iter().map(|&k| (k, ind.color, 1.0)));
        }
        if let Some((e, start)) = &self.active {
            let len = e.ms.max(1) as f32 / 1000.0;
            let p = (now - start) / len;
            if !(0.0..1.0).contains(&p) {
                self.active = None;
            } else {
                for l in leds {
                    let a = match e.style {
                        Style::Flash => 1.0 - p,
                        Style::Pulse => (std::f32::consts::PI * p).sin(),
                        Style::Wave => (1.0 - (l.x - (p * 1.4 - 0.2)).abs() / 0.2).max(0.0),
                    };
                    if a > 0.01 {
                        out.push((l.index, e.color, a));
                    }
                }
            }
        }
        out
    }

    /// Apply a dispatched event from Discord. Returns the voice channel to watch when it
    /// changed (Some(None) = left).
    pub fn dispatch(&mut self, evt: &str, data: &Value, setup: &Setup, now: f32) -> Option<Option<String>> {
        let user = |d: &Value| d["user_id"].as_str().or_else(|| d["user"]["id"].as_str()).map(str::to_string);
        match evt {
            "VOICE_SETTINGS_UPDATE" => {
                self.voice.muted = data["mute"].as_bool().unwrap_or(self.voice.muted);
                self.voice.deafened = data["deaf"].as_bool().unwrap_or(self.voice.deafened);
            }
            "VOICE_CHANNEL_SELECT" => {
                let channel = data["channel_id"].as_str().map(str::to_string);
                if channel != self.channel {
                    let event = if channel.is_some() { Event::JoinedVoice } else { Event::LeftVoice };
                    self.trigger(event, setup, now);
                    return Some(self.set_channel(channel));
                }
            }
            "VOICE_STATE_CREATE" if user(data) != self.me => self.trigger(Event::UserJoined, setup, now),
            "VOICE_STATE_DELETE" if user(data) != self.me => {
                if let Some(u) = user(data) {
                    self.speakers.remove(&u);
                    self.voice.others_speaking = self.speakers.len();
                }
                self.trigger(Event::UserLeft, setup, now);
            }
            "SPEAKING_START" | "SPEAKING_STOP" => {
                let on = evt == "SPEAKING_START";
                match user(data) {
                    Some(u) if Some(&u) == self.me.as_ref() => self.voice.speaking = on,
                    Some(u) => {
                        if on {
                            self.speakers.insert(u);
                        } else {
                            self.speakers.remove(&u);
                        }
                        self.voice.others_speaking = self.speakers.len();
                    }
                    None => {}
                }
            }
            "NOTIFICATION_CREATE" => self.trigger(Event::Notification, setup, now),
            _ => {}
        }
        None
    }

    /// The channel you're in, without an effect (at connect time).
    pub fn set_channel(&mut self, channel: Option<String>) -> Option<String> {
        self.voice.in_voice = channel.is_some();
        self.speakers.clear();
        self.voice.others_speaking = 0;
        self.voice.speaking = false;
        self.channel = channel.clone();
        channel
    }
}

/// `paint` over a frame: each painted key blends from its colour (black when the frame
/// leaves it out) towards the paint by its strength.
pub fn compose(base: &[(Rgb, Vec<u8>)], paint: &[(u8, Rgb, f32)]) -> Vec<(Rgb, Vec<u8>)> {
    let mut colors: HashMap<u8, Rgb> = base.iter().flat_map(|(c, ks)| ks.iter().map(move |&k| (k, *c))).collect();
    for &(k, c, a) in paint {
        let under = colors.get(&k).copied().unwrap_or([0, 0, 0]);
        colors.insert(k, mix(under, c, a));
    }
    let mut groups: BTreeMap<Rgb, Vec<u8>> = BTreeMap::new();
    for (k, c) in colors {
        if c != [0, 0, 0] {
            groups.entry(c).or_default().push(k);
        }
    }
    groups.into_iter().map(|(c, mut ks)| { ks.sort_unstable(); (c, ks) }).collect()
}

/// The frame Discord streams on its own: its background with the paint on top.
pub fn own_frame(setup: &Setup, leds: &[Led], paint: &[(u8, Rgb, f32)]) -> Vec<(Rgb, Vec<u8>)> {
    let base = match setup.background.kind {
        BackgroundKind::Off => Vec::new(),
        BackgroundKind::Solid => vec![(setup.background.color, leds.iter().map(|l| l.index).collect())],
    };
    compose(&base, paint)
}

// ------------------------------------------------------------ sharing with other streams

/// The latest paint, for the other streams to put over their frames.
static PAINT: Mutex<Option<(Instant, Vec<(u8, Rgb, f32)>)>> = Mutex::new(None);
/// When another stream last sent a frame.
static OTHER_STREAM: Mutex<Option<Instant>> = Mutex::new(None);

fn set_paint(p: Vec<(u8, Rgb, f32)>) {
    *PAINT.lock().unwrap() = if p.is_empty() { None } else { Some((Instant::now(), p)) };
}

/// Put Discord's overlay over a frame from another stream (and note that one streams).
/// None when Discord has nothing to show.
pub fn overlay(groups: &[(Rgb, Vec<u8>)]) -> Option<Vec<(Rgb, Vec<u8>)>> {
    *OTHER_STREAM.lock().unwrap() = Some(Instant::now());
    let paint = PAINT.lock().unwrap();
    let (at, p) = paint.as_ref()?;
    (at.elapsed() < Duration::from_millis(500)).then(|| compose(groups, p))
}

fn other_stream_running() -> bool {
    OTHER_STREAM.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(400))
}

// ------------------------------------------------------------ the RPC protocol

/// Opcodes of the IPC frames.
pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;

/// An IPC frame: opcode and length (little-endian u32 each), then JSON.
pub fn encode(op: u32, body: &Value) -> Vec<u8> {
    let json = body.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + json.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    out
}

/// Take one whole frame off the front of `buf`, if it's all there.
pub fn decode(buf: &mut Vec<u8>) -> Option<(u32, Value)> {
    if buf.len() < 8 {
        return None;
    }
    let op = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if buf.len() < 8 + len {
        return None;
    }
    let body = serde_json::from_slice(&buf[8..8 + len]).unwrap_or(Value::Null);
    buf.drain(..8 + len);
    Some((op, body))
}

/// The scopes Lumen75 asks for: voice state and notifications, read only.
pub const SCOPES: [&str; 3] = ["rpc", "rpc.voice.read", "rpc.notifications.read"];

/// Your Discord application and the sign-in Discord gave it (saved with the settings).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Auth {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at: u64,
}

impl Auth {
    fn token_valid(&self) -> bool {
        !self.access_token.is_empty() && unix_now() + 60 < self.expires_at
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Swap an authorisation code or a refresh token for an access token.
fn token(auth: &Auth, grant: &[(&str, &str)]) -> Result<Auth, String> {
    let send = |extra: &[(&str, &str)]| {
        let mut form = vec![("client_id", auth.client_id.as_str()), ("client_secret", auth.client_secret.as_str())];
        form.extend_from_slice(grant);
        form.extend_from_slice(extra);
        ureq::post("https://discord.com/api/oauth2/token").timeout(Duration::from_secs(15)).send_form(&form)
    };
    let mut reply = send(&[]);
    // some apps need the redirect the portal lists; http://localhost is the one we ask for
    if matches!(reply, Err(ureq::Error::Status(400, _))) {
        reply = send(&[("redirect_uri", "http://localhost")]);
    }
    let body = match reply {
        Ok(r) => r.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            return Err(format!("Discord refused the sign-in ({code}): {text}. Check the Client ID and Secret."));
        }
        Err(e) => return Err(format!("Can't reach Discord to sign in: {e}")),
    };
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let access = v["access_token"].as_str().ok_or("Discord's reply had no access token.")?;
    Ok(Auth {
        access_token: access.to_string(),
        refresh_token: v["refresh_token"].as_str().unwrap_or(&auth.refresh_token).to_string(),
        expires_at: unix_now() + v["expires_in"].as_u64().unwrap_or(3600),
        ..auth.clone()
    })
}

fn command(cmd: &str, args: Value, evt: Option<&str>) -> Value {
    static NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let nonce = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed).to_string();
    let mut v = json!({ "cmd": cmd, "args": args, "nonce": nonce });
    if let Some(e) = evt {
        v["evt"] = json!(e);
    }
    v
}

#[cfg(windows)]
pub mod run {
    use super::*;
    use crate::device::Keyboard;
    use std::io::{Read, Write};
    use std::os::windows::io::AsRawHandle;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    pub enum Event {
        /// One line for the page: connecting, waiting for approval, connected as ...
        Status(String),
        Voice(Voice),
        /// New tokens to save.
        Auth(Auth),
        /// The keys Discord painted (for the preview).
        Frame(Vec<(u8, Rgb)>),
    }

    pub struct Handle {
        stop: Arc<AtomicBool>,
        pub setup: Arc<Mutex<Setup>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Handle {
        pub fn stop(mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            set_paint(Vec::new());
        }
    }

    /// The pipe to the Discord app: the first `discord-ipc-N` that opens.
    struct Pipe {
        file: std::fs::File,
        buf: Vec<u8>,
    }

    impl Pipe {
        fn open() -> Option<Pipe> {
            (0..10).find_map(|n| {
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(format!(r"\\.\pipe\discord-ipc-{n}"))
                    .ok()
                    .map(|file| Pipe { file, buf: Vec::new() })
            })
        }

        fn send(&mut self, op: u32, body: &Value) -> Result<(), String> {
            self.file.write_all(&encode(op, body)).map_err(|e| format!("Discord closed the connection: {e}"))
        }

        /// The next whole frame, without blocking: None when nothing (complete) has arrived.
        fn poll(&mut self) -> Result<Option<(u32, Value)>, String> {
            if let Some(f) = decode(&mut self.buf) {
                return Ok(Some(f));
            }
            let mut avail = 0u32;
            let ok = unsafe {
                PeekNamedPipe(self.file.as_raw_handle() as _, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut())
            };
            if ok == 0 {
                return Err("Discord closed the connection.".into());
            }
            if avail > 0 {
                let mut chunk = vec![0u8; avail as usize];
                let n = self.file.read(&mut chunk).map_err(|e| format!("Discord closed the connection: {e}"))?;
                self.buf.extend_from_slice(&chunk[..n]);
            }
            Ok(decode(&mut self.buf))
        }
    }

    const CHANNEL_EVENTS: [&str; 4] = ["SPEAKING_START", "SPEAKING_STOP", "VOICE_STATE_CREATE", "VOICE_STATE_DELETE"];

    pub fn start(
        kb: Arc<Mutex<Option<Keyboard>>>,
        open: fn() -> crate::device::Result<Keyboard>,
        auth: Auth,
        setup: Setup,
        emit: impl Fn(Event) + Send + 'static,
    ) -> Handle {
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(setup));
        let (stop2, shared2) = (stop.clone(), shared.clone());
        let thread = std::thread::spawn(move || {
            let leds = crate::audio::leds();
            let started = Instant::now();
            let mut auth = auth;
            let mut status = String::new();
            let say = |s: &str, status: &mut String| {
                if s != status {
                    *status = s.to_string();
                    emit(Event::Status(s.to_string()));
                }
            };
            let mut voice_seen = Voice::default();
            let mut streaming = false;
            'connect: while !stop2.load(Ordering::Relaxed) {
                if auth.client_id.trim().is_empty() || auth.client_secret.trim().is_empty() {
                    say("Add your Discord application's Client ID and Secret below.", &mut status);
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                let Some(mut pipe) = Pipe::open() else {
                    say("Waiting for the Discord app to run on this PC.", &mut status);
                    for _ in 0..30 {
                        if stop2.load(Ordering::Relaxed) {
                            break 'connect;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    continue;
                };
                say("Connecting to Discord.", &mut status);
                if let Err(e) = pipe.send(OP_HANDSHAKE, &json!({ "v": 1, "client_id": auth.client_id.trim() })) {
                    say(&e, &mut status);
                    continue;
                }
                let mut overlay = Overlay::default();
                let period = Duration::from_secs_f32(1.0 / crate::audio::FPS as f32);
                let mut next_frame = Instant::now();
                while !stop2.load(Ordering::Relaxed) {
                    // ---- messages from Discord
                    let msg = match pipe.poll() {
                        Ok(m) => m,
                        Err(e) => {
                            say(&e, &mut status);
                            std::thread::sleep(Duration::from_secs(2));
                            continue 'connect;
                        }
                    };
                    if let Some((op, body)) = msg {
                        let now = started.elapsed().as_secs_f32();
                        let setup = shared2.lock().unwrap().clone();
                        let cmd = body["cmd"].as_str().unwrap_or("");
                        let evt = body["evt"].as_str().unwrap_or("");
                        let reply: Result<(), String> = (|| {
                            match (op, cmd, evt) {
                                (OP_PING, _, _) => pipe.send(OP_PONG, &body)?,
                                (OP_CLOSE, _, _) => {
                                    let why = body["message"].as_str().unwrap_or("Discord closed the connection.");
                                    return Err(format!("Discord closed the connection: {why}"));
                                }
                                (_, "DISPATCH", "READY") => {
                                    if auth.token_valid() {
                                        pipe.send(OP_FRAME, &command("AUTHENTICATE", json!({ "access_token": auth.access_token }), None))?;
                                    } else if !auth.refresh_token.is_empty() {
                                        let refresh = auth.refresh_token.clone();
                                        match token(&auth, &[("grant_type", "refresh_token"), ("refresh_token", &refresh)]) {
                                            Ok(a) => {
                                                auth = a;
                                                emit(Event::Auth(auth.clone()));
                                                pipe.send(OP_FRAME, &command("AUTHENTICATE", json!({ "access_token": auth.access_token }), None))?;
                                            }
                                            Err(_) => {
                                                auth.refresh_token.clear();
                                                pipe.send(OP_FRAME, &command("AUTHORIZE", json!({ "client_id": auth.client_id.trim(), "scopes": SCOPES }), None))?;
                                                say("Approve Lumen75 in the Discord app.", &mut status);
                                            }
                                        }
                                    } else {
                                        pipe.send(OP_FRAME, &command("AUTHORIZE", json!({ "client_id": auth.client_id.trim(), "scopes": SCOPES }), None))?;
                                        say("Approve Lumen75 in the Discord app.", &mut status);
                                    }
                                }
                                (_, "AUTHORIZE", "ERROR") => {
                                    let why = body["data"]["message"].as_str().unwrap_or("not approved");
                                    say(&format!("Discord didn't approve Lumen75: {why}. Press Reconnect to ask again."), &mut status);
                                }
                                (_, "AUTHORIZE", _) => {
                                    let code = body["data"]["code"].as_str().unwrap_or("").to_string();
                                    say("Signing in to Discord.", &mut status);
                                    match token(&auth, &[("grant_type", "authorization_code"), ("code", &code)]) {
                                        Ok(a) => {
                                            auth = a;
                                            emit(Event::Auth(auth.clone()));
                                            pipe.send(OP_FRAME, &command("AUTHENTICATE", json!({ "access_token": auth.access_token }), None))?;
                                        }
                                        Err(e) => say(&e, &mut status),
                                    }
                                }
                                (_, "AUTHENTICATE", "ERROR") => {
                                    // an old or revoked token: ask again
                                    auth.access_token.clear();
                                    auth.refresh_token.clear();
                                    emit(Event::Auth(auth.clone()));
                                    pipe.send(OP_FRAME, &command("AUTHORIZE", json!({ "client_id": auth.client_id.trim(), "scopes": SCOPES }), None))?;
                                    say("Approve Lumen75 in the Discord app.", &mut status);
                                }
                                (_, "AUTHENTICATE", _) => {
                                    overlay.me = body["data"]["user"]["id"].as_str().map(str::to_string);
                                    let name = body["data"]["user"]["global_name"]
                                        .as_str()
                                        .or(body["data"]["user"]["username"].as_str())
                                        .unwrap_or("you");
                                    say(&format!("Connected as {name}."), &mut status);
                                    for e in ["VOICE_SETTINGS_UPDATE", "VOICE_CHANNEL_SELECT", "NOTIFICATION_CREATE"] {
                                        pipe.send(OP_FRAME, &command("SUBSCRIBE", json!({}), Some(e)))?;
                                    }
                                    pipe.send(OP_FRAME, &command("GET_VOICE_SETTINGS", json!({}), None))?;
                                    pipe.send(OP_FRAME, &command("GET_SELECTED_VOICE_CHANNEL", json!({}), None))?;
                                }
                                (_, "GET_VOICE_SETTINGS", _) => {
                                    overlay.dispatch("VOICE_SETTINGS_UPDATE", &body["data"], &setup, now);
                                }
                                (_, "GET_SELECTED_VOICE_CHANNEL", _) => {
                                    let channel = body["data"]["id"].as_str().map(str::to_string);
                                    if let Some(c) = overlay.set_channel(channel) {
                                        for e in CHANNEL_EVENTS {
                                            pipe.send(OP_FRAME, &command("SUBSCRIBE", json!({ "channel_id": c }), Some(e)))?;
                                        }
                                    }
                                }
                                (_, "DISPATCH", e) => {
                                    let before = overlay.channel.clone();
                                    if let Some(now_in) = overlay.dispatch(e, &body["data"], &setup, now) {
                                        if let Some(old) = before {
                                            for e in CHANNEL_EVENTS {
                                                pipe.send(OP_FRAME, &command("UNSUBSCRIBE", json!({ "channel_id": old }), Some(e)))?;
                                            }
                                        }
                                        if let Some(c) = now_in {
                                            for e in CHANNEL_EVENTS {
                                                pipe.send(OP_FRAME, &command("SUBSCRIBE", json!({ "channel_id": c }), Some(e)))?;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                            Ok(())
                        })();
                        if let Err(e) = reply {
                            say(&e, &mut status);
                            std::thread::sleep(Duration::from_secs(2));
                            continue 'connect;
                        }
                        if overlay.voice != voice_seen {
                            voice_seen = overlay.voice.clone();
                            emit(Event::Voice(voice_seen.clone()));
                        }
                        continue; // more messages may be waiting
                    }
                    // ---- lighting
                    if Instant::now() >= next_frame {
                        next_frame = Instant::now() + period;
                        let setup = shared2.lock().unwrap().clone();
                        let paint = overlay.paint(&setup, &leds, started.elapsed().as_secs_f32());
                        set_paint(paint.clone());
                        if !paint.is_empty() && !other_stream_running() {
                            let frame = own_frame(&setup, &leds, &paint);
                            let _ = crate::device::send_frame_raw(&kb, open, &frame);
                            streaming = true;
                        } else if streaming && paint.is_empty() {
                            streaming = false; // the keyboard goes back to its own effect
                        }
                        let keys = paint.iter().map(|&(k, c, a)| (k, mix([0, 0, 0], c, a))).collect();
                        emit(Event::Frame(keys));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            set_paint(Vec::new());
        });
        Handle { stop, setup: shared, thread: Some(thread) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::leds;

    fn key(name: &str) -> u8 {
        crate::keymap::key_by_name(name).unwrap().index
    }

    fn painted(p: &[(u8, Rgb, f32)], k: u8) -> Option<Rgb> {
        p.iter().rev().find(|(i, _, _)| *i == k).map(|(_, c, a)| mix([0, 0, 0], *c, *a))
    }

    #[test]
    fn frames_round_trip_and_wait_for_the_whole_frame() {
        let body = json!({ "cmd": "DISPATCH", "evt": "READY" });
        let bytes = encode(OP_FRAME, &body);
        assert_eq!(&bytes[..4], &[1, 0, 0, 0]);
        let mut buf = bytes[..10].to_vec();
        assert!(decode(&mut buf).is_none());
        buf.extend_from_slice(&bytes[10..]);
        buf.extend_from_slice(&encode(OP_PING, &json!({})));
        assert_eq!(decode(&mut buf), Some((OP_FRAME, body)));
        assert_eq!(decode(&mut buf).map(|f| f.0), Some(OP_PING));
        assert!(buf.is_empty());
    }

    #[test]
    fn mute_lights_its_keys_and_deafen_wins_over_it() {
        let (setup, leds) = (Setup::default(), leds());
        let mut o = Overlay::default();
        assert!(o.paint(&setup, &leds, 0.0).is_empty());
        o.dispatch("VOICE_SETTINGS_UPDATE", &json!({ "mute": true, "deaf": false }), &setup, 0.0);
        assert_eq!(painted(&o.paint(&setup, &leds, 0.0), key("f1")), Some([255, 0, 0]));
        o.dispatch("VOICE_SETTINGS_UPDATE", &json!({ "mute": true, "deaf": true }), &setup, 0.0);
        assert_eq!(painted(&o.paint(&setup, &leds, 0.0), key("f1")), Some([255, 90, 0]));
    }

    #[test]
    fn joining_plays_an_effect_and_tracks_who_speaks() {
        let (setup, leds) = (Setup::default(), leds());
        let mut o = Overlay { me: Some("me".into()), ..Overlay::default() };
        assert_eq!(o.dispatch("VOICE_CHANNEL_SELECT", &json!({ "channel_id": "42" }), &setup, 0.0), Some(Some("42".into())));
        assert!(o.voice.in_voice);
        assert!(!o.paint(&setup, &leds, 0.3).is_empty()); // the join wave
        assert!(o.paint(&setup, &leds, 5.0).is_empty()); // over, nothing else on
        o.dispatch("SPEAKING_START", &json!({ "user_id": "me" }), &setup, 5.0);
        assert_eq!(painted(&o.paint(&setup, &leds, 5.0), key("space")), Some([0, 255, 80]));
        o.dispatch("SPEAKING_START", &json!({ "user_id": "friend" }), &setup, 5.0);
        assert_eq!(o.voice.others_speaking, 1);
        o.dispatch("SPEAKING_STOP", &json!({ "user_id": "friend" }), &setup, 5.0);
        assert_eq!(o.voice.others_speaking, 0);
        // you joining doesn't count as someone joining
        o.dispatch("VOICE_STATE_CREATE", &json!({ "user": { "id": "me" } }), &setup, 6.0);
        o.dispatch("SPEAKING_STOP", &json!({ "user_id": "me" }), &setup, 6.0);
        assert!(o.paint(&setup, &leds, 6.1).is_empty());
        assert_eq!(o.dispatch("VOICE_CHANNEL_SELECT", &json!({ "channel_id": null }), &setup, 7.0), Some(None));
        assert!(!o.voice.in_voice);
    }

    #[test]
    fn a_notification_flashes_over_another_stream() {
        let (setup, leds) = (Setup::default(), leds());
        let mut o = Overlay::default();
        o.dispatch("NOTIFICATION_CREATE", &json!({ "title": "x" }), &setup, 0.0);
        let p = o.paint(&setup, &leds, 0.0);
        let base = vec![([0, 0, 255], leds.iter().map(|l| l.index).collect::<Vec<_>>())];
        let f = compose(&base, &p);
        let q = key("q");
        let c = f.iter().find(|(_, ks)| ks.contains(&q)).unwrap().0;
        assert_eq!(c, [88, 101, 242]); // full strength at the start of the flash
        assert!(o.paint(&setup, &leds, 1.0).is_empty());
    }

    #[test]
    fn own_frame_keeps_the_rest_dark_by_default() {
        let (setup, leds) = (Setup::default(), leds());
        let f = own_frame(&setup, &leds, &[(key("f1"), [255, 0, 0], 1.0)]);
        assert_eq!(f, vec![([255, 0, 0], vec![key("f1")])]);
    }

    #[test]
    fn old_settings_without_discord_load_the_defaults() {
        let s: Setup = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Setup::default());
    }
}
