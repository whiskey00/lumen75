//! Talking to an S75 Pro: the command sequences open-aula describes, over USB feature reports
//! or the dongle's 32-byte frames. How the link is opened lives in `winlink.rs`.

use super::protocol::{self as p, op, radio, Lighting, Report, Settings, Step, Time, BLOCK, RADIO, REPORT};
use crate::device::{derr, Result};
use std::thread::sleep;
use std::time::Duration;

/// How the app reaches the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Connection {
    #[serde(rename = "USB cable")]
    Usb,
    #[serde(rename = "2.4G dongle")]
    Dongle,
}

/// The open connection: USB feature reports (plus the screen's output report), or the
/// dongle's frames.
pub trait Link: Send {
    fn connection(&self) -> Connection;
    fn set_feature(&mut self, r: &Report) -> Result<()>;
    fn get_feature(&mut self) -> Result<Report>;
    /// Send a dongle frame and wait up to 300 ms for its echo (`cmd len seq`).
    fn radio(&mut self, frame: &[u8; RADIO]) -> Result<Option<[u8; RADIO]>>;
    /// Send one 4096-byte screen chunk and wait up to 300 ms for the keyboard to take it.
    fn screen(&mut self, chunk: &[u8]) -> Result<()>;
}

const NO_ANSWER: &str = "The S75 Pro didn't answer. It may be asleep: press a key to wake it, then try again.";
const USB_ONLY: &str = "This needs the S75 Pro on the USB cable.";

pub struct S75 {
    link: Box<dyn Link>,
    /// The vendor driver waits this long around every report.
    delay: Duration,
}

impl S75 {
    pub fn new(link: Box<dyn Link>) -> Self {
        S75 { link, delay: Duration::from_millis(35) }
    }

    pub fn connection(&self) -> Connection {
        self.link.connection()
    }

    fn usb(&self) -> bool {
        self.connection() == Connection::Usb
    }

    /// A control command, read back as its acknowledgement when `readback` (repeated once if
    /// the keyboard answers `ff`, as the driver does).
    fn cmd(&mut self, opcode: u8, b2: u8, count: u16, readback: bool) -> Result<()> {
        let c = p::command(opcode, b2, count);
        for attempt in 0..2 {
            sleep(self.delay);
            self.link.set_feature(&c)?;
            if !readback {
                return Ok(());
            }
            sleep(self.delay);
            let r = self.link.get_feature()?;
            if !(r[1] == opcode && r[3] == 0xff) || attempt == 1 {
                return Ok(());
            }
        }
        Ok(())
    }

    fn data(&mut self, payload: &Report) -> Result<()> {
        sleep(self.delay);
        self.link.set_feature(payload)
    }

    /// Data packets, each retried a few times as the driver does.
    fn block(&mut self, data: &[u8]) -> Result<()> {
        sleep(self.delay);
        for chunk in data.chunks(REPORT) {
            let mut r = [0u8; REPORT];
            r[..chunk.len()].copy_from_slice(chunk);
            let mut tries = 0;
            while let Err(e) = self.link.set_feature(&r) {
                tries += 1;
                if tries > 10 {
                    return Err(e);
                }
                sleep(self.delay);
            }
            sleep(self.delay);
        }
        Ok(())
    }

    /// Frames over the dongle. A frame without an echo isn't fatal (the driver goes on too),
    /// but none at all means the keyboard isn't listening.
    fn radio_all(&mut self, frames: &[[u8; RADIO]]) -> Result<()> {
        let mut answered = 0;
        for f in frames {
            if self.link.radio(f)?.is_some() {
                answered += 1;
            }
            sleep(Duration::from_millis(2));
        }
        if answered == 0 {
            return derr(NO_ANSWER);
        }
        Ok(())
    }

    /// One lighting payload: begin, the command, the data, save, apply.
    fn light_write(&mut self, payload: &Report) -> Result<()> {
        self.cmd(op::BEGIN, 0, 0, true)?;
        self.cmd(op::LIGHT, 0, 1, true)?;
        self.data(payload)?;
        self.cmd(op::END, 0, 0, true)?;
        self.cmd(op::APPLY, 0, 0, false)
    }

    pub fn set_lighting(&mut self, l: &Lighting) -> Result<()> {
        let payload = p::light_payload(l);
        if !self.usb() {
            return self.radio_all(&[p::radio_short(radio::LIGHT, &payload)]);
        }
        self.light_write(&payload)
    }

    /// The side light bar (see `protocol::BAR_MODES`).
    pub fn set_bar(&mut self, mode: u8) -> Result<()> {
        if !self.usb() {
            let mut d = [0u8; 28];
            d[0] = mode;
            d[14] = 0xaa;
            d[15] = 0x55;
            return self.radio_all(&[p::radio_frame(radio::LIGHT, 1, 0, &d)]);
        }
        self.light_write(&p::bar_payload(mode))
    }

    /// Per-key colours (by light index): switches to the per-key mode, then sends the table.
    pub fn set_perkey(&mut self, colors: &[(u8, [u8; 3])], brightness: u8) -> Result<()> {
        let mode = p::light_payload(&Lighting { mode: p::USER_LIGHT_MODE, rgb: [0, 0, 0], colorful: false, brightness, speed: 0, direction: 0 });
        let table = p::perkey_block(colors);
        if !self.usb() {
            self.radio_all(&[p::radio_short(radio::LIGHT, &mode)])?;
            return self.radio_all(&p::radio_block(radio::USER_LIGHT, &table));
        }
        self.light_write(&mode)?;
        self.cmd(op::BEGIN, 0, 0, true)?;
        self.cmd(op::USER_LIGHT, 0, (BLOCK / REPORT) as u16, true)?;
        self.block(&table)?;
        self.cmd(op::END, 0, 0, true)?;
        self.cmd(op::APPLY, 0, 0, true)
    }

    /// The per-key colours stored on the keyboard (USB only).
    pub fn read_perkey(&mut self, indexes: &[u8]) -> Result<Vec<(u8, [u8; 3])>> {
        if !self.usb() {
            return derr(USB_ONLY);
        }
        let n = BLOCK / REPORT;
        self.cmd(op::USER_LIGHT_READ, 0, n as u16, true)?;
        let mut buf = vec![0u8; BLOCK];
        for i in 0..n {
            sleep(Duration::from_millis(3));
            buf[i * REPORT..(i + 1) * REPORT].copy_from_slice(&self.link.get_feature()?);
        }
        self.cmd(op::END, 0, 0, true)?;
        Ok(p::decode_perkey(&buf, indexes))
    }

    pub fn set_settings(&mut self, s: &Settings) -> Result<()> {
        let payload = p::settings_payload(s);
        if !self.usb() {
            return self.radio_all(&[p::radio_short(radio::SETTINGS, &payload)]);
        }
        self.cmd(op::BEGIN, 0, 0, true)?;
        self.cmd(op::SETTINGS, 1, 1, true)?;
        self.data(&payload)?;
        self.cmd(op::END, 0, 0, true)
    }

    /// A whole keymap layer (base or Fn), by key index.
    pub fn set_keymap(&mut self, fn_layer: bool, entries: &[(u8, p::Entry)]) -> Result<()> {
        let table = p::keymap_block(entries);
        if !self.usb() {
            return self.radio_all(&p::radio_block(if fn_layer { radio::KEYMAP_FN } else { radio::KEYMAP }, &table));
        }
        self.cmd(op::BEGIN, 0, 0, true)?;
        self.cmd(if fn_layer { op::KEYMAP_FN } else { op::KEYMAP }, 0, (BLOCK / REPORT) as u16, true)?;
        self.block(&table)?;
        self.cmd(op::END, 0, 0, true)?;
        self.cmd(op::APPLY, 0, 0, true)
    }

    /// All macros at once (the keyboard stores them as one block).
    pub fn set_macros(&mut self, macros: &[Vec<Step>]) -> Result<()> {
        if !self.usb() {
            let frames = p::radio_macros(macros).map_err(crate::device::DeviceError)?;
            return self.radio_all(&frames);
        }
        let (data, packets) = p::macro_block(macros).map_err(crate::device::DeviceError)?;
        self.cmd(op::MACRO_BEGIN, 0, 0, true)?;
        self.cmd(op::MACRO, 0, packets, true)?;
        sleep(Duration::from_millis(30));
        self.block(&data)?;
        sleep(Duration::from_millis(30));
        self.cmd(op::END, 0, 0, true)
    }

    pub fn set_time(&mut self, t: &Time) -> Result<()> {
        let payload = p::time_payload(t, 1);
        if !self.usb() {
            return self.radio_all(&[p::radio_short(radio::TIME, &payload)]);
        }
        self.cmd(op::BEGIN, 0, 0, true)?;
        self.cmd(op::TIME, 0, 1, true)?;
        self.data(&payload)?;
        sleep(self.delay);
        let _ = self.link.get_feature()?;
        self.cmd(op::END, 0, 0, true)
    }

    /// Upload a screen image (`protocol::screen_image`) to a slot, over USB only. `progress`
    /// gets the share done (0..1) after each chunk.
    pub fn upload_screen(&mut self, image: &[u8], slot: u8, mut progress: impl FnMut(f32)) -> Result<()> {
        if !self.usb() {
            return derr("The screen can only be changed over the USB cable.");
        }
        let chunks = image.len() / p::SCREEN_CHUNK;
        self.cmd(op::BEGIN, 0, 0, true)?;
        self.cmd(op::SCREEN, slot, chunks as u16, true)?;
        for (i, chunk) in image.chunks(p::SCREEN_CHUNK).enumerate() {
            self.link.screen(chunk)?;
            progress((i + 1) as f32 / chunks as f32);
        }
        self.cmd(op::END, 0, 0, true)
    }

    /// Battery percentage, over the dongle only (None on USB or without an answer).
    pub fn battery(&mut self) -> Result<Option<u8>> {
        if self.usb() {
            return Ok(None);
        }
        let r = self.link.radio(&p::radio_frame(radio::BATTERY, 1, 0, &[]))?;
        Ok(r.filter(|r| r[0] == radio::BATTERY && r[3] > 0).map(|r| r[3].min(100)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Records what's sent; every feature read answers the last command, every frame is echoed.
    struct Fake {
        conn: Connection,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        last: Report,
    }

    impl Link for Fake {
        fn connection(&self) -> Connection {
            self.conn
        }
        fn set_feature(&mut self, r: &Report) -> Result<()> {
            self.sent.lock().unwrap().push(r.to_vec());
            self.last = *r;
            Ok(())
        }
        fn get_feature(&mut self) -> Result<Report> {
            Ok(self.last)
        }
        fn radio(&mut self, f: &[u8; RADIO]) -> Result<Option<[u8; RADIO]>> {
            self.sent.lock().unwrap().push(f.to_vec());
            let mut echo = *f;
            if f[0] == radio::BATTERY {
                echo[3] = 87;
            }
            Ok(Some(echo))
        }
        fn screen(&mut self, chunk: &[u8]) -> Result<()> {
            self.sent.lock().unwrap().push(chunk.to_vec());
            Ok(())
        }
    }

    fn fake(conn: Connection) -> (S75, Arc<Mutex<Vec<Vec<u8>>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut kb = S75::new(Box::new(Fake { conn, sent: sent.clone(), last: [0; REPORT] }));
        kb.delay = Duration::ZERO;
        (kb, sent)
    }

    /// The opcodes of the command reports sent (data packets left out).
    fn ops(sent: &[Vec<u8>]) -> Vec<u8> {
        sent.iter().filter(|r| r[0] == 0x04 && r[62] != 0xaa).map(|r| r[1]).collect()
    }

    #[test]
    fn usb_writes_follow_begin_command_data_end_apply() {
        let (mut kb, sent) = fake(Connection::Usb);
        kb.set_lighting(&Lighting::default()).unwrap();
        let s = sent.lock().unwrap();
        assert_eq!(ops(&s), [op::BEGIN, op::LIGHT, op::END, op::APPLY]);
        assert_eq!(s[2][62..], [0xaa, 0x55]); // the payload after the command
        drop(s);
        sent.lock().unwrap().clear();
        kb.set_keymap(true, &[(38, p::DISABLED)]).unwrap();
        let s = sent.lock().unwrap();
        assert_eq!(ops(&s), [op::BEGIN, op::KEYMAP_FN, op::END, op::APPLY]);
        assert_eq!(s.len(), 4 + 9); // 9 data packets
    }

    #[test]
    fn dongle_writes_are_frames_with_echoes() {
        let (mut kb, sent) = fake(Connection::Dongle);
        kb.set_settings(&Settings::default()).unwrap();
        kb.set_keymap(false, &[]).unwrap();
        let s = sent.lock().unwrap();
        assert_eq!(s.len(), 1 + 21);
        assert_eq!(s[0][0], radio::SETTINGS);
        assert!(s[1..].iter().all(|f| f[0] == radio::KEYMAP));
        drop(s);
        assert_eq!(kb.battery().unwrap(), Some(87));
        assert!(kb.read_perkey(&[38]).is_err()); // USB only
        assert!(kb.upload_screen(&[0; 4096], 1, |_| {}).is_err());
    }

    #[test]
    fn screen_upload_sends_every_chunk() {
        let (mut kb, sent) = fake(Connection::Usb);
        let mut done = 0.0;
        kb.upload_screen(&vec![0u8; 3 * 4096], 1, |f| done = f).unwrap();
        let s = sent.lock().unwrap();
        assert_eq!(s.iter().filter(|c| c.len() == 4096).count(), 3);
        assert_eq!(done, 1.0);
        assert_eq!(kb_battery_on_usb(), None);
    }

    fn kb_battery_on_usb() -> Option<u8> {
        fake(Connection::Usb).0.battery().unwrap()
    }
}
