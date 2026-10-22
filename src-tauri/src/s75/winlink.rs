//! Finding and opening an S75 Pro on Windows: its USB configuration collection (64-byte
//! feature report, no report ID), the screen's 4096-byte output report, and the dongle's
//! 32-byte reports.

use super::device::{Connection, Link, S75};
use super::protocol::{Report, RADIO, REPORT};
use crate::device::{derr, Channel, DeviceError, Result};
use crate::win::{caps, interface_paths, open, WinChannel};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::time::{Duration, Instant};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{HidD_GetAttributes, HidD_GetFeature, HidD_SetFeature, HIDD_ATTRIBUTES};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, GENERIC_READ, GENERIC_WRITE, HANDLE};

pub const USB: (u16, u16) = (0x0c45, 0x800a);
/// The dongle reports Apple's vendor ID.
pub const DONGLE: (u16, u16) = (0x05ac, 0x024f);

/// Where an S75 Pro's collections are.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Found {
    pub usb: Option<String>,
    pub screen: Option<String>,
    pub dongle: Option<String>,
}

impl Found {
    pub fn any(&self) -> bool {
        self.usb.is_some() || self.dongle.is_some()
    }
}

/// The S75 Pro's collections, by their IDs and report sizes (lengths include the report ID
/// byte Windows adds).
pub fn find() -> Found {
    let mut f = Found::default();
    for path in interface_paths() {
        let lower = path.to_lowercase();
        if !lower.contains("vid_0c45&pid_800a") && !lower.contains("vid_05ac&pid_024f") {
            continue;
        }
        let Some(h) = open(&path, 0, 0) else { continue };
        unsafe {
            let mut a: HIDD_ATTRIBUTES = zeroed();
            a.Size = size_of::<HIDD_ATTRIBUTES>() as u32;
            if HidD_GetAttributes(h, &mut a) != 0 {
                if let Some(c) = caps(h) {
                    let id = (a.VendorID, a.ProductID);
                    if id == USB && c.FeatureReportByteLength as usize == REPORT + 1 && f.usb.is_none() {
                        f.usb = Some(path.clone());
                    } else if id == USB && c.OutputReportByteLength as usize > 4096 && f.screen.is_none() {
                        f.screen = Some(path.clone());
                    } else if id == DONGLE
                        && c.OutputReportByteLength as usize == RADIO + 1
                        && c.InputReportByteLength as usize == RADIO + 1
                        && f.dongle.is_none()
                    {
                        f.dongle = Some(path.clone());
                    }
                }
            }
            CloseHandle(h);
        }
    }
    f
}

fn open_rw(path: &str) -> Result<HANDLE> {
    match open(path, GENERIC_READ | GENERIC_WRITE, 0) {
        Some(h) => Ok(h),
        None if unsafe { GetLastError() } == ERROR_ACCESS_DENIED => {
            derr("Can't open the S75 Pro: access denied. Close the official AULA driver, then try again.")
        }
        None => derr(format!("Can't open the S75 Pro (Windows error {}).", unsafe { GetLastError() })),
    }
}

/// USB: synchronous feature reports on the configuration collection; the screen collection
/// (if found) for uploads.
struct UsbLink {
    h: HANDLE,
    screen: Option<WinChannel>,
}

// used from one thread at a time, behind the app's mutex
unsafe impl Send for UsbLink {}

impl Drop for UsbLink {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.h) };
    }
}

impl Link for UsbLink {
    fn connection(&self) -> Connection {
        Connection::Usb
    }

    fn set_feature(&mut self, r: &Report) -> Result<()> {
        let mut buf = [0u8; REPORT + 1];
        buf[1..].copy_from_slice(r);
        if unsafe { HidD_SetFeature(self.h, buf.as_ptr() as *const c_void, buf.len() as u32) } == 0 {
            return derr(format!("Writing to the S75 Pro failed (Windows error {}).", unsafe { GetLastError() }));
        }
        Ok(())
    }

    fn get_feature(&mut self) -> Result<Report> {
        let mut buf = [0u8; REPORT + 1];
        if unsafe { HidD_GetFeature(self.h, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) } == 0 {
            return derr(format!("Reading from the S75 Pro failed (Windows error {}).", unsafe { GetLastError() }));
        }
        let mut r = [0u8; REPORT];
        r.copy_from_slice(&buf[1..]);
        Ok(r)
    }

    fn radio(&mut self, _: &[u8; RADIO]) -> Result<Option<[u8; RADIO]>> {
        derr("Not a dongle connection.")
    }

    fn screen(&mut self, chunk: &[u8]) -> Result<()> {
        let Some(ch) = self.screen.as_mut() else {
            return derr("The S75 Pro's screen interface wasn't found. Unplug the cable and plug it back in.");
        };
        ch.drain()?;
        let mut data = Vec::with_capacity(chunk.len() + 1);
        data.push(0);
        data.extend_from_slice(chunk);
        ch.write_bytes(&data)?;
        let _ = ch.recv(Duration::from_millis(300))?; // the keyboard's go-ahead; carry on without it
        Ok(())
    }
}

/// The dongle: 32-byte output reports, echoed in input reports.
struct DongleLink {
    ch: WinChannel,
}

impl Link for DongleLink {
    fn connection(&self) -> Connection {
        Connection::Dongle
    }

    fn set_feature(&mut self, _: &Report) -> Result<()> {
        derr("Not a USB connection.")
    }

    fn get_feature(&mut self) -> Result<Report> {
        derr("Not a USB connection.")
    }

    fn radio(&mut self, frame: &[u8; RADIO]) -> Result<Option<[u8; RADIO]>> {
        self.ch.drain()?;
        let mut data = [0u8; RADIO + 1];
        data[1..].copy_from_slice(frame);
        if self.ch.write_bytes(&data).is_err() {
            std::thread::sleep(Duration::from_millis(10));
            self.ch.write_bytes(&data)?;
        }
        let deadline = Instant::now() + Duration::from_millis(300);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(r) = self.ch.recv(left)? else { break };
            // with or without the report ID byte in front
            let r = if r.len() == RADIO + 1 { &r[1..] } else { &r[..] };
            if r.len() >= RADIO && r[..3] == frame[..3] {
                let mut out = [0u8; RADIO];
                out.copy_from_slice(&r[..RADIO]);
                return Ok(Some(out));
            }
        }
        Ok(None)
    }

    fn screen(&mut self, _: &[u8]) -> Result<()> {
        derr("The screen can only be changed over the USB cable.")
    }
}

/// Open an S75 Pro: the USB cable if it's there, else the dongle.
pub fn open_s75() -> Result<S75> {
    let f = find();
    if let Some(usb) = &f.usb {
        let h = open_rw(usb)?;
        let screen = f.screen.as_deref().and_then(|p| WinChannel::open(p).ok());
        return Ok(S75::new(Box::new(UsbLink { h, screen })));
    }
    if let Some(d) = &f.dongle {
        let ch = WinChannel::open(d).map_err(|e| DeviceError(e.0.replace("keyboard", "S75 Pro")))?;
        return Ok(S75::new(Box::new(DongleLink { ch })));
    }
    derr("No AULA S75 Pro found. Plug in the cable, or the dongle with the keyboard switched to 2.4G.")
}
