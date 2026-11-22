//! Input suara (dikte). Aplikasi merekam sendiri dari mikrofon pilihan pengguna (`voice_capture`, WASAPI) dan
//! mengenali ucapan dengan Whisper lokal (`voice_stt`); sesi di `voice_session`. Modul ini memuat daftar mikrofon
//! (port useAudioInputs) dan kunci preferensi. Menggantikan pintasan dikte Windows (Win+H) yang tidak bisa memilih
//! perangkat maupun bahasa. Di OS lain dilaporkan tidak tersedia (tidak dipalsukan).

/// true bila perekaman mikrofon tersedia di platform ini.
pub fn supported() -> bool {
    cfg!(windows)
}

/// Kunci preferensi dikte (prefs.json): kosakata (initial prompt) dan kamus koreksi `salah = benar` per baris.
pub const VOCAB_KEY: &str = "zandor.voice.vocabulary";
pub const CORRECTIONS_KEY: &str = "zandor.voice.corrections";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_inputs_does_not_panic_and_has_at_most_one_default() {
        let d = audio_inputs();
        assert!(d.iter().filter(|x| x.is_default).count() <= 1);
    }
}
