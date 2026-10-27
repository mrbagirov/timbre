//! Finding the microphone and the virtual cable by name.
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Debug)]
pub struct MicInfo {
    pub found: bool,
    pub name: String,
    pub transport: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct CableInfo {
    pub found: bool,
    pub name: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Detect {
    pub mic: MicInfo,
    pub cable: CableInfo,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// What the user picked in settings. Empty means "choose automatically".
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Choice {
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub output: String,
}

pub fn name_of<D: DeviceTrait>(d: &D) -> String {
    d.description()
        .map(|x| x.name().to_string())
        .unwrap_or_default()
}

fn transport_of<D: DeviceTrait>(d: &D) -> String {
    d.description()
        .map(|x| x.interface_type().to_string())
        .unwrap_or_default()
}

fn is_mic_name(n: &str) -> bool {
    let l = n.to_lowercase();
    l.contains("am8") || l.contains("fifine")
}

fn is_cable_name(n: &str) -> bool {
    n.to_lowercase().contains("cable input")
}

pub fn detect() -> Detect {
    let host = cpal::default_host();
    let mut inputs = vec![];
    let mut outputs = vec![];
    let mut mic = MicInfo {
        found: false,
        name: String::new(),
        transport: String::new(),
    };
    let mut cable = CableInfo {
        found: false,
        name: String::new(),
    };
    if let Ok(devs) = host.input_devices() {
        for d in devs {
            let n = name_of(&d);
            if !mic.found && is_mic_name(&n) {
                mic = MicInfo {
                    found: true,
                    name: n.clone(),
                    transport: transport_of(&d),
                };
            }
            inputs.push(n);
        }
    }
    if let Ok(devs) = host.output_devices() {
        for d in devs {
            let n = name_of(&d);
            if !cable.found && is_cable_name(&n) {
                cable = CableInfo {
                    found: true,
                    name: n.clone(),
                };
            }
            outputs.push(n);
        }
    }
    if !mic.found {
        // not found by name: say which input will be used instead, so the interface can show it
        if let Some(d) = host.default_input_device() {
            mic.name = name_of(&d);
            mic.transport = transport_of(&d);
        }
    }
    Detect {
        mic,
        cable,
        inputs,
        outputs,
    }
}

pub fn pick_input(choice: &Choice) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !choice.input.is_empty() {
        if let Ok(devs) = host.input_devices() {
            for d in devs {
                if name_of(&d) == choice.input {
                    return Some(d);
                }
            }
        }
    }
    if let Ok(devs) = host.input_devices() {
        for d in devs {
            if is_mic_name(&name_of(&d)) {
                return Some(d);
            }
        }
    }
    host.default_input_device()
}

pub fn pick_output(choice: &Choice) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !choice.output.is_empty() {
        if let Ok(devs) = host.output_devices() {
            for d in devs {
                if name_of(&d) == choice.output {
                    return Some(d);
                }
            }
        }
    }
    if let Ok(devs) = host.output_devices() {
        for d in devs {
            if is_cable_name(&name_of(&d)) {
                return Some(d);
            }
        }
    }
    None
}

/// A text report for support: audio devices with their formats, and the USB interfaces Windows sees.
pub fn report(choice: &Choice) -> String {
    use std::fmt::Write;
    let host = cpal::default_host();
    let mut s = String::new();
    let _ = writeln!(s, "timbre {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        s,
        "chosen input: {:?}, output: {:?}",
        choice.input, choice.output
    );
    for (title, input) in [("inputs", true), ("outputs", false)] {
        let _ = writeln!(s, "\n{title}:");
        let devs: Vec<cpal::Device> = if input {
            host.input_devices()
                .map(|d| d.collect())
                .unwrap_or_default()
        } else {
            host.output_devices()
                .map(|d| d.collect())
                .unwrap_or_default()
        };
        for d in devs {
            let desc = d.description().ok();
            let name = desc
                .as_ref()
                .map(|x| x.name().to_string())
                .unwrap_or_default();
            let iface = desc
                .as_ref()
                .map(|x| x.interface_type().to_string())
                .unwrap_or_default();
            let _ = writeln!(s, "- {name} [{iface}]");
            let cfgs: Vec<_> = if input {
                d.supported_input_configs()
                    .map(|c| c.collect())
                    .unwrap_or_default()
            } else {
                d.supported_output_configs()
                    .map(|c| c.collect())
                    .unwrap_or_default()
            };
            for c in cfgs.iter().take(6) {
                let _ = writeln!(
                    s,
                    "    {} ch, {}..{} Hz, {:?}",
                    c.channels(),
                    c.min_sample_rate(),
                    c.max_sample_rate(),
                    c.sample_format()
                );
            }
        }
    }
    let _ = writeln!(s, "\nusb interfaces:");
    let _ = writeln!(s, "{}", usb_interfaces());
    let _ = writeln!(s, "\nmicrophone internals over the vendor channel (read only):");
    let _ = writeln!(s, "{}", crate::am8::report());
    let _ = writeln!(s, "\nmicrophone HID interfaces (3142:A010, read only):");
    let _ = writeln!(s, "{}", hid_probe(0x3142, 0xA010, 8000));
    s
}

#[cfg(windows)]
fn usb_interfaces() -> String {
    use std::os::windows::process::CommandExt;
    let script = "Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'USB\\VID_*' } | Select-Object FriendlyName,Class,InstanceId | Format-Table -AutoSize | Out-String -Width 240";
    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x0800_0000) // no console window
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(e) => format!("could not run PowerShell: {e}"),
    }
}

#[cfg(not(windows))]
fn usb_interfaces() -> String {
    "only available on Windows".to_string()
}

/// Read-only look at the HID interfaces of the microphone: what each one is, its report descriptor, and what it sends
/// while the user touches the mute button and turns the knobs. Nothing is ever written to the device.
#[cfg(windows)]
pub fn hid_probe(vid: u16, pid: u16, listen_ms: i32) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => return format!("hidapi: {e}"),
    };
    let infos: Vec<_> = api
        .device_list()
        .filter(|d| d.vendor_id() == vid && d.product_id() == pid)
        .collect();
    if infos.is_empty() {
        return format!("no HID interfaces for {vid:04X}:{pid:04X}");
    }
    let mut devs = Vec::new();
    for info in &infos {
        let _ = writeln!(
            s,
            "interface {} usage_page {:#06X} usage {:#06X} product {:?}",
            info.interface_number(),
            info.usage_page(),
            info.usage(),
            info.product_string().unwrap_or("")
        );
        match api.open_path(info.path()) {
            Ok(d) => {
                let mut desc = [0u8; 4096];
                match d.get_report_descriptor(&mut desc) {
                    Ok(n) => {
                        let hex: Vec<String> =
                            desc[..n].iter().map(|b| format!("{b:02X}")).collect();
                        let _ = writeln!(s, "  report descriptor ({n} bytes): {}", hex.join(" "));
                    }
                    Err(e) => {
                        let _ = writeln!(s, "  report descriptor: {e}");
                    }
                }
                devs.push((info.interface_number(), d));
            }
            Err(e) => {
                let _ = writeln!(s, "  could not open for reading: {e}");
            }
        }
    }
    let _ = writeln!(
        s,
        "\nlistening for {} ms, touch the mute button and turn the knobs:",
        listen_ms
    );
    let step = 50;
    let mut waited = 0;
    let mut buf = [0u8; 256];
    let mut seen = 0;
    while waited < listen_ms {
        for (iface, d) in &devs {
            if let Ok(n) = d.read_timeout(&mut buf, step) {
                if n > 0 && seen < 200 {
                    seen += 1;
                    let hex: Vec<String> = buf[..n].iter().map(|b| format!("{b:02X}")).collect();
                    let _ = writeln!(s, "  interface {iface}: {}", hex.join(" "));
                }
            }
        }
        waited += step * devs.len().max(1) as i32;
    }
    let _ = writeln!(s, "{seen} input reports seen");
    s
}

#[cfg(not(windows))]
pub fn hid_probe(_vid: u16, _pid: u16, _listen_ms: i32) -> String {
    "only available on Windows".to_string()
}
