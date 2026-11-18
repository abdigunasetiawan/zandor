//! Terminal — PTY lokal (portable-pty, ConPTY di Windows) + emulator VT (vt100) digambar dengan GPUI.
//! Pengganti panel Terminal berbasis xterm.js; ukuran font dari pengaturan `terminalFontSize`.
use crate::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, FocusHandle, Focusable, FontWeight, HighlightStyle, Hsla, IntoElement,
    KeyDownEvent, Pixels, StyledText, Window,
};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub struct Terminal {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    master: Option<Box<dyn MasterPty + Send>>,
    focus: FocusHandle,
    size: (u16, u16),
    error: Option<String>,
    exited: Arc<std::sync::atomic::AtomicBool>,
}

fn default_shell() -> CommandBuilder {
    if cfg!(windows) {
        let mut c = CommandBuilder::new("powershell.exe");
        c.arg("-NoLogo");
        c
    } else {
        CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()))
    }
}

impl Terminal {
    pub fn new(cwd: Option<String>, cx: &mut Context<Self>) -> Self {
        Self::with_shell(cwd, None, cx)
    }

    /// Shell tertentu (path exe + argumen); None = shell bawaan.
    pub fn with_shell(cwd: Option<String>, shell: Option<(String, Vec<String>)>, cx: &mut Context<Self>) -> Self {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 5000)));
        let mut me = Self {
            parser: parser.clone(),
            writer: Arc::new(Mutex::new(None)),
            master: None,
            focus: cx.focus_handle(),
            size: (24, 80),
            error: None,
            exited: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let pty = native_pty_system();
        let pair = match pty.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }) {
            Ok(p) => p,
            Err(e) => {
                me.error = Some(e.to_string());
                return me;
            }
        };
        let mut cmd = match shell {
            Some((exe, args)) => {
                let mut c = CommandBuilder::new(exe);
                c.args(args);
                c
            }
            None => default_shell(),
        };
        if let Some(d) = cwd.filter(|d| std::path::Path::new(d).is_dir()) {
            cmd.cwd(d);
        }
        cmd.env("TERM", "xterm-256color");
        let mut child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                me.error = Some(e.to_string());
                return me;
            }
        };
        drop(pair.slave);
        let mut reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => {
                me.error = Some(e.to_string());
                return me;
            }
        };
        *me.writer.lock().unwrap() = pair.master.take_writer().ok();
        let writer = me.writer.clone();
        me.master = Some(pair.master);
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
        let exited = me.exited.clone();
        std::thread::Builder::new()
            .name("zandor-pty".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut p) = parser.lock() {
                                p.process(&buf[..n]);
                                answer_queries(&buf[..n], &p, &writer);
                            }
                            let _ = tx.unbounded_send(());
                        }
                    }
                }
                let _ = child.wait();
                exited.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = tx.unbounded_send(());
            })
            .ok();
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while rx.next().await.is_some() {
                // gabungkan ledakan output menjadi satu render
                while rx.try_recv().is_ok() {}
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        me
    }

    pub fn exited(&self) -> bool {
        self.exited.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn send(&mut self, bytes: &[u8]) {
        if let Some(w) = self.writer.lock().unwrap().as_mut() {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if (rows, cols) == self.size || rows < 2 || cols < 10 {
            return;
        }
        self.size = (rows, cols);
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(rows, cols);
        }
        if let Some(m) = &self.master {
            let _ = m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = &k.modifiers;
        let app_cursor = self.parser.lock().map(|p| p.screen().application_cursor()).unwrap_or(false);
        let arrow = |c: char| if app_cursor { format!("\x1bO{c}") } else { format!("\x1b[{c}") };
        let bytes: Vec<u8> = match k.key.as_str() {
            "enter" => b"\r".to_vec(),
            "backspace" => b"\x7f".to_vec(),
            "tab" => if m.shift { b"\x1b[Z".to_vec() } else { b"\t".to_vec() },
            "escape" => b"\x1b".to_vec(),
            "up" => arrow('A').into_bytes(),
            "down" => arrow('B').into_bytes(),
            "right" => arrow('C').into_bytes(),
            "left" => arrow('D').into_bytes(),
            "home" => b"\x1b[H".to_vec(),
            "end" => b"\x1b[F".to_vec(),
            "delete" => b"\x1b[3~".to_vec(),
            "pageup" => b"\x1b[5~".to_vec(),
            "pagedown" => b"\x1b[6~".to_vec(),
            "v" if m.control && m.shift => {
                if let Some(t) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let bracket = self.parser.lock().map(|p| p.screen().bracketed_paste()).unwrap_or(false);
                    let mut v = vec![];
                    if bracket {
                        v.extend_from_slice(b"\x1b[200~");
                    }
                    v.extend_from_slice(t.as_bytes());
                    if bracket {
                        v.extend_from_slice(b"\x1b[201~");
                    }
                    v
                } else {
                    vec![]
                }
            }
            "c" if m.control && m.shift => {
                let text = self.parser.lock().map(|p| p.screen().contents()).unwrap_or_default();
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                vec![]
            }
            key if m.control && key.len() == 1 => {
                let c = key.as_bytes()[0].to_ascii_lowercase();
                if c.is_ascii_lowercase() { vec![c - b'a' + 1] } else { vec![] }
            }
            _ => match &k.key_char {
                Some(s) if !m.control => {
                    let mut v = vec![];
                    if m.alt {
                        v.push(0x1b);
                    }
                    v.extend_from_slice(s.as_bytes());
                    v
                }
                _ => vec![],
            },
        };
        if !bytes.is_empty() {
            self.send(&bytes);
            cx.stop_propagation();
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Menutup master mengakhiri sesi ConPTY; shell ikut keluar.
        if let Ok(mut w) = self.writer.lock() {
            *w = None;
        }
        self.master = None;
    }
}

/// Jawab kueri terminal yang ditunggu ConPTY/aplikasi: DSR posisi kursor (ESC[6n) dan atribut perangkat (ESC[c).
fn answer_queries(chunk: &[u8], p: &vt100::Parser, writer: &Arc<Mutex<Option<Box<dyn Write + Send>>>>) {
    let has = |pat: &[u8]| chunk.windows(pat.len()).any(|w| w == pat);
    let mut reply = Vec::new();
    if has(b"[6n") {
        let (r, c) = p.screen().cursor_position();
        reply.extend_from_slice(format!("[{};{}R", r + 1, c + 1).as_bytes());
    }
    if has(b"[c") || has(b"[0c") {
        reply.extend_from_slice(b"[?1;2c");
    }
    if !reply.is_empty() {
        if let Ok(mut w) = writer.lock() {
            if let Some(w) = w.as_mut() {
                let _ = w.write_all(&reply);
                let _ = w.flush();
            }
        }
    }
}

impl Focusable for Terminal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Palet ANSI 16 warna (gelap) + kubus 256.
fn ansi(t: &Theme, c: vt100::Color, fg: bool) -> Option<Hsla> {
    const BASE: [u32; 16] = [
        0x1e1e1e, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xd4d4d4, 0x5c6370, 0xff7b86, 0xb5e890, 0xffd68a,
        0x7cc4ff, 0xde96f0, 0x76d7e3, 0xffffff,
    ];
    let rgb = |v: u32| Some(gpui::rgb(v).into());
    match c {
        vt100::Color::Default => fg.then_some(t.colors.fg),
        vt100::Color::Rgb(r, g, b) => rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32),
        vt100::Color::Idx(i) if i < 16 => rgb(BASE[i as usize]),
        vt100::Color::Idx(i) if i < 232 => {
            let i = i as u32 - 16;
            let s = |x: u32| if x == 0 { 0 } else { 55 + x * 40 };
            rgb((s(i / 36) << 16) | (s((i / 6) % 6) << 8) | s(i % 6))
        }
        vt100::Color::Idx(i) => {
            let v = 8 + (i as u32 - 232) * 10;
            rgb((v << 16) | (v << 8) | v)
        }
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let fs = crate::core::settings::SettingsStore::global(cx).read(cx).num("terminalFontSize").max(8.) as f32 * t.scale;
        let line_h = (fs * 1.35).round();
        let cell_w = fs * 0.6;
        if let Some(e) = &self.error {
            return div().p(t.space(3)).text_color(t.colors.danger).child(e.clone()).into_any_element();
        }
        let focused = self.focus.is_focused(window);
        let mut rows_el = vec![];
        if let Ok(p) = self.parser.lock() {
            let s = p.screen();
            let (rows, cols) = s.size();
            let (cr, cc) = s.cursor_position();
            let show_cursor = !s.hide_cursor() && focused;
            for r in 0..rows {
                let mut text = String::new();
                let mut hl: Vec<(std::ops::Range<usize>, HighlightStyle)> = vec![];
                for c in 0..cols {
                    let Some(cell) = s.cell(r, c) else { continue };
                    if cell.is_wide_continuation() {
                        continue;
                    }
                    let start = text.len();
                    let ch = cell.contents();
                    text.push_str(if ch.is_empty() { " " } else { &ch });
                    let (mut fg, mut bg) = (ansi(&t, cell.fgcolor(), true), ansi(&t, cell.bgcolor(), false));
                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                        fg = fg.or(Some(t.colors.bg));
                        bg = bg.or(Some(t.colors.fg));
                    }
                    if show_cursor && r == cr && c == cc {
                        fg = Some(t.colors.bg);
                        bg = Some(t.colors.fg);
                    }
                    let h = HighlightStyle {
                        color: fg,
                        background_color: bg,
                        font_weight: cell.bold().then_some(FontWeight::BOLD),
                        font_style: cell.italic().then_some(gpui::FontStyle::Italic),
                        underline: cell.underline().then(|| gpui::UnderlineStyle { thickness: px(1.), color: fg, wavy: false }),
                        ..Default::default()
                    };
                    hl.push((start..text.len(), h));
                }
                rows_el.push(div().h(px(line_h)).whitespace_nowrap().child(StyledText::new(text).with_highlights(hl)));
            }
        }
        let me = cx.entity().downgrade();
        let exited = self.exited.load(std::sync::atomic::Ordering::Relaxed);
        div()
            .id("terminal")
            .key_context("Terminal")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|s, ev: &KeyDownEvent, _, cx| s.on_key(ev, cx)))
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|s, _, w, cx| {
                w.focus(&s.focus, cx);
                cx.notify();
            }))
            .relative()
            .size_full()
            .overflow_hidden()
            .p(t.space(2))
            .bg(t.colors.bg)
            .font_family(t.font_terminal.clone())
            .text_size(px(fs))
            .line_height(px(line_h))
            .text_color(t.colors.fg)
            .child(
                canvas(
                    move |b: Bounds<Pixels>, _, cx| {
                        let w: f32 = b.size.width.into();
                        let h: f32 = b.size.height.into();
                        let (rows, cols) = ((h / line_h).floor() as u16, (w / cell_w).floor() as u16);
                        me.update(cx, |s, _| s.resize(rows, cols)).ok();
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .children(rows_el)
            .when(exited, |d| d.child(div().text_color(t.colors.fg_subtle).child("[proses selesai]")))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pty_menjalankan_perintah_dan_vt_mengurai() {
        let pair = native_pty_system().openpty(PtySize { rows: 10, cols: 60, pixel_width: 0, pixel_height: 0 }).unwrap();
        let mut cmd = if cfg!(windows) { CommandBuilder::new("cmd.exe") } else { CommandBuilder::new("sh") };
        if cfg!(windows) {
            cmd.args(["/c", "echo zandor-pty-ok"]);
        } else {
            cmd.args(["-c", "echo zandor-pty-ok"]);
        }
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer: Arc<Mutex<Option<Box<dyn Write + Send>>>> = Arc::new(Mutex::new(pair.master.take_writer().ok()));
        let mut parser = vt100::Parser::new(10, 60, 0);
        let start = std::time::Instant::now();
        let mut buf = [0u8; 4096];
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        while start.elapsed() < std::time::Duration::from_secs(10) {
            if let Ok(b) = rx.recv_timeout(std::time::Duration::from_millis(200)) {
                parser.process(&b);
                answer_queries(&b, &parser, &writer);
            }
            if parser.screen().contents().contains("zandor-pty-ok") {
                break;
            }
        }
        let _ = child.kill();
        assert!(parser.screen().contents().contains("zandor-pty-ok"), "{}", parser.screen().contents());
    }
}
