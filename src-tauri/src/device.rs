//! Talking to the F75 over its configuration channel: packets, retries and acks.
//! Ported from aula-f75-hub (`aula/device.py`, MIT). How the channel is found and opened
//! lives in `win.rs`.

use crate::protocol::{self as p, Packet};
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct DeviceError(pub String);

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeviceError {}

impl From<p::ProtocolError> for DeviceError {
    fn from(e: p::ProtocolError) -> Self {
        DeviceError(e.0)
    }
}

pub type Result<T> = std::result::Result<T, DeviceError>;

pub fn derr<T>(msg: impl Into<String>) -> Result<T> {
    Err(DeviceError(msg.into()))
}

const ASLEEP: &str = "The keyboard didn't answer. It may be asleep: press any key to wake it, then try again.";

/// The open configuration channel: raw 20-byte reports in and out.
pub trait Channel: Send {
    fn write(&mut self, pkt: &Packet) -> Result<()>;
    /// One incoming report, or None if nothing came within `timeout`.
    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>>;
    fn drain(&mut self) -> Result<()> {
        while self.recv(Duration::from_millis(20))?.is_some() {}
        Ok(())
    }
}

/// The wired configuration channel: 520-byte feature reports (see `protocol.rs`).
pub trait FeatureChannel: Send {
    fn set(&mut self, report: &p::WiredReport) -> Result<()>;
    fn get(&mut self, report: &mut p::WiredReport) -> Result<()>;
}

const NOT_ON_CABLE: &str = "This isn't available over the USB cable yet. Use the 2.4G dongle or Bluetooth for it.";

/// Stands in for the packet channel in wired mode, where only the table reads work so far.
struct NoPackets;

impl Channel for NoPackets {
    fn write(&mut self, _: &Packet) -> Result<()> {
        derr(NOT_ON_CABLE)
    }
    fn recv(&mut self, _: Duration) -> Result<Option<Vec<u8>>> {
        derr(NOT_ON_CABLE)
    }
}

/// Knows whether a key is held. While the keyboard answers configuration traffic it can
/// drop the release report of a key pressed at that moment, and the OS then auto-repeats
/// that key, so the keyboard is only talked to while no key is held.
pub trait KeyGuard: Send {
    fn wait_released(&mut self, quiet: Duration, limit: Duration) -> Result<()>;
}

/// The connection the app looks for, chosen in the app and saved with its settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Connection {
    /// Whichever one is there (the cable first).
    #[default]
    Auto,
    Cable,
    Dongle,
    Bluetooth,
}

impl Connection {
    /// The mode it allows, or None for any.
    pub fn mode(self) -> Option<Mode> {
        match self {
            Connection::Auto => None,
            Connection::Cable => Some(Mode::Wired),
            Connection::Dongle => Some(Mode::Dongle),
            Connection::Bluetooth => Some(Mode::Bluetooth),
        }
    }
}

// A plain static, so `open` keeps working as a fn pointer (the animations and audio use it).
static CONNECTION: AtomicU8 = AtomicU8::new(0);

pub fn set_connection(c: Connection) {
    CONNECTION.store(c as u8, Ordering::Relaxed);
}

pub fn connection() -> Connection {
    match CONNECTION.load(Ordering::Relaxed) {
        1 => Connection::Cable,
        2 => Connection::Dongle,
        3 => Connection::Bluetooth,
        _ => Connection::Auto,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Mode {
    #[serde(rename = "2.4G dongle")]
    Dongle,
    #[serde(rename = "Bluetooth")]
    Bluetooth,
    #[serde(rename = "USB cable")]
    Wired,
}

pub struct Keyboard {
    pub mode: Mode,
    ch: Box<dyn Channel>,
    feature: Option<Box<dyn FeatureChannel>>,
    guard: Box<dyn KeyGuard>,
    timeout: Duration,
    last_write: Option<Instant>,
}

impl Keyboard {
    pub fn new(mode: Mode, ch: Box<dyn Channel>, guard: Box<dyn KeyGuard>) -> Self {
        Keyboard { mode, ch, feature: None, guard, timeout: Duration::from_secs(1), last_write: None }
    }

    pub fn wired(feature: Box<dyn FeatureChannel>, guard: Box<dyn KeyGuard>) -> Self {
        Keyboard { feature: Some(feature), ..Keyboard::new(Mode::Wired, Box::new(NoPackets), guard) }
    }

    fn bluetooth(&self) -> bool {
        self.mode == Mode::Bluetooth
    }

    fn wait_keys_released(&mut self) -> Result<()> {
        self.guard.wait_released(Duration::from_millis(400), Duration::from_secs(5))
    }

    /// Right after a write the keyboard is busy saving. On Windows over the dongle a read or
    /// write sent straight after one went unanswered (tested 2026-10-20), so leave a pause.
    fn settle(&self, after_write: Duration) {
        if let Some(t) = self.last_write {
            let since = t.elapsed();
            if since < after_write {
                sleep(after_write - since);
            }
        }
    }

    /// Quick single read: does the keyboard answer on this channel?
    pub fn probe(&mut self) -> Result<bool> {
        if self.feature.is_some() {
            return Ok(self.wired_read(p::WIRED_READ_LED, 0, p::LED_LEN).is_ok());
        }
        self.wait_keys_released()?;
        self.ch.drain()?;
        self.ch.write(&p::read_request(p::CMD_PROFILE, 0))?;
        let deadline = Instant::now() + Duration::from_millis(600);
        while Instant::now() < deadline {
            if let Some(r) = self.ch.recv(Duration::from_millis(100))? {
                if p::parse_packet(&r).is_some_and(|rep| rep.cmd == p::CMD_PROFILE | p::READ) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Send a read command and reassemble the multi-packet reply by index. Packets get lost
    /// (over Bluetooth roughly every other one) and the keyboard always resends the whole
    /// reply, so the request is repeated and the parts are collected across attempts.
    pub fn read(&mut self, cmd: u8, sub: u8) -> Result<Vec<u8>> {
        let rcmd = cmd | p::READ;
        // Over Bluetooth LE only 4-7 packets of a reply get through per attempt, at random, so
        // a 37-packet read took 14-33 attempts (9-28 s, tested on Windows 2026-10-21). Keep
        // asking while new packets still come in; stalls of 15 attempts on one packet happened.
        let retries = if self.bluetooth() { 60 } else { 5 };
        let max_stall = if self.bluetooth() { 25 } else { retries };
        let mut stall = 0;
        let mut parts: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
        let mut total: Option<u8> = None;
        self.settle(Duration::from_millis(500));
        for attempt in 0..retries {
            if stall >= max_stall {
                break;
            }
            let before = parts.len();
            self.wait_keys_released()?;
            self.ch.drain()?;
            if self.bluetooth() {
                // Which packets get lost depends on the burst's phase against the radio's
                // slots, and a pause alone doesn't move it (the request lands on a slot too):
                // tested on Windows 2026-10-21, a plain retry lost the same packets every time.
                // A few small requests in front do shift it, so vary how many.
                for _ in 0..attempt % 4 {
                    self.ch.write(&p::read_request(p::CMD_PROFILE, 0))?;
                }
                sleep(Duration::from_millis(rand::rng().random_range(0..50)));
            }
            self.ch.write(&p::read_request(cmd, sub))?;
            let deadline = Instant::now() + 2 * self.timeout;
            let mut last: Option<Instant> = None;
            while Instant::now() < deadline {
                if last.is_some_and(|l| l.elapsed() > Duration::from_millis(250)) {
                    break; // this burst is over
                }
                let Some(r) = self.ch.recv(Duration::from_millis(50))? else { continue };
                let Some(rep) = p::parse_packet(&r) else { continue };
                if rep.cmd != rcmd {
                    continue;
                }
                if rep.error {
                    return derr(format!("The keyboard rejected read 0x{rcmd:02x}."));
                }
                last = Some(Instant::now());
                total = Some(rep.total);
                if rep.index < rep.total {
                    parts.insert(rep.index, rep.data);
                }
                if parts.len() == rep.total as usize {
                    return Ok(parts.into_values().flatten().collect());
                }
            }
            stall = if parts.len() > before { 0 } else { stall + 1 };
            // a short pause before asking again helps the keyboard catch up
            sleep(Duration::from_millis(150));
        }
        match total {
            None => derr(ASLEEP),
            Some(t) => {
                let missing: Vec<u8> = (0..t).filter(|i| !parts.contains_key(i)).collect();
                derr(format!("Read 0x{rcmd:02x} incomplete (packets {missing:?} never arrived). Try again."))
            }
        }
    }

    /// Read a table over the cable: the request, then its reply (a few tries, as the
    /// official app does).
    fn wired_read(&mut self, cmd: u8, sub: u8, len: usize) -> Result<Vec<u8>> {
        self.wait_keys_released()?;
        let Some(f) = self.feature.as_mut() else { return derr(NOT_ON_CABLE) };
        for _ in 0..3 {
            f.set(&p::wired_read_request(cmd, sub))?;
            sleep(Duration::from_millis(10));
            let mut r = [0u8; p::WIRED_REPORT_LEN];
            r[0] = p::WIRED_REPORT_ID;
            f.get(&mut r)?;
            if let Some(d) = p::wired_reply(&r, cmd, sub, len) {
                return Ok(d);
            }
            sleep(Duration::from_millis(100));
        }
        derr(format!("Read 0x{cmd:02x} over the cable got no matching reply. Try again."))
    }

    /// Write a table over the cable, then read it back to check the keyboard kept it.
    fn wired_write(&mut self, cmd: u8, read_cmd: u8, sub: u8, data: &[u8]) -> Result<()> {
        self.wait_keys_released()?;
        let Some(f) = self.feature.as_mut() else { return derr(NOT_ON_CABLE) };
        let mut r = p::wired_request(cmd, sub, data.len());
        r[p::WIRED_HEADER..p::WIRED_HEADER + data.len()].copy_from_slice(data);
        f.set(&r)?;
        if self.wired_read(read_cmd, sub, data.len())? != data {
            return derr("The keyboard didn't keep the change. Try again.");
        }
        Ok(())
    }

    /// Write data in 14-byte packets; the keyboard echoes each one back as an ack.
    pub fn write(&mut self, cmd: u8, sub: u8, data: &[u8]) -> Result<()> {
        self.settle(Duration::from_millis(300));
        self.wait_keys_released()?;
        self.ch.drain()?;
        // over Bluetooth acks get lost on the way back too, so a packet is resent more often
        let retries = if self.bluetooth() { 8 } else { 5 };
        for pkt in p::split_packets(cmd, sub, data) {
            let mut acked = false;
            for attempt in 0..retries {
                if attempt > 0 {
                    sleep(Duration::from_millis(100 * attempt));
                    self.ch.drain()?;
                }
                self.ch.write(&pkt)?;
                if self.wait_ack(&pkt)? {
                    acked = true;
                    break;
                }
            }
            if !acked {
                return derr(format!("Write 0x{cmd:02x} not acknowledged (packet {}). {ASLEEP}", pkt[3]));
            }
            sleep(Duration::from_millis(15));
        }
        self.last_write = Some(Instant::now());
        Ok(())
    }

    fn wait_ack(&mut self, pkt: &Packet) -> Result<bool> {
        let deadline = Instant::now() + self.timeout;
        while Instant::now() < deadline {
            let Some(r) = self.ch.recv(Duration::from_millis(300))? else { continue };
            let Some(rep) = p::parse_packet(&r) else { continue };
            if rep.cmd != pkt[1] || rep.index != pkt[3] {
                continue;
            }
            if rep.error {
                return derr(format!("The keyboard rejected command 0x{:02x}.", pkt[1]));
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Show a real-time frame (colour groups) until the next one arrives. Nothing is stored or
    /// acknowledged, so a lost packet loses the frame; the gap between packets keeps that rare.
    /// Without a new frame the keyboard goes back to its own effect after about 2 seconds.
    /// Typing keeps working while frames stream, so the key guard is skipped (tested upstream).
    /// Returns what was shown: over 2.4G and Bluetooth a frame with too many colours is
    /// merged down to fit (`anim::fit`); the cable shows every key's own colour.
    pub fn send_live(&mut self, groups: &[(p::Rgb, Vec<u8>)]) -> Result<Vec<(p::Rgb, Vec<u8>)>> {
        if let Some(f) = self.feature.as_mut() {
            f.set(&p::wired_live_frame(groups))?; // one report, every key's own colour
            return Ok(groups.to_vec());
        }
        let groups = crate::anim::fit(groups.to_vec());
        let frame = p::live_frame(&groups)?;
        if frame.len() > p::LIVE_BUDGET {
            return derr(format!("frame too long ({} bytes)", frame.len()));
        }
        let gap = if self.bluetooth() { p::LIVE_GAP_MS_BT } else { p::LIVE_GAP_MS };
        for pkt in p::split_packets(p::CMD_LIVE, p::LIVE_KEYS, &frame) {
            self.ch.write(&pkt)?;
            sleep(Duration::from_millis(gap));
        }
        self.ch.drain()?;
        Ok(groups)
    }

    // ------------------------------------------------------------ tables

    pub fn get_led(&mut self) -> Result<p::LedConfig> {
        if self.feature.is_some() {
            return Ok(p::LedConfig::parse(&self.wired_read(p::WIRED_READ_LED, 0, p::LED_LEN)?)?);
        }
        Ok(p::LedConfig::parse(&self.read(p::CMD_SET_LED, 0)?)?)
    }

    pub fn set_led(&mut self, led: &p::LedConfig) -> Result<()> {
        if self.feature.is_some() {
            return self.wired_write(p::WIRED_WRITE_LED, p::WIRED_READ_LED, 0, &led.0);
        }
        self.write(p::CMD_SET_LED, 0, &led.0)
    }

    pub fn get_custom(&mut self) -> Result<Vec<u8>> {
        self.read_sized(p::CMD_SET_CUSTOM, 0, p::CUSTOM_LEN)
    }

    pub fn set_custom(&mut self, raw: &[u8]) -> Result<()> {
        check_len(raw, p::CUSTOM_LEN, "per-key colour table")?;
        self.write(p::CMD_SET_CUSTOM, 0, raw)
    }

    pub fn get_matrix(&mut self, layer: u8) -> Result<Vec<u8>> {
        if self.feature.is_some() {
            return self.wired_read(p::WIRED_READ_MATRIX, layer, p::MATRIX_BYTES);
        }
        self.read_sized(p::CMD_SET_MATRIX, layer, p::MATRIX_BYTES)
    }

    pub fn set_matrix(&mut self, raw: &[u8], layer: u8) -> Result<()> {
        check_len(raw, p::MATRIX_BYTES, "keymap")?;
        self.write(p::CMD_SET_MATRIX, layer, raw)
    }

    pub fn get_palette(&mut self) -> Result<Vec<u8>> {
        if self.feature.is_some() {
            return self.wired_read(p::WIRED_READ_PALETTE, 0, p::PALETTE_LEN);
        }
        self.read_sized(p::CMD_SET_PALETTE, 0, p::PALETTE_LEN)
    }

    pub fn set_palette(&mut self, raw: &[u8]) -> Result<()> {
        check_len(raw, p::PALETTE_LEN, "effect palettes")?;
        self.write(p::CMD_SET_PALETTE, 0, raw)
    }

    pub fn get_macro_store(&mut self) -> Result<Vec<u8>> {
        self.read_sized(p::CMD_SET_MACRO, 0, p::MACRO_STORE_LEN)
    }

    pub fn set_macro_store(&mut self, raw: &[u8]) -> Result<()> {
        check_len(raw, p::MACRO_STORE_LEN, "macro store")?;
        self.write(p::CMD_SET_MACRO, 0, raw)
    }

    /// Battery percentage (experimental: first byte of the 0x4a reply).
    pub fn get_battery(&mut self) -> Result<Option<u8>> {
        if self.mode == Mode::Wired {
            return Ok(None); // the cable has no battery report
        }
        let d = self.read(p::CMD_PROFILE, 0)?;
        Ok(d.first().copied().filter(|&b| b <= 100))
    }

    fn read_sized(&mut self, cmd: u8, sub: u8, len: usize) -> Result<Vec<u8>> {
        let d = self.read(cmd, sub)?;
        if d.len() != len {
            return derr(format!("Read 0x{:02x} returned {} bytes, expected {len}.", cmd | p::READ, d.len()));
        }
        Ok(d)
    }
}

/// Stream one real-time frame through the shared keyboard, opening it first if needed. On an
/// error the keyboard is closed, so the next frame opens it again (it may have been switched
/// off or unplugged). Used by the audio lighting and the animations.
pub fn send_frame(
    kb: &std::sync::Mutex<Option<Keyboard>>,
    open: fn() -> Result<Keyboard>,
    groups: &[(p::Rgb, Vec<u8>)],
) -> std::result::Result<Vec<(p::Rgb, Vec<u8>)>, String> {
    let mut guard = kb.lock().map_err(|_| "keyboard lock poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(open().map_err(|e| e.0)?);
    }
    let r = guard.as_mut().unwrap().send_live(groups);
    if r.is_err() {
        *guard = None;
    }
    r.map_err(|e| e.0)
}

fn check_len(raw: &[u8], len: usize, what: &str) -> Result<()> {
    if raw.len() != len {
        return derr(format!("{what} must be {len} bytes"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A fake keyboard: echoes writes as acks and answers reads from a table.
    struct Fake {
        tables: BTreeMap<(u8, u8), Vec<u8>>,
        inbox: VecDeque<Vec<u8>>,
        written: Arc<Mutex<Vec<Packet>>>,
        drop_first_ack: bool,
    }

    impl Channel for Fake {
        fn write(&mut self, pkt: &Packet) -> Result<()> {
            self.written.lock().unwrap().push(*pkt);
            if pkt[1] & p::READ != 0 {
                let data = self.tables.get(&(pkt[1] & !p::READ, pkt[4] >> 4)).cloned().unwrap_or_default();
                for r in p::split_packets(pkt[1], pkt[4] >> 4, &data) {
                    self.inbox.push_back(r.to_vec());
                }
            } else if self.drop_first_ack {
                self.drop_first_ack = false;
            } else {
                self.inbox.push_back(pkt.to_vec());
            }
            Ok(())
        }
        fn recv(&mut self, _: Duration) -> Result<Option<Vec<u8>>> {
            Ok(self.inbox.pop_front())
        }
        fn drain(&mut self) -> Result<()> {
            Ok(())
        }
    }

    struct NoGuard;
    impl KeyGuard for NoGuard {
        fn wait_released(&mut self, _: Duration, _: Duration) -> Result<()> {
            Ok(())
        }
    }

    fn fake(drop_first_ack: bool) -> (Keyboard, Arc<Mutex<Vec<Packet>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));
        let mut tables = BTreeMap::new();
        tables.insert((p::CMD_SET_LED, 0), p::factory::led());
        tables.insert((p::CMD_PROFILE, 0), vec![0x64, 0x01]);
        let ch = Fake { tables, inbox: VecDeque::new(), written: written.clone(), drop_first_ack };
        (Keyboard::new(Mode::Dongle, Box::new(ch), Box::new(NoGuard)), written)
    }

    #[test]
    fn reads_reassemble_multi_packet_replies() {
        let (mut kb, _) = fake(false);
        assert_eq!(kb.get_led().unwrap().0.to_vec(), p::factory::led());
        assert_eq!(kb.get_battery().unwrap(), Some(100));
    }

    #[test]
    fn writes_retry_an_unacknowledged_packet() {
        let (mut kb, written) = fake(true);
        kb.set_led(&p::LedConfig::parse(&p::factory::led()).unwrap()).unwrap();
        let w = written.lock().unwrap();
        assert_eq!(w.len(), 11); // packet 0 sent twice, then 1..9
        assert_eq!(w[0], w[1]);
    }

    /// A fake wired keyboard: answers a read request with the table, on the next get.
    struct FakeWired {
        tables: BTreeMap<(u8, u8), Vec<u8>>,
        reply: Option<p::WiredReport>,
    }

    impl FeatureChannel for FakeWired {
        fn set(&mut self, report: &p::WiredReport) -> Result<()> {
            if report[1] == p::WIRED_WRITE_LED {
                let len = u16::from_le_bytes([report[6], report[7]]) as usize;
                self.tables.insert((p::WIRED_READ_LED, 0), report[p::WIRED_HEADER..p::WIRED_HEADER + len].to_vec());
                return Ok(());
            }
            let mut r = *report;
            let data = self.tables.get(&(report[1], report[2])).cloned().unwrap_or_default();
            r[p::WIRED_HEADER..p::WIRED_HEADER + data.len()].copy_from_slice(&data);
            self.reply = Some(r);
            Ok(())
        }
        fn get(&mut self, report: &mut p::WiredReport) -> Result<()> {
            *report = self.reply.take().unwrap_or([0; p::WIRED_REPORT_LEN]);
            Ok(())
        }
    }

    #[test]
    fn wired_reads_tables_and_refuses_the_rest() {
        let mut tables = BTreeMap::new();
        tables.insert((p::WIRED_READ_LED, 0), p::factory::led());
        tables.insert((p::WIRED_READ_PALETTE, 0), p::factory::palette());
        tables.insert((p::WIRED_READ_MATRIX, 1), p::factory::matrix(1));
        let mut kb = Keyboard::wired(Box::new(FakeWired { tables, reply: None }), Box::new(NoGuard));
        assert!(kb.probe().unwrap());
        assert_eq!(kb.get_led().unwrap().0.to_vec(), p::factory::led());
        assert_eq!(kb.get_palette().unwrap(), p::factory::palette());
        assert_eq!(kb.get_matrix(1).unwrap(), p::factory::matrix(1));
        assert_eq!(kb.get_battery().unwrap(), None);
        assert!(kb.get_custom().unwrap_err().0.contains("USB cable"));
        assert!(kb.set_custom(&p::factory::custom()).is_err());
    }

    #[test]
    fn wired_lighting_writes_are_read_back() {
        let mut tables = BTreeMap::new();
        tables.insert((p::WIRED_READ_LED, 0), p::factory::led());
        let mut kb = Keyboard::wired(Box::new(FakeWired { tables, reply: None }), Box::new(NoGuard));
        let mut led = kb.get_led().unwrap();
        led.set_effect(2);
        kb.set_led(&led).unwrap();
        assert_eq!(kb.get_led().unwrap().effect(), 2);
    }
}
