//! Check the loopback capture on its own: `cargo run --example loopback` while music plays.
//! Captures 1.5 s from every output device and prints how loud each one was.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn main() {
    let host = cpal::default_host();
    let default = host.default_output_device().and_then(|d| d.name().ok()).unwrap_or_default();
    for device in host.output_devices().expect("no output devices") {
        let name = device.name().unwrap_or_default();
        let Ok(config) = device.default_output_config() else {
            println!("{name}: no config");
            continue;
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(Mutex::new(0f32));
        let (c, p) = (calls.clone(), peak.clone());
        let stream = device.build_input_stream(
            &config.config(),
            move |data: &[f32], _: &_| {
                c.fetch_add(1, Ordering::Relaxed);
                let m = data.iter().fold(0f32, |a, v| a.max(v.abs()));
                let mut pk = p.lock().unwrap();
                *pk = pk.max(m);
            },
            |e| eprintln!("stream error: {e}"),
            None,
        );
        let Ok(stream) = stream else {
            println!("{name}: can't capture");
            continue;
        };
        stream.play().ok();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        drop(stream);
        println!(
            "{}{name} ({} ch, {} Hz): callbacks {}, peak {:.4}",
            if name == default { "[default] " } else { "" },
            config.channels(),
            config.sample_rate().0,
            calls.load(Ordering::Relaxed),
            *peak.lock().unwrap()
        );
    }
}
