//! Windows HID access to the F75's configuration channel. Ported from aula-f75-hub's
//! `aula/backends/windows.py` (MIT), which was tested on hardware (2026-10-20).

use crate::device::{derr, Channel, DeviceError, FeatureChannel, KeyGuard, Mode, Result};
use crate::protocol::{Packet, WiredReport, PACKET_LEN, WIRED_REPORT_LEN};
use serde::Serialize;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::time::{Duration, Instant};
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetFeature, HidD_GetHidGuid, HidD_GetPreparsedData,
    HidD_GetProductString, HidD_SetFeature, HidP_GetCaps, HIDD_ATTRIBUTES, HIDP_CAPS,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyboardLayout};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

const VENDOR_WIRELESS: u16 = 0x3554;
const PID_DONGLE: u16 = 0xFA09;
// FA07 is what upstream saw (Linux); an F75 paired with Windows over classic Bluetooth says FA08
const PID_BLUETOOTH: [u16; 2] = [0xFA07, 0xFA08];
const VENDOR_WIRED: u16 = 0x258A;
const PID_WIRED: u16 = 0x010C;
const VENDOR_USAGE_PAGE: u16 = 0xFF02;
const VENDOR_USAGE: u16 = 0x02;
const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;

fn bad(h: HANDLE) -> bool {
    h.is_null() || h == INVALID_HANDLE_VALUE
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub path: String,
    pub mode: String,
    pub supported: bool,
    pub name: String,
}

/// Device paths of every HID collection present.
fn interface_paths() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let mut guid: GUID = zeroed();
        HidD_GetHidGuid(&mut guid);
        let info = SetupDiGetClassDevsW(&guid, std::ptr::null(), std::ptr::null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if info == 0 || info == -1 {
            return out;
        }
        let mut i = 0;
        loop {
            let mut iface: SP_DEVICE_INTERFACE_DATA = zeroed();
            iface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(info, std::ptr::null(), &guid, i, &mut iface) == 0 {
                break;
            }
            i += 1;
            let mut need = 0u32;
            SetupDiGetDeviceInterfaceDetailW(info, &iface, std::ptr::null_mut(), 0, &mut need, std::ptr::null_mut());
            if need == 0 {
                continue;
            }
            let mut buf = vec![0u32; (need as usize).div_ceil(4)];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(info, &iface, detail, need, std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                continue;
            }
            let p = (*detail).DevicePath.as_ptr();
            let len = (0..).take_while(|&n| *p.add(n) != 0).count();
            out.push(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
        }
        SetupDiDestroyDeviceInfoList(info);
    }
    out
}

fn open(path: &str, access: u32, flags: u32) -> Option<HANDLE> {
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let h = unsafe {
        CreateFileW(wide.as_ptr(), access, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, flags, std::ptr::null_mut())
    };
    (!bad(h)).then_some(h)
}

fn caps(h: HANDLE) -> Option<HIDP_CAPS> {
    unsafe {
        let mut pp = zeroed();
        if HidD_GetPreparsedData(h, &mut pp) == 0 {
            return None;
        }
        let mut c: HIDP_CAPS = zeroed();
        let ok = HidP_GetCaps(pp, &mut c) == HIDP_STATUS_SUCCESS;
        HidD_FreePreparsedData(pp);
        ok.then_some(c)
    }
}

/// Could this HID path be an F75? USB paths say `vid_3554`, but Bluetooth ones say
/// `vid&023554` (LE) or `vid&00023554` (classic), so only the vendor digits are matched;
/// the attributes decide.
fn maybe_aula(path: &str) -> bool {
    let lower = path.to_lowercase();
    let Some(vid) = lower.find("vid") else { return false };
    let tail = &lower[vid..];
    let tail = &tail[..tail.find("pid").unwrap_or(tail.len())];
    tail.contains("3554") || tail.contains("258a")
}

/// The configuration channel of each F75: the vendor collection with 20-byte reports
/// (wireless), or the one with the 520-byte feature report (wired).
pub fn find_devices() -> Vec<DeviceInfo> {
    let mut found = Vec::new();
    for path in interface_paths() {
        if !maybe_aula(&path) {
            continue;
        }
        // opened without read/write access: works even for collections Windows keeps for itself
        let Some(h) = open(&path, 0, 0) else { continue };
        unsafe {
            let mut a: HIDD_ATTRIBUTES = zeroed();
            a.Size = size_of::<HIDD_ATTRIBUTES>() as u32;
            if HidD_GetAttributes(h, &mut a) != 0 {
                let mut name = [0u16; 128];
                let name = if HidD_GetProductString(h, name.as_mut_ptr() as *mut c_void, 256) != 0 {
                    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
                    String::from_utf16_lossy(&name[..len])
                } else {
                    String::new()
                };
                let c = caps(h);
                let usage = c.map(|c| (c.UsagePage, c.Usage)).unwrap_or((0, 0));
                let feature_len = c.map(|c| c.FeatureReportByteLength as usize).unwrap_or(0);
                if (a.VendorID, a.ProductID) == (VENDOR_WIRED, PID_WIRED) {
                    if feature_len == WIRED_REPORT_LEN {
                        found.push(DeviceInfo { path: path.clone(), mode: "USB cable".into(), supported: true, name });
                    }
                } else if a.VendorID == VENDOR_WIRELESS
                    && (a.ProductID == PID_DONGLE || PID_BLUETOOTH.contains(&a.ProductID))
                    && usage == (VENDOR_USAGE_PAGE, VENDOR_USAGE)
                {
                    let mode = if a.ProductID == PID_DONGLE { "2.4G dongle" } else { "Bluetooth" };
                    found.push(DeviceInfo { path: path.clone(), mode: mode.into(), supported: true, name });
                }
            }
            CloseHandle(h);
        }
    }
    found
}

/// Overlapped reads and writes on the collection's handle. A read that times out stays
/// pending and is picked up by the next recv().
pub struct WinChannel {
    h: HANDLE,
    in_len: usize,
    out_len: usize,
    rd_ev: HANDLE,
    wr_ev: HANDLE,
    rd_ov: Box<OVERLAPPED>,
    rd_buf: Box<[u8]>,
    pending: bool,
}

// The handles are only used from one thread at a time (the app keeps the keyboard behind a mutex).
unsafe impl Send for WinChannel {}

impl WinChannel {
    pub fn open(path: &str) -> Result<Self> {
        let Some(h) = open(path, GENERIC_READ | GENERIC_WRITE, FILE_FLAG_OVERLAPPED) else {
            let e = unsafe { GetLastError() };
            if e == ERROR_ACCESS_DENIED {
                return derr("Can't open the keyboard: access denied. Close the official AULA app (and other keyboard tools), then try again.");
            }
            return derr(format!("Can't open the keyboard (Windows error {e})."));
        };
        let c = caps(h);
        let in_len = c.map(|c| c.InputReportByteLength as usize).unwrap_or(64).max(1);
        let out_len = c.map(|c| c.OutputReportByteLength as usize).unwrap_or(PACKET_LEN);
        unsafe {
            Ok(WinChannel {
                h,
                in_len,
                out_len,
                rd_ev: CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()),
                wr_ev: CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()),
                rd_ov: Box::new(zeroed()),
                rd_buf: vec![0u8; in_len].into_boxed_slice(),
                pending: false,
            })
        }
    }

    fn last_error(what: &str) -> DeviceError {
        DeviceError(format!("{what} (Windows error {})", unsafe { GetLastError() }))
    }
}

impl Channel for WinChannel {
    fn write(&mut self, pkt: &Packet) -> Result<()> {
        // Windows wants exactly the collection's output report length
        let mut data = vec![0u8; self.out_len.max(PACKET_LEN)];
        data[..PACKET_LEN].copy_from_slice(pkt);
        data.truncate(self.out_len);
        unsafe {
            ResetEvent(self.wr_ev);
            let mut ov: OVERLAPPED = zeroed();
            ov.hEvent = self.wr_ev;
            if WriteFile(self.h, data.as_ptr(), data.len() as u32, std::ptr::null_mut(), &mut ov) == 0
                && GetLastError() != ERROR_IO_PENDING
            {
                return Err(Self::last_error("Writing to the keyboard failed"));
            }
            if WaitForSingleObject(self.wr_ev, 1000) != WAIT_OBJECT_0 {
                // cancel only this write (not the pending read), and wait until Windows is
                // done with `ov` before it goes out of scope
                CancelIoEx(self.h, &ov);
                let mut done = 0u32;
                GetOverlappedResult(self.h, &ov, &mut done, 1);
                return derr("Writing to the keyboard timed out.");
            }
            let mut done = 0u32;
            if GetOverlappedResult(self.h, &ov, &mut done, 0) == 0 {
                return Err(Self::last_error("Writing to the keyboard failed"));
            }
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        unsafe {
            if !self.pending {
                ResetEvent(self.rd_ev);
                *self.rd_ov = zeroed();
                self.rd_ov.hEvent = self.rd_ev;
                if ReadFile(self.h, self.rd_buf.as_mut_ptr(), self.in_len as u32, std::ptr::null_mut(), &mut *self.rd_ov) == 0
                    && GetLastError() != ERROR_IO_PENDING
                {
                    return Err(Self::last_error("Reading from the keyboard failed"));
                }
                self.pending = true;
            }
            if WaitForSingleObject(self.rd_ev, timeout.as_millis() as u32) == WAIT_TIMEOUT {
                return Ok(None);
            }
            self.pending = false;
            let mut got = 0u32;
            if GetOverlappedResult(self.h, &*self.rd_ov, &mut got, 0) == 0 {
                // Windows cancels a thread's pending I/O when the thread exits. The app uses
                // the channel from different worker threads, so a read left pending by one of
                // them can come back aborted: that's no data, and the next call reads again.
                if GetLastError() == ERROR_OPERATION_ABORTED {
                    return Ok(None);
                }
                return Err(Self::last_error("Reading from the keyboard failed"));
            }
            Ok(Some(self.rd_buf[..got as usize].to_vec()))
        }
    }
}

/// The wired configuration channel. Feature reports are synchronous, so no overlapped I/O.
pub struct WiredChannel {
    h: HANDLE,
}

impl WiredChannel {
    pub fn open(path: &str) -> Result<Self> {
        match open(path, GENERIC_READ | GENERIC_WRITE, 0) {
            Some(h) => Ok(WiredChannel { h }),
            None if unsafe { GetLastError() } == ERROR_ACCESS_DENIED => derr(
                "Can't open the keyboard: access denied. Close the official AULA app (and other keyboard tools), then try again.",
            ),
            None => derr(format!("Can't open the keyboard (Windows error {}).", unsafe { GetLastError() })),
        }
    }
}

// as for WinChannel: used from one thread at a time
unsafe impl Send for WiredChannel {}

impl FeatureChannel for WiredChannel {
    fn set(&mut self, report: &WiredReport) -> Result<()> {
        if unsafe { HidD_SetFeature(self.h, report.as_ptr() as *const c_void, WIRED_REPORT_LEN as u32) } == 0 {
            return Err(WinChannel::last_error("Writing to the keyboard failed"));
        }
        Ok(())
    }

    fn get(&mut self, report: &mut WiredReport) -> Result<()> {
        if unsafe { HidD_GetFeature(self.h, report.as_mut_ptr() as *mut c_void, WIRED_REPORT_LEN as u32) } == 0 {
            return Err(WinChannel::last_error("Reading from the keyboard failed"));
        }
        Ok(())
    }
}

impl Drop for WiredChannel {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.h) };
    }
}

impl Drop for WinChannel {
    fn drop(&mut self) {
        unsafe {
            if self.pending {
                // CancelIoEx: the read may have been issued by another thread
                CancelIoEx(self.h, &*self.rd_ov);
                let mut got = 0u32;
                GetOverlappedResult(self.h, &*self.rd_ov, &mut got, 1);
            }
            CloseHandle(self.rd_ev);
            CloseHandle(self.wr_ev);
            CloseHandle(self.h);
        }
    }
}

/// Every virtual key except mouse buttons (a GUI click holds one) and the pseudo keys IME
/// and packet input use. GetAsyncKeyState also sees other keyboards.
pub struct WinKeyGuard {
    last_down: Instant,
}

impl WinKeyGuard {
    pub fn new() -> Self {
        WinKeyGuard { last_down: Instant::now() - Duration::from_secs(10) }
    }

    fn any_down() -> bool {
        (0x08..0xFF).filter(|vk| *vk != 0xE5 && *vk != 0xE7).any(|vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0)
    }
}

impl KeyGuard for WinKeyGuard {
    fn wait_released(&mut self, quiet: Duration, limit: Duration) -> Result<()> {
        let start = Instant::now();
        loop {
            let now = Instant::now();
            if Self::any_down() {
                self.last_down = now;
            } else if now - self.last_down >= quiet {
                return Ok(());
            }
            if now - start > limit {
                return derr("A key seems to be held down. Release all keys and try again.");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Open the keyboard: the one supported channel, or (several candidates) the one that answers.
pub fn open_keyboard() -> Result<crate::device::Keyboard> {
    let all = find_devices();
    let mut devs: Vec<&DeviceInfo> = all.iter().filter(|d| d.supported).collect();
    // Windows keeps listing a paired Bluetooth keyboard while it's on the cable: try the cable first
    devs.sort_by_key(|d| d.mode != "USB cable");
    if devs.is_empty() {
        return derr("AULA F75 not found. Is the dongle plugged in and the keyboard switched on (2.4G or Bluetooth), or the cable plugged in with the switch on wired?");
    }
    let make = |d: &DeviceInfo| -> Result<crate::device::Keyboard> {
        if d.mode == "USB cable" {
            return Ok(crate::device::Keyboard::wired(Box::new(WiredChannel::open(&d.path)?), Box::new(WinKeyGuard::new())));
        }
        let mode = if d.mode == "Bluetooth" { Mode::Bluetooth } else { Mode::Dongle };
        Ok(crate::device::Keyboard::new(mode, Box::new(WinChannel::open(&d.path)?), Box::new(WinKeyGuard::new())))
    };
    if devs.len() == 1 {
        return make(devs[0]);
    }
    for d in devs {
        let mut kb = make(d)?;
        if kb.probe()? {
            return Ok(kb);
        }
    }
    derr("The keyboard didn't answer. It may be asleep: press any key to wake it, then try again.")
}

/// Is the official AULA app (OemDrv.exe) running? It uses the same channel.
pub fn vendor_app_running() -> bool {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if bad(snap) {
            return false;
        }
        let mut e: PROCESSENTRY32W = zeroed();
        e.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut more = Process32FirstW(snap, &mut e) != 0;
        while more {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case("OemDrv.exe") {
                found = true;
                break;
            }
            more = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
        found
    }
}

/// Turkish Q (KLID 0000041F) or US for macro text: the layout of the window in front.
pub fn detect_layout() -> crate::macros::Layout {
    unsafe {
        let mut tid = 0;
        let w = GetForegroundWindow();
        if !w.is_null() {
            tid = GetWindowThreadProcessId(w, std::ptr::null_mut());
        }
        // Turkish Q is HKL 041F041F; Turkish F shares the language word but not the layout word
        let hkl = GetKeyboardLayout(tid) as usize;
        if hkl & 0xFFFF_FFFF == 0x041F_041F {
            crate::macros::Layout::TurkishQ
        } else {
            crate::macros::Layout::Us
        }
    }
}

#[cfg(test)]
mod tests {
    use super::maybe_aula;

    #[test]
    fn finds_usb_and_bluetooth_paths() {
        assert!(maybe_aula(r"\\?\hid#vid_3554&pid_fa09&mi_01&col06#8&2c132f91&0&0005#{4d1e55b2-f16f-11cf-88cb-001111000030}"));
        assert!(maybe_aula(r"\\?\hid#{00001812-0000-1000-8000-00805f9b34fb}_dev_vid&023554_pid&fa07_rev&0001_e4b1c2a3d4f5&col03#9&1a2b3c4d&0&0002#{4d1e55b2-f16f-11cf-88cb-001111000030}"));
        assert!(maybe_aula(r"\\?\hid#{00001124-0000-1000-8000-00805f9b34fb}_vid&00023554_pid&fa07&col03#8&2f2e3d4c&0&0002#{4d1e55b2-f16f-11cf-88cb-001111000030}"));
        assert!(maybe_aula(r"\\?\hid#vid_258a&pid_010c&mi_01&col05#8&1&0&0004#{4d1e55b2-f16f-11cf-88cb-001111000030}"));
        // another vendor whose product id happens to be 3554
        assert!(!maybe_aula(r"\\?\hid#vid_046d&pid_3554#8&1&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"));
    }
}
