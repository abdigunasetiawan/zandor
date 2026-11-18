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

/// Satu baris teks hasil OCR: teks baris + kata dengan kotak ternormalisasi (0..1 terhadap halaman).
#[derive(Clone, Debug, Default)]
pub struct TextLine {
    pub words: Vec<(String, [f32; 4])>,
}

/// Lapisan teks halaman. Windows.Data.Pdf tidak memberi teks, jadi halaman dirender lalu dibaca dengan OCR bawaan
/// Windows (Windows.Media.Ocr, bahasa profil pengguna). Hasil kosong bila OCR tidak tersedia.
pub fn page_text(path: &str, index: u32) -> Result<Vec<TextLine>, String> {
    #[cfg(windows)]
    {
        use windows::Data::Pdf::PdfPageRenderOptions;
        use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, SoftwareBitmap};
        use windows::Media::Ocr::OcrEngine;
        use windows::Storage::Streams::InMemoryRandomAccessStream;
        let run = || -> windows::core::Result<Vec<TextLine>> {
            let doc = open(path)?;
            let page = doc.GetPage(index)?;
            let stream = InMemoryRandomAccessStream::new()?;
            let opts = PdfPageRenderOptions::new()?;
            let w = 1800u32;
            opts.SetDestinationWidth(w)?;
            page.RenderWithOptionsToStreamAsync(&stream, &opts)?.get()?;
            let dec = BitmapDecoder::CreateAsync(&stream)?.get()?;
            let bmp = dec.GetSoftwareBitmapAsync()?.get()?;
            let bmp = SoftwareBitmap::ConvertWithAlpha(&bmp, BitmapPixelFormat::Bgra8, BitmapAlphaMode::Premultiplied)?;
            let (bw, bh) = (bmp.PixelWidth()? as f32, bmp.PixelHeight()? as f32);
            let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
            let res = engine.RecognizeAsync(&bmp)?.get()?;
            let mut out = vec![];
            for line in res.Lines()? {
                let mut words = vec![];
                for wd in line.Words()? {
                    let r = wd.BoundingRect()?;
                    words.push((wd.Text()?.to_string(), [r.X / bw, r.Y / bh, r.Width / bw, r.Height / bh]));
                }
                out.push(TextLine { words });
            }
            Ok(out)
        };
        run().map_err(|e| e.message().to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (path, index);
        Ok(vec![])
    }
}

/// Temuan kueri dalam baris: kotak kata-kata yang tersentuh rentang temuan (tanpa membedakan huruf).
pub fn find_in_lines(lines: &[TextLine], q: &str) -> Vec<Vec<[f32; 4]>> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    let mut hits = vec![];
    for l in lines {
        let mut text = String::new();
        let mut spans = vec![];
        for (w, b) in &l.words {
            if !text.is_empty() {
                text.push(' ');
            }
            let s = text.len();
            text.push_str(&w.to_lowercase());
            spans.push((s, text.len(), *b));
        }
        let mut from = 0;
        while let Some(k) = text[from..].find(&q) {
            let (a, z) = (from + k, from + k + q.len());
            hits.push(spans.iter().filter(|(s, e, _)| *s < z && *e > a).map(|x| x.2).collect());
            from = z;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    #[test]
    fn cari_lintas_kata() {
        let l = super::TextLine { words: vec![("Halo".into(), [0., 0., 0.1, 0.1]), ("Dunia".into(), [0.2, 0., 0.1, 0.1]), ("halo".into(), [0.4, 0., 0.1, 0.1])] };
        let h = super::find_in_lines(&[l], "o du");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].len(), 2);
        assert_eq!(super::find_in_lines(&[super::TextLine::default()], "x").len(), 0);
    }

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

#[cfg(all(test, windows))]
mod ocr_tests {
    #[test]
    fn ocr_membaca_teks_halaman() {
        let content = "BT /F1 36 Tf 20 40 Td (Hello Zandor) Tj ET";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 100] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
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
        let p = std::env::temp_dir().join(format!("zandor-ocr-{}.pdf", std::process::id()));
        std::fs::write(&p, out).unwrap();
        let lines = super::page_text(&p.to_string_lossy(), 0).unwrap();
        let _ = std::fs::remove_file(&p);
        eprintln!("{lines:?}");
        assert_eq!(super::find_in_lines(&lines, "zandor").len(), 1);
    }
}
