//! Read-only look inside the Fifine AM8 (3142:A010, MVsilicon chip) over its vendor HID interface (usage page 0xFF00).
//!
//! The protocol was worked out by Austrum-lab/fifine-am8 (MIT) and dank0i/sc3. Here it is used for READS only:
//! a request is `A5 5A CTRL 00 16` (LEN 0 means "read"), and the reply comes back with the same CTRL.
//! Hard rules that this file enforces:
//!  - only CTRL codes from `READABLE` are ever sent, with an empty body;
//!  - the 8-byte Feature report is never touched (writing it can send the chip into its bootloader);
//!  - nothing is written to the microphone's settings.

#[cfg(windows)]
mod imp {
    use std::fmt::Write;
    use std::time::Duration;

    const VID: u16 = 0x3142;
    const PID: u16 = 0xA010;
    const REPORT: usize = 256;

    /// Blocks that are known to answer reads and have no side effects.
    const READABLE: &[u8] = &[
        0x00, // firmware version
        0x06, // PGA1 (mic analog)
        0x07, // ADC1 (mic digital)
        0x08, // AGC1
        0x09, // DAC0 (headphone volume)
        0x84, // noise gate, other path
        0x90, // mic noise gate
        0x91, // silence detector
        0x95, // mic gain / mute
        0x99, // mic EQ
        0xAD, // mic EQ, second instance
    ];

    struct Link {
        dev: hidapi::HidDevice,
    }

    impl Link {
        fn open(api: &hidapi::HidApi) -> Result<Link, String> {
            let info = api
                .device_list()
                .find(|d| {
                    d.vendor_id() == VID && d.product_id() == PID && d.usage_page() == 0xFF00
                })
                .ok_or("vendor HID interface 0xFF00 not found")?;
            let dev = api
                .open_path(info.path())
                .map_err(|e| format!("cannot open the vendor interface: {e}"))?;
            Ok(Link { dev })
        }

        /// Sends a read request for `ctrl` and waits for a reply with the same CTRL. Returns the reply body.
        fn read(&self, ctrl: u8) -> Option<Vec<u8>> {
            if !READABLE.contains(&ctrl) {
                return None;
            }
            // report id 0 + 256 data bytes
            let mut out = [0u8; REPORT + 1];
            out[1] = 0xA5;
            out[2] = 0x5A;
            out[3] = ctrl;
            out[4] = 0x00;
            out[5] = 0x16;
            for _ in 0..5 {
                if self.dev.write(&out).is_err() {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                std::thread::sleep(Duration::from_millis(10));
                let mut inp = [0u8; REPORT + 1];
                // on Windows the first byte is the report id (0)
                match self.dev.get_input_report(&mut inp) {
                    Ok(n) if n >= 5 => {
                        // the report id byte may or may not be counted, so look for the frame start at 0 or 1
                        let all = &inp[..n.min(REPORT + 1)];
                        for off in 0..2usize {
                            let b = &all[off.min(all.len())..];
                            if b.len() >= 4 && b[0] == 0xA5 && b[1] == 0x5A && b[2] == ctrl {
                                let len = b[3] as usize;
                                if b.len() >= 4 + len {
                                    return Some(b[4..4 + len].to_vec());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            None
        }

        /// `FF` followed by int16 LE values; the first one is the enable flag.
        fn params(&self, node: u8) -> Option<Vec<i16>> {
            let b = self.read(node)?;
            if b.first() != Some(&0xFF) {
                return None;
            }
            Some(
                b[1..]
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect(),
            )
        }
    }

    fn hex(b: &[u8]) -> String {
        b.iter()
            .map(|x| format!("{x:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn report() -> String {
        let mut s = String::new();
        let api = match hidapi::HidApi::new() {
            Ok(a) => a,
            Err(e) => return format!("hidapi: {e}"),
        };
        let link = match Link::open(&api) {
            Ok(l) => l,
            Err(e) => return e,
        };
        let _ = writeln!(s, "channel open, reading (read-only):");
        for &c in READABLE {
            match link.read(c) {
                Some(b) => {
                    let _ = writeln!(s, "  CTRL {c:02X}: {}", hex(&b));
                }
                None => {
                    let _ = writeln!(s, "  CTRL {c:02X}: no reply");
                }
            }
        }
        if let Some(g) = link.params(0x95) {
            if g.len() >= 3 {
                let _ = writeln!(
                    s,
                    "mic gain node: enabled={} mute={} gain={:+.2} dB",
                    g[0],
                    g[1],
                    g[2] as f32 / 100.0
                );
            }
        }
        if let Some(n) = link.params(0x90) {
            if n.len() >= 5 {
                let _ = writeln!(
                    s,
                    "mic noise gate: enabled={} threshold={:.1} dB ratio={} attack={} release={}",
                    n[0],
                    n[1] as f32 / 100.0,
                    n[2],
                    n[3],
                    n[4]
                );
            }
        }
        if let Some(e) = link.params(0x99) {
            let _ = writeln!(s, "mic EQ node 0x99 ({} values): {:?}", e.len(), e);
        }
        s
    }
}

#[cfg(windows)]
pub fn report() -> String {
    imp::report()
}

#[cfg(not(windows))]
pub fn report() -> String {
    "only available on Windows".to_string()
}
