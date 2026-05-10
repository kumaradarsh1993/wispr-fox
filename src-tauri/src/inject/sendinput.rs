//! Win32 SendInput Unicode keystroke injection.
//!
//! Each codepoint is sent as a `INPUT_KEYBOARD` event with `wVk = 0` and
//! `dwFlags = KEYEVENTF_UNICODE`. Codepoints above the BMP (>0xFFFF) need a
//! UTF-16 surrogate pair — two events per codepoint. Newlines are translated
//! to VK_RETURN keystrokes (otherwise apps see U+000A literally and ignore it).

use std::mem::size_of;

use anyhow::{anyhow, Result};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_RETURN,
};

pub fn send(text: &str) -> Result<()> {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.len() * 2);

    for ch in text.chars() {
        if ch == '\n' {
            inputs.push(make_vk_input(VK_RETURN, false));
            inputs.push(make_vk_input(VK_RETURN, true));
            continue;
        }
        if ch == '\r' {
            // Drop bare CR; we handled CRLF via the \n branch above.
            continue;
        }

        let mut buf = [0u16; 2];
        let units = ch.encode_utf16(&mut buf);
        for &unit in units.iter() {
            inputs.push(make_unicode_input(unit, false));
            inputs.push(make_unicode_input(unit, true));
        }
    }

    if inputs.is_empty() {
        return Ok(());
    }

    let n = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if n as usize != inputs.len() {
        let err = unsafe { windows::Win32::Foundation::GetLastError() };
        return Err(anyhow!(
            "SendInput dispatched {n}/{} events (last error {:?})",
            inputs.len(),
            err
        ));
    }
    Ok(())
}

/// Send a Ctrl+V keystroke combo. Used by the clipboard fallback.
pub fn send_ctrl_v() -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_V};

    let inputs = [
        make_vk_input(VK_CONTROL, false),
        make_vk_input(VK_V, false),
        make_vk_input(VK_V, true),
        make_vk_input(VK_CONTROL, true),
    ];
    let n = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if n as usize != inputs.len() {
        return Err(anyhow!("Ctrl+V SendInput dispatched {n}/4 events"));
    }
    Ok(())
}

fn make_unicode_input(scan: u16, key_up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn make_vk_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    let flags: KEYBD_EVENT_FLAGS =
        if key_up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
