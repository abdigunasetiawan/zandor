//! Input suara (dikte). GPUI tidak punya Web Speech API milik WebView; di Windows port ini memakai dikte bawaan sistem
//! (Win+H) yang mengetik ke kolom yang sedang fokus — composer difokuskan dulu oleh pemanggil. Bahasa mengikuti
//! pengaturan dikte Windows. Daftar mikrofon dibaca dari WASAPI (port useAudioInputs); karena dikte Windows selalu
//! mendengar mikrofon bawaan sistem, memilih perangkat lain memunculkan peringatan + tautan ke Pengaturan Suara
//! (aplikasi tidak mengubah pengaturan sistem sendiri). Di OS lain dilaporkan tidak tersedia (tidak dipalsukan).

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

#[derive(Clone, Debug, PartialEq)]
pub struct AudioInput {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

pub const DEVICE_KEY: &str = "zandor.voice.device";

/// Mikrofon aktif (port enumerateDevices audioinput).
#[cfg(windows)]
pub fn audio_inputs() -> Vec<AudioInput> {
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::{eCapture, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    // SAFETY: panggilan COM standar; string hasil dibebaskan dengan CoTaskMemFree.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(en) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return vec![] };
        let take = |p: windows::core::PWSTR| {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as _));
            s
        };
        let def = en.GetDefaultAudioEndpoint(eCapture, eConsole).ok().and_then(|d| d.GetId().ok()).map(take).unwrap_or_default();
        let Ok(col) = en.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else { return vec![] };
        let n = col.GetCount().unwrap_or(0);
        let mut out = vec![];
        for i in 0..n {
            let Ok(d) = col.Item(i) else { continue };
            let Ok(id) = d.GetId().map(take) else { continue };
            let label = d
                .OpenPropertyStore(STGM_READ)
                .ok()
                .and_then(|ps| ps.GetValue(&PKEY_Device_FriendlyName).ok())
                .and_then(|v| PropVariantToStringAlloc(&v).ok())
                .map(take)
                .unwrap_or_default();
            out.push(AudioInput { is_default: id == def, id, label });
        }
        out
    }
}

#[cfg(not(windows))]
pub fn audio_inputs() -> Vec<AudioInput> {
    vec![]
}

/// Bila pengguna memilih mikrofon yang bukan bawaan sistem: nama perangkat itu (dikte Windows tidak akan memakainya).
pub fn chosen_not_default(chosen: &str, devices: &[AudioInput]) -> Option<String> {
    if chosen.is_empty() {
        return None;
    }
    devices.iter().find(|d| d.id == chosen && !d.is_default).map(|d| d.label.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_default_detection() {
        let d = vec![
            AudioInput { id: "a".into(), label: "A".into(), is_default: true },
            AudioInput { id: "b".into(), label: "B".into(), is_default: false },
        ];
        assert_eq!(chosen_not_default("", &d), None);
        assert_eq!(chosen_not_default("a", &d), None);
        assert_eq!(chosen_not_default("b", &d), Some("B".into()));
        assert_eq!(chosen_not_default("gone", &d), None);
        let _ = audio_inputs();
    }
}
