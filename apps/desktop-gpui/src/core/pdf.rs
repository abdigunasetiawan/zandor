//! Perender PDF bawaan Windows (Windows.Data.Pdf, WinRT): jumlah halaman, ukuran halaman, dan render halaman ke PNG.
//! Pengganti pdf.js di versi React. Dipanggil dari thread latar (MTA).

#[derive(Clone, Debug, PartialEq)]
pub struct PdfInfo {
    /// Ukuran tiap halaman dalam DIP (1/96 inci).
    pub pages: Vec<(f32, f32)>,
}

#[cfg(windows)]
fn init_mta() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    // Aman dipanggil berulang; S_FALSE bila sudah diinisialisasi.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

#[cfg(windows)]
fn open(path: &str) -> windows::core::Result<windows::Data::Pdf::PdfDocument> {
    use windows::core::HSTRING;
    use windows::Data::Pdf::PdfDocument;
    use windows::Storage::StorageFile;
    init_mta();
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))?.get()?;
    PdfDocument::LoadFromFileAsync(&file)?.get()
}

/// Baca jumlah dan ukuran halaman.
pub fn info(path: &str) -> Result<PdfInfo, String> {
    #[cfg(windows)]
    {
        let doc = open(path).map_err(|e| e.message().to_string())?;
        let n = doc.PageCount().map_err(|e| e.message().to_string())?;
        let mut pages = Vec::with_capacity(n as usize);
        for i in 0..n {
            let p = doc.GetPage(i).map_err(|e| e.message().to_string())?;
            let s = p.Size().map_err(|e| e.message().to_string())?;
            pages.push((s.Width, s.Height));
        }
        Ok(PdfInfo { pages })
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("penampil PDF hanya tersedia di Windows".into())
    }
}

/// Render satu halaman (indeks 0) ke PNG dengan lebar `width` piksel.
pub fn render_page(path: &str, index: u32, width: u32) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        use windows::Data::Pdf::PdfPageRenderOptions;
        use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
        let run = || -> windows::core::Result<Vec<u8>> {
            let doc = open(path)?;
            let page = doc.GetPage(index)?;
            let stream = InMemoryRandomAccessStream::new()?;
            let opts = PdfPageRenderOptions::new()?;
            opts.SetDestinationWidth(width.max(16))?;
            // bawaan encoder = PNG
            page.RenderWithOptionsToStreamAsync(&stream, &opts)?.get()?;
            let size = stream.Size()? as u32;
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
            reader.LoadAsync(size)?.get()?;
            let mut buf = vec![0u8; size as usize];
            reader.ReadBytes(&mut buf)?;
            Ok(buf)
        };
        run().map_err(|e| e.message().to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (path, index, width);
        Err("penampil PDF hanya tersedia di Windows".into())
    }
}

#[cfg(test)]
mod tests {
    /// PDF minimal satu halaman (A4-ish 200x100 pt).
    fn tiny_pdf() -> Vec<u8> {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offs = vec![];
        for (n, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend(format!("{} 0 obj\n{}\nendobj\n", n + 1, o).as_bytes());
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offs {
            out.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }

    #[cfg(windows)]
    #[test]
    fn render_pdf_bawaan_windows() {
        let p = std::env::temp_dir().join(format!("zandor-uji-{}.pdf", std::process::id()));
        std::fs::write(&p, tiny_pdf()).unwrap();
        let path = p.to_string_lossy().into_owned();
        let info = super::info(&path).unwrap();
        assert_eq!(info.pages.len(), 1);
        assert!(info.pages[0].0 > info.pages[0].1);
        let png = super::render_page(&path, 0, 300).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!(&png[1..4], b"PNG");
    }
}
