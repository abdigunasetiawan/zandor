#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;

fn main() {
    // `Zandor office-check <berkas> ...` (ADR-074): pemeriksa dokumen untuk skill bawaan, jalan tanpa membuka jendela
    // dan sebelum single-instance, jadi aman dipanggil agent saat aplikasi terbuka.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("office-check") {
        let (out, code) = zandor_desktop_lib::office_check::run(&args[1..]);
        let _ = if code == 2 {
            writeln!(std::io::stderr(), "{out}")
        } else {
            writeln!(std::io::stdout(), "{out}")
        };
        std::process::exit(code);
    }
    zandor_desktop_lib::run()
}
