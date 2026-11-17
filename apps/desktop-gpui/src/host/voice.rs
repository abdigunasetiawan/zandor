//! Input suara (dikte). GPUI tidak punya Web Speech API milik WebView; di Windows port ini memakai dikte bawaan sistem
//! (Win+H) yang mengetik ke kolom yang sedang fokus — composer difokuskan dulu oleh pemanggil. Perangkat input dan
//! bahasa mengikuti pengaturan dikte Windows. Di OS lain dilaporkan tidak tersedia (tidak dipalsukan).

/// true bila dikte sistem tersedia di platform ini.
pub fn supported() -> bool {
    cfg!(windows)
}

/// Buka/tutup panel dikte sistem (Win+H menyalakan/mematikan). Galat dilaporkan apa adanya.
#[cfg(windows)]
pub fn toggle_dictation() -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_LWIN};
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    };
    let h = b'H' as u16;
    let seq = [key(VK_LWIN, false), key(h, false), key(h, true), key(VK_LWIN, true)];
    // SAFETY: larik INPUT valid dan ukuran struktur benar.
    let n = unsafe { SendInput(seq.len() as u32, seq.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    if n as usize == seq.len() { Ok(()) } else { Err("SendInput ditolak sistem".into()) }
}

#[cfg(not(windows))]
pub fn toggle_dictation() -> Result<(), String> {
    Err("dikte sistem tidak tersedia".into())
}
