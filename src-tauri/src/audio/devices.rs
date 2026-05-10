//! Input-device enumeration for the settings UI.

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct InputDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn list() -> Result<Vec<InputDeviceInfo>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let devices = host
        .input_devices()
        .context("enumerating input devices")?;

    let mut out = Vec::new();
    for dev in devices {
        let name = dev.name().unwrap_or_else(|_| "<unnamed>".into());
        let is_default = name == default_name;
        out.push(InputDeviceInfo { name, is_default });
    }
    Ok(out)
}
