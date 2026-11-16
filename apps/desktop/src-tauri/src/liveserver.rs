// Live server lokal untuk pratinjau HTML di Browser Panel 4: menyajikan folder project (statis, GET/HEAD) di
// 127.0.0.1 port acak, dan menyisipkan skrip kecil ke halaman HTML yang memuat ulang halaman saat berkas di folder
// berubah. Satu server per folder project, hidup sampai aplikasi ditutup.
//
// Keamanan: hanya mendengar di loopback; path dinormalisasi dan wajib tetap di dalam folder (tanpa `..`, symlink
// keluar ditolak lewat kanonisasi). Akses butuh token acak: permintaan pertama membawa `?__zl=<token>` lalu server
// memasang cookie `SameSite=Strict; HttpOnly`, sehingga situs internet lain tidak bisa membaca berkas project
// meskipun mengetahui port-nya.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LIVE_PATH: &str = "/__zandor_live";
const MAX_SERVE_BYTES: u64 = 200 * 1024 * 1024;
const MAX_WALK: usize = 20_000;

#[derive(Clone)]
struct Server {
    port: u16,
    token: String,
}

fn servers() -> &'static Mutex<HashMap<PathBuf, Server>> {
    static S: OnceLock<Mutex<HashMap<PathBuf, Server>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn new_token() -> String {
    // Cukup acak untuk token lokal: waktu nano + alamat stack + penghitung, di-hash dua kali.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut out = String::new();
    for round in 0..2u8 {
        let mut h = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            .hash(&mut h);
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .hash(&mut h);
        (&h as *const _ as usize).hash(&mut h);
        std::process::id().hash(&mut h);
        round.hash(&mut h);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Mulai (atau pakai ulang) server untuk `root` dan kembalikan URL halaman `rel` lengkap dengan token.
pub fn start(root: &Path, rel: &str) -> Result<String, String> {
    let root = crate::files::dunce(root).ok_or("folder tidak ditemukan")?;
    let mut map = servers().lock().map_err(|e| e.to_string())?;
    let srv = match map.get(&root) {
        Some(s) => s.clone(),
        None => {
            let listener =
                TcpListener::bind("127.0.0.1:0").map_err(|e| format!("gagal membuka port: {e}"))?;
            let port = listener.local_addr().map_err(|e| e.to_string())?.port();
            let srv = Server {
                port,
                token: new_token(),
            };
            let (r, tok) = (root.clone(), srv.token.clone());
            std::thread::Builder::new()
                .name(format!("zandor-live-{port}"))
                .spawn(move || serve(listener, r, tok))
                .map_err(|e| e.to_string())?;
            map.insert(root.clone(), srv.clone());
            srv
        }
    };
    let path: String = rel
        .replace('\\', "/")
        .split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/");
    Ok(format!(
        "http://127.0.0.1:{}/{}?__zl={}",
        srv.port, path, srv.token
    ))
}

fn encode_segment(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn serve(listener: TcpListener, root: PathBuf, token: String) {
    for conn in listener.incoming().flatten() {
        let (root, token) = (root.clone(), token.clone());
        let _ = std::thread::Builder::new().spawn(move || {
            let _ = conn.set_read_timeout(Some(Duration::from_secs(10)));
            let Ok(mut done) = conn.try_clone() else {
                return;
            };
            let _ = handle(conn, &root, &token);
            // Tutup rapi: tanpa ini Windows bisa mengirim RST dan klien kehilangan respons yang sudah ditulis.
            let _ = done.shutdown(std::net::Shutdown::Write);
            let _ = done.set_read_timeout(Some(Duration::from_millis(500)));
            let mut sink = [0u8; 1024];
            while matches!(done.read(&mut sink), Ok(n) if n > 0) {}
        });
    }
}

struct Req {
    method: String,
    path: String,
    query: String,
    cookie_ok: bool,
}

fn parse(stream: &TcpStream, token: &str) -> Option<Req> {
    let mut r = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut cookie_ok = false;
    let mut total = 0usize;
    loop {
        let mut h = String::new();
        let n = r.read_line(&mut h).ok()?;
        total += n;
        if n == 0 || h == "\r\n" || h == "\n" || total > 64 * 1024 {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.trim().eq_ignore_ascii_case("cookie") {
                cookie_ok |= v.split(';').any(|c| c.trim() == format!("zl={token}"));
            }
        }
    }
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Some(Req {
        method,
        path,
        query,
        cookie_ok,
    })
}

/// Path URL → berkas di dalam `root`. None bila keluar folder atau tidak valid.
pub(crate) fn resolve(root: &Path, url_path: &str) -> Option<PathBuf> {
    let p = decode(url_path)?;
    if p.contains('\0') {
        return None;
    }
    let rel = Path::new(p.trim_start_matches('/'));
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let mut full = root.join(rel);
    if full.is_dir() {
        full = full.join("index.html");
    }
    let canon = crate::files::dunce(&full)?;
    canon.starts_with(root).then_some(canon)
}

fn content_type(p: &Path) -> &'static str {
    match p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs" | "cjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("wasm") => "application/wasm",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Versi folder = waktu modifikasi terbaru (abaikan .git, node_modules, target). Dipakai skrip live reload.
pub(crate) fn version(root: &Path) -> u128 {
    let mut best = 0u128;
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            seen += 1;
            if seen > MAX_WALK {
                return best;
            }
            let name = e.file_name();
            let name = name.to_string_lossy();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if !matches!(
                    name.as_ref(),
                    ".git" | "node_modules" | "target" | "dist" | ".next"
                ) {
                    stack.push(e.path());
                }
                continue;
            }
            if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                let t = m
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                best = best.max(t);
            }
        }
    }
    best
}

const RELOAD_SCRIPT: &str = r#"<script>(()=>{let v=null;const tick=async()=>{try{const r=await fetch("/__zandor_live",{cache:"no-store"});const n=await r.text();if(v!==null&&n!==v){location.reload();return}v=n}catch{}setTimeout(tick,800)};tick()})();</script>"#;

pub(crate) fn inject(html: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(html);
    let lower = s.to_ascii_lowercase();
    match lower.rfind("</body>") {
        Some(i) => format!("{}{}{}", &s[..i], RELOAD_SCRIPT, &s[i..]).into_bytes(),
        None => format!("{s}{RELOAD_SCRIPT}").into_bytes(),
    }
}

fn respond(
    s: &mut TcpStream,
    status: &str,
    headers: &[(&str, String)],
    body: &[u8],
    head: bool,
) -> std::io::Result<()> {
    let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n", body.len());
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    s.write_all(out.as_bytes())?;
    if !head {
        s.write_all(body)?;
    }
    s.flush()
}

fn handle(mut s: TcpStream, root: &Path, token: &str) -> std::io::Result<()> {
    let Some(req) = parse(&s, token) else {
        return Ok(());
    };
    let head = req.method == "HEAD";
    if req.method != "GET" && !head {
        return respond(&mut s, "405 Method Not Allowed", &[], b"", head);
    }
    let via_query = req.query.split('&').any(|kv| kv == format!("__zl={token}"));
    if !req.cookie_ok && !via_query {
        return respond(
            &mut s,
            "403 Forbidden",
            &[],
            "Akses ditolak: buka lewat Zandor.".as_bytes(),
            head,
        );
    }
    let mut headers: Vec<(&str, String)> = Vec::new();
    if via_query {
        headers.push((
            "Set-Cookie",
            format!("zl={token}; Path=/; HttpOnly; SameSite=Strict"),
        ));
    }
    if req.path == LIVE_PATH {
        headers.push(("Content-Type", "text/plain".into()));
        return respond(
            &mut s,
            "200 OK",
            &headers,
            version(root).to_string().as_bytes(),
            head,
        );
    }
    let Some(file) = resolve(root, &req.path) else {
        return respond(&mut s, "404 Not Found", &headers, b"Not found", head);
    };
    let Ok(mut f) = std::fs::File::open(&file) else {
        return respond(&mut s, "404 Not Found", &headers, b"Not found", head);
    };
    if f.metadata().map(|m| m.len()).unwrap_or(0) > MAX_SERVE_BYTES {
        return respond(&mut s, "413 Payload Too Large", &headers, b"", head);
    }
    let mut body = Vec::new();
    f.read_to_end(&mut body)?;
    let ct = content_type(&file);
    if ct.starts_with("text/html") {
        body = inject(&body);
    }
    headers.push(("Content-Type", ct.into()));
    respond(&mut s, "200 OK", &headers, &body, head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("zl-test-{}", new_token()));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(
            d.join("index.html"),
            "<html><body><link href=\"style.css\"></body></html>",
        )
        .unwrap();
        std::fs::write(d.join("style.css"), "body{}").unwrap();
        std::fs::write(d.join("sub/a.html"), "<p>a</p>").unwrap();
        crate::files::dunce(&d).unwrap()
    }

    fn get(url: &str, cookie: Option<&str>) -> (String, String) {
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        let mut s = TcpStream::connect(host).unwrap();
        let c = cookie
            .map(|c| format!("Cookie: {c}\r\n"))
            .unwrap_or_default();
        write!(s, "GET /{path} HTTP/1.1\r\nHost: {host}\r\n{c}\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let (h, b) = out.split_once("\r\n\r\n").unwrap();
        (h.to_string(), b.to_string())
    }

    #[test]
    fn resolve_stays_inside_root() {
        let d = tmp();
        assert!(resolve(&d, "/index.html").is_some());
        assert!(resolve(&d, "/").is_some()); // index.html folder
        assert!(resolve(&d, "/sub/a.html").is_some());
        assert!(resolve(&d, "/../etc/passwd").is_none());
        assert!(resolve(&d, "/%2e%2e/x").is_none());
        assert!(resolve(&d, "/nope.css").is_none());
    }

    #[test]
    fn inject_adds_reload_script_before_body_end() {
        let out = String::from_utf8(inject(b"<html><BODY>x</BODY></html>")).unwrap();
        assert!(out.contains("__zandor_live"));
        assert!(out.find("__zandor_live").unwrap() < out.find("</BODY>").unwrap());
        assert!(String::from_utf8(inject(b"<p>x</p>"))
            .unwrap()
            .ends_with("</script>"));
    }

    #[test]
    fn serves_with_token_and_rejects_without() {
        let d = tmp();
        let url = start(&d, "index.html").unwrap();
        let (h, b) = get(&url, None);
        assert!(h.starts_with("HTTP/1.1 200"), "{h}");
        assert!(h.contains("Set-Cookie: zl="));
        assert!(b.contains("__zandor_live"));
        let base = url
            .split('?')
            .next()
            .unwrap()
            .trim_end_matches("index.html")
            .to_string();
        let token = url.rsplit("__zl=").next().unwrap();
        // aset relatif lewat cookie
        let (h, b) = get(&format!("{base}style.css"), Some(&format!("zl={token}")));
        assert!(h.contains("text/css"), "{h}");
        assert_eq!(b, "body{}");
        // tanpa token/cookie → 403
        let (h, _) = get(&format!("{base}style.css"), None);
        assert!(h.starts_with("HTTP/1.1 403"), "{h}");
        // server dipakai ulang untuk folder yang sama
        assert_eq!(
            start(&d, "sub/a.html").unwrap().split('/').nth(2),
            url.split('/').nth(2)
        );
        // versi berubah setelah berkas diubah
        let v1 = version(&d);
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(d.join("style.css"), "body{color:red}").unwrap();
        assert!(version(&d) >= v1);
    }
}
