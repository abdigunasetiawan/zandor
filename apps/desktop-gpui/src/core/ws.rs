//! Klien WebSocket minimal (RFC 6455) untuk I/O terminal PTY engine: `ws://` (engine lokal) di atas TCP polos dan
//! `wss://` (engine remote HTTPS) lewat native-tls. Handshake HTTP Upgrade, bingkai klien bermasker, pembacaan bingkai
//! teks/biner/close/ping.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// Aliran koneksi: TCP polos (klon terpisah untuk baca/tulis) atau TLS bersama. TLS tidak bisa dipisah, jadi pembaca
/// memakai batas waktu baca pendek agar kunci dilepas berkala dan penulis di thread lain bisa menulis.
pub enum Stream {
    Plain(TcpStream),
    Tls(Arc<Mutex<native_tls::TlsStream<TcpStream>>>, TcpStream),
}

impl Stream {
    fn try_clone(&self) -> std::io::Result<Stream> {
        Ok(match self {
            Stream::Plain(s) => Stream::Plain(s.try_clone()?),
            Stream::Tls(t, raw) => Stream::Tls(t.clone(), raw.try_clone()?),
        })
    }
    fn shutdown(&self) {
        let _ = match self {
            Stream::Plain(s) => s.shutdown(std::net::Shutdown::Both),
            Stream::Tls(_, raw) => raw.shutdown(std::net::Shutdown::Both),
        };
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(t, _) => loop {
                let r = t.lock().map_err(|_| std::io::Error::other("kunci TLS rusak"))?.read(buf);
                match r {
                    Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => std::thread::sleep(std::time::Duration::from_millis(2)),
                    other => return other,
                }
            },
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(t, _) => t.lock().map_err(|_| std::io::Error::other("kunci TLS rusak"))?.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(t, _) => t.lock().map_err(|_| std::io::Error::other("kunci TLS rusak"))?.flush(),
        }
    }
}

/// (tls, host:port, host tanpa port, path) dari URL ws:// atau wss://.
pub fn parse_ws_url(url: &str) -> Result<(bool, String, String, String), String> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        return Err("hanya ws:// atau wss:// yang didukung".into());
    };
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let name = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map(|(h, _)| h).unwrap_or(host).to_string();
    let addr = if name.len() != host.len() { host.to_string() } else { format!("{host}:{}", if tls { 443 } else { 80 }) };
    Ok((tls, addr, name, path.to_string()))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(data: &[u8]) -> String {
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub enum Frame {
    Text(Vec<u8>),
    Binary(Vec<u8>),
    Close,
}

pub struct WsReader {
    r: BufReader<Stream>,
    w: Stream,
}

/// Penulis: tiap `write` = satu bingkai teks bermasker (input terminal).
pub struct WsWriter(Stream);

fn mask_key() -> [u8; 4] {
    let mut k = [0u8; 4];
    let _ = getrandom::fill(&mut k);
    k
}

fn write_frame(s: &mut Stream, opcode: u8, data: &[u8]) -> std::io::Result<()> {
    let mut f = vec![0x80 | opcode];
    let n = data.len();
    if n < 126 {
        f.push(0x80 | n as u8);
    } else if n < 65536 {
        f.push(0x80 | 126);
        f.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        f.push(0x80 | 127);
        f.extend_from_slice(&(n as u64).to_be_bytes());
    }
    let k = mask_key();
    f.extend_from_slice(&k);
    f.extend(data.iter().enumerate().map(|(i, b)| b ^ k[i % 4]));
    s.write_all(&f)
}

impl Write for WsWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write_frame(&mut self.0, 0x1, buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
impl Drop for WsWriter {
    /// Menutup penulis menutup koneksi; pembaca di thread lain ikut berhenti.
    fn drop(&mut self) {
        let _ = write_frame(&mut self.0, 0x8, &[]);
        self.0.shutdown();
    }
}

/// Sambung ke `ws://host:port/path?query`; header tambahan (mis. Authorization) opsional.
pub fn connect(url: &str, headers: &[(&str, &str)]) -> Result<(WsReader, WsWriter), String> {
    let (tls, addr, name, path) = parse_ws_url(url)?;
    let host = addr.as_str();
    let tcp = TcpStream::connect(&addr).map_err(|e| e.to_string())?;
    tcp.set_nodelay(true).ok();
    let mut s = if tls {
        let raw = tcp.try_clone().map_err(|e| e.to_string())?;
        let t = native_tls::TlsConnector::new().map_err(|e| e.to_string())?.connect(&name, tcp).map_err(|e| format!("TLS: {e}"))?;
        // batas waktu baca pendek setelah handshake: pembaca melepas kunci berkala (lihat Stream)
        raw.set_read_timeout(Some(std::time::Duration::from_millis(20))).ok();
        Stream::Tls(Arc::new(Mutex::new(t)), raw)
    } else {
        Stream::Plain(tcp)
    };
    let mut key = [0u8; 16];
    let _ = getrandom::fill(&mut key);
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n", base64(&key));
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let w = s.try_clone().map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let mut line = String::new();
    r.read_line(&mut line).map_err(|e| e.to_string())?;
    if !line.contains(" 101 ") {
        return Err(format!("handshake ditolak: {}", line.trim()));
    }
    loop {
        line.clear();
        r.read_line(&mut line).map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            break;
        }
    }
    let w2 = w.try_clone().map_err(|e| e.to_string())?;
    Ok((WsReader { r, w: w2 }, WsWriter(w)))
}

impl WsReader {
    /// Bingkai berikutnya (fragmen digabung; ping dijawab pong). Err = koneksi putus.
    pub fn next(&mut self) -> std::io::Result<Frame> {
        let mut msg: Vec<u8> = vec![];
        let mut kind = 0u8;
        loop {
            let mut h = [0u8; 2];
            self.r.read_exact(&mut h)?;
            let (fin, op) = (h[0] & 0x80 != 0, h[0] & 0x0f);
            let masked = h[1] & 0x80 != 0;
            let mut n = (h[1] & 0x7f) as u64;
            if n == 126 {
                let mut b = [0u8; 2];
                self.r.read_exact(&mut b)?;
                n = u16::from_be_bytes(b) as u64;
            } else if n == 127 {
                let mut b = [0u8; 8];
                self.r.read_exact(&mut b)?;
                n = u64::from_be_bytes(b);
            }
            let mut k = [0u8; 4];
            if masked {
                self.r.read_exact(&mut k)?;
            }
            let mut data = vec![0u8; n as usize];
            self.r.read_exact(&mut data)?;
            if masked {
                for (i, b) in data.iter_mut().enumerate() {
                    *b ^= k[i % 4];
                }
            }
            match op {
                0x8 => return Ok(Frame::Close),
                0x9 => {
                    write_frame(&mut self.w, 0xA, &data)?;
                    continue;
                }
                0xA => continue,
                0x0 => msg.extend(data),
                o => {
                    kind = o;
                    msg = data;
                }
            }
            if fin {
                return Ok(if kind == 0x1 { Frame::Text(msg) } else { Frame::Binary(msg) });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_standar() {
        assert_eq!(super::base64(b"hello"), "aGVsbG8=");
        assert_eq!(super::base64(b"ab"), "YWI=");
        assert_eq!(super::base64(b"abc"), "YWJj");
    }
}

/// Uji nyata terhadap engine: `ZANDOR_TEST_ENGINE=<url> ZANDOR_TEST_PASS=<pw> cargo test pty_engine -- --ignored`.
#[cfg(test)]
mod engine_tests {
    #[test]
    #[ignore]
    fn pty_engine_echo() {
        use crate::core::engine::{ConnectionInfo, EngineApi};
        let url = std::env::var("ZANDOR_TEST_ENGINE").unwrap();
        let api = EngineApi::new(ConnectionInfo { url, username: "opencode".into(), password: std::env::var("ZANDOR_TEST_PASS").unwrap() });
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let p = api.pty_create(&dir, "cmd.exe", &[], Some("uji")).unwrap();
        let u = api.pty_connect_url(&dir, &p.id, 0).unwrap();
        let (mut r, mut w) = super::connect(&u, &[]).unwrap();
        use std::io::Write;
        w.write_all(b"echo ZANDOR_OK\r").unwrap();
        let mut got = String::new();
        for _ in 0..200 {
            match r.next().unwrap() {
                super::Frame::Text(d) | super::Frame::Binary(d) => got.push_str(&String::from_utf8_lossy(&d)),
                super::Frame::Close => break,
            }
            if got.matches("ZANDOR_OK").count() >= 2 {
                break;
            }
        }
        assert!(api.pty_list(&dir).unwrap().iter().any(|x| x.id == p.id));
        api.pty_remove(&dir, &p.id).unwrap();
        assert!(got.matches("ZANDOR_OK").count() >= 2, "{got}");
    }
}

#[cfg(test)]
mod url_tests {
    #[test]
    fn url_ws_dan_wss() {
        assert_eq!(super::parse_ws_url("ws://127.0.0.1:4096/pty/x").unwrap(), (false, "127.0.0.1:4096".into(), "127.0.0.1".into(), "/pty/x".into()));
        assert_eq!(super::parse_ws_url("wss://eng.example.com/a?b=1").unwrap(), (true, "eng.example.com:443".into(), "eng.example.com".into(), "/a?b=1".into()));
        assert!(super::parse_ws_url("http://x").is_err());
    }

    /// Uji nyata TLS + handshake WebSocket ke server echo publik: `cargo test wss_nyata -- --ignored`.
    #[test]
    #[ignore]
    fn wss_nyata() {
        let (mut r, mut w) = super::connect("wss://echo.websocket.org/", &[]).unwrap();
        use std::io::Write;
        w.write_all(b"zandor").unwrap();
        for _ in 0..5 {
            if let super::Frame::Text(d) = r.next().unwrap() {
                if d == b"zandor" {
                    return;
                }
            }
        }
        panic!("tidak ada echo");
    }
}
