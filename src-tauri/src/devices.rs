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
