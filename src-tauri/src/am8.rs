//! Read-only look inside the Fifine AM8 (3142:A010, MVsilicon chip) over its vendor HID interface (usage page 0xFF00).
//!
//! The protocol was worked out by Austrum-lab/fifine-am8 (MIT) and dank0i/sc3. Here it is used for READS only:
//! a request is `A5 5A CTRL 00 16` (LEN 0 means "read"), and the reply comes back with the same CTRL.
//! Hard rules that this file enforces:
//!  - reads: only CTRL codes from `READABLE`, with an empty body;
//!  - writes: only single-parameter writes (`A5 5A node 03 idx lo hi 16`) into the three nodes in `WRITABLE`
//!    (mic gain/mute 0x95, mic gate 0x90, mic EQ 0x99), with the parameter index and the value range checked;
//!    every write is read back and compared;
//!  - the 8-byte Feature report is never touched (writing it can send the chip into its bootloader), and the
//!    dangerous codes (0xFE, 0x11, 0xFB, 0xFD, 0xFF, 0xA0, 0xA1, 0xA3) are not reachable from here;
//!  - settings live in the microphone's RAM and are lost when it is replugged.

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

    const GAIN_NODE: u8 = 0x95;
    const GATE_NODE: u8 = 0x90;
    const EQ_NODE: u8 = 0x99;

    impl Link {
        /// Single-parameter write. The firmware does not reply, so the caller verifies by reading back.
        fn write_param(&self, node: u8, idx: u8, val: i16) -> Result<(), String> {
            // allow-list: node and parameter index
            let ok = match node {
                GAIN_NODE => idx == 1 || idx == 2,   // mute flag, gain in centi-dB
                GATE_NODE => idx <= 4,                // enable, threshold, ratio, attack, release
                EQ_NODE => idx <= 52,                 // enable, pregain, calc type, 10 filters x 5
                _ => false,
            };
            if !ok {
                return Err(format!("refused: node {node:02X} index {idx}"));
            }
            let mut out = [0u8; REPORT + 1];
            out[1] = 0xA5;
            out[2] = 0x5A;
            out[3] = node;
            out[4] = 0x03;
            out[5] = idx;
            let v = val.to_le_bytes();
            out[6] = v[0];
            out[7] = v[1];
            out[8] = 0x16;
            self.dev.write(&out).map_err(|e| format!("write: {e}"))?;
            std::thread::sleep(Duration::from_millis(10));
            Ok(())
        }

        /// Reads a node after a write. A different block is read first, because the chip re-serves its last reply.
        fn fresh_params(&self, node: u8) -> Option<Vec<i16>> {
            let _ = self.read(0x00);
            self.params(node)
        }

        fn set_checked(&self, node: u8, idx: u8, val: i16) -> Result<(), String> {
            self.write_param(node, idx, val)?;
            let p = self.fresh_params(node).ok_or("no reply after the write")?;
            match p.get(idx as usize) {
                Some(&v) if v == val => Ok(()),
                other => Err(format!("not applied: wanted {val}, read back {other:?}")),
            }
        }
    }

    #[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
    #[serde(rename_all = "camelCase", default)]
    pub struct Status {
        pub gain_db: f32,
        pub mute: bool,
        pub gate_on: bool,
        pub gate_db: f32,
        pub gate_ratio: i16,
        pub lowcut_on: bool,
        pub lowcut_hz: i16,
        pub clarity_db: f32,
        pub bass_db: f32,
        pub monitor: f32,
    }

    fn band(k: usize) -> usize {
        3 + (k - 1) * 5
    }

    /// One open link is kept between calls (the status is polled twice a second) and re-opened after any error.
    fn with_link<T>(f: impl FnOnce(&Link) -> Result<T, String>) -> Result<T, String> {
        static LINK: std::sync::Mutex<Option<Link>> = std::sync::Mutex::new(None);
        let mut guard = LINK.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            let api = hidapi::HidApi::new().map_err(|e| format!("hidapi: {e}"))?;
            *guard = Some(Link::open(&api)?);
        }
        let res = f(guard.as_ref().unwrap());
        if res.is_err() {
            *guard = None;
        }
        res
    }

    fn read_status(l: &Link) -> Result<Status, String> {
        let g = l.fresh_params(GAIN_NODE).ok_or("no reply from the gain block")?;
        let n = l.params(GATE_NODE).ok_or("no reply from the gate block")?;
        let e = l.params(EQ_NODE).ok_or("no reply from the EQ block")?;
        if g.len() < 3 || n.len() < 5 || e.len() < band(3) + 5 {
            return Err("unexpected reply shape".into());
        }
        let eq_on = e[0] != 0;
        let b1 = band(1);
        let b2 = band(2);
        let b3 = band(3);
        Ok(Status {
            gain_db: g[2] as f32 / 100.0,
            mute: g[1] != 0,
            gate_on: n[0] != 0,
            gate_db: n[1] as f32 / 100.0,
            gate_ratio: n[2],
            lowcut_on: eq_on && e[b1] != 0 && e[b1 + 1] == 4,
            lowcut_hz: e[b1 + 2],
            clarity_db: if eq_on && e[b2] != 0 { e[b2 + 4] as f32 / 256.0 } else { 0.0 },
            bass_db: if eq_on && e[b3] != 0 { e[b3 + 4] as f32 / 256.0 } else { 0.0 },
            monitor: monitor_pct(l),
        })
    }

    /// Headphone knob on the body: the DAC volume, 0..4095, left channel at body offset 7.
    fn monitor_pct(l: &Link) -> f32 {
        match l.read(0x09) {
            Some(d) if d.len() >= 9 => u16::from_le_bytes([d[7], d[8]]) as f32 * 100.0 / 4095.0,
            _ => -1.0,
        }
    }

    /// Light read for the live view: mute flag, gain and the headphone knob only.
    pub fn poll(prev: &Status) -> Result<Status, String> {
        let prev = prev.clone();
        with_link(move |l| {
            let g = l.params(GAIN_NODE).ok_or("no reply from the gain block")?;
            if g.len() < 3 {
                return Err("unexpected reply shape".into());
            }
            let mut s = prev;
            s.mute = g[1] != 0;
            s.gain_db = g[2] as f32 / 100.0;
            s.monitor = monitor_pct(l);
            Ok(s)
        })
    }

    pub fn status() -> Result<Status, String> {
        with_link(read_status)
    }

    /// keys: gain (dB, -12..12), mute (0/1), gate (0 = off, else threshold dB -60..-20),
    /// lowcut (0 = off, else Hz 60..300), clarity (0 = off, else dB 0.5..9), bass (0 = off, else dB -12..9)
    pub fn set(key: &str, value: f64) -> Result<Status, String> {
        if !value.is_finite() {
            return Err("bad value".into());
        }
        with_link(|l| {
            match key {
                "gain" => l.set_checked(GAIN_NODE, 2, (value.clamp(-12.0, 12.0) * 100.0).round() as i16)?,
                "mute" => l.set_checked(GAIN_NODE, 1, if value >= 0.5 { 1 } else { 0 })?,
                "gate" => {
                    if value == 0.0 {
                        l.set_checked(GATE_NODE, 0, 0)?;
                    } else {
                        l.set_checked(GATE_NODE, 1, (value.clamp(-60.0, -20.0) * 100.0).round() as i16)?;
                        l.set_checked(GATE_NODE, 2, 4)?;
                        l.set_checked(GATE_NODE, 0, 1)?;
                    }
                }
                "lowcut" => {
                    let b = band(1) as u8;
                    if value == 0.0 {
                        l.set_checked(EQ_NODE, b, 0)?;
                    } else {
                        l.set_checked(EQ_NODE, b + 1, 4)?;
                        l.set_checked(EQ_NODE, b + 2, value.clamp(60.0, 300.0).round() as i16)?;
                        l.set_checked(EQ_NODE, b + 3, 724)?;
                        l.set_checked(EQ_NODE, b + 4, 0)?;
                        l.set_checked(EQ_NODE, b, 1)?;
                        l.set_checked(EQ_NODE, 0, 1)?;
                    }
                }
                "clarity" => {
                    let b = band(2) as u8;
                    if value == 0.0 {
                        l.set_checked(EQ_NODE, b, 0)?;
                    } else {
                        l.set_checked(EQ_NODE, b + 1, 0)?;
                        l.set_checked(EQ_NODE, b + 2, 3000)?;
                        l.set_checked(EQ_NODE, b + 3, 1024)?;
                        l.set_checked(EQ_NODE, b + 4, (value.clamp(0.5, 9.0) * 256.0).round() as i16)?;
                        l.set_checked(EQ_NODE, b, 1)?;
                        l.set_checked(EQ_NODE, 0, 1)?;
                    }
                }
                "bass" => {
                    let b = band(3) as u8;
                    if value == 0.0 {
                        l.set_checked(EQ_NODE, b, 0)?;
                    } else {
                        l.set_checked(EQ_NODE, b + 1, 1)?;
                        l.set_checked(EQ_NODE, b + 2, 150)?;
                        l.set_checked(EQ_NODE, b + 3, 724)?;
                        l.set_checked(EQ_NODE, b + 4, (value.clamp(-12.0, 9.0) * 256.0).round() as i16)?;
                        l.set_checked(EQ_NODE, b, 1)?;
                        l.set_checked(EQ_NODE, 0, 1)?;
                    }
                }
                _ => return Err(format!("unknown setting {key}")),
            }
            read_status(l)
        })
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
pub use imp::Status;
#[cfg(windows)]
pub fn status() -> Result<Status, String> {
    imp::status()
}
#[cfg(windows)]
pub fn poll(prev: &Status) -> Result<Status, String> {
    imp::poll(prev)
}
#[cfg(windows)]
pub fn set(key: &str, value: f64) -> Result<Status, String> {
    imp::set(key, value)
}

#[cfg(windows)]
pub fn report() -> String {
    imp::report()
}

#[cfg(not(windows))]
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {}
#[cfg(not(windows))]
pub fn status() -> Result<Status, String> {
    Err("only available on Windows".into())
}
#[cfg(not(windows))]
pub fn poll(_prev: &Status) -> Result<Status, String> {
    Err("only available on Windows".into())
}
#[cfg(not(windows))]
pub fn set(_key: &str, _value: f64) -> Result<Status, String> {
    Err("only available on Windows".into())
}

#[cfg(not(windows))]
pub fn report() -> String {
    "only available on Windows".to_string()
}
