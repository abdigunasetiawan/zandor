//! Satu instance Zandor per pengguna. Dua instance berbagi folder data (registry, prefs, DB engine) dan masing-masing
//! membawa engine opencode sendiri, sehingga saling mengunci. Instance kedua membawa jendela pertama ke depan lalu keluar.
//! Peluncuran ulang cepat (mis. dev-watch mematikan lalu menjalankan lagi) diberi waktu ~2 detik agar mutex lama sempat lepas.

/// true = lanjut berjalan; false = sudah ada instance lain (jendelanya diaktifkan), proses ini harus keluar.
#[cfg(windows)]
pub fn acquire_or_activate() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};

    let name: Vec<u16> = r"Local\Zandor-GPUI-single-instance".encode_utf16().chain(std::iter::once(0)).collect();
    for _ in 0..8 {
        // SAFETY: `name` berakhir NUL dan hidup selama pemanggilan; atribut keamanan boleh null.
        let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if h.is_null() {
            return true; // gagal membuat mutex: jangan menghalangi aplikasi jalan
        }
        // SAFETY: dibaca segera setelah CreateMutexW pada thread yang sama.
        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            return true; // handle sengaja tidak ditutup: mutex hidup selama proses
        }
        // SAFETY: `h` handle sah milik kita.
        unsafe { CloseHandle(h) };
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let title: Vec<u16> = "Zandor".encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: judul berakhir NUL; handle jendela hanya dipakai untuk memunculkannya.
    unsafe {
        let w = FindWindowW(std::ptr::null(), title.as_ptr());
        if !w.is_null() {
            if IsIconic(w) != 0 {
                ShowWindow(w, SW_RESTORE);
            }
            SetForegroundWindow(w);
        }
    }
    false
}

// TODO(port): penjaga satu-instance untuk macOS/Linux.
#[cfg(not(windows))]
pub fn acquire_or_activate() -> bool {
    true
}
