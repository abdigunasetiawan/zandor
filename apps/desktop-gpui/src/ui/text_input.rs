//! Input teks satu baris — diadaptasi dari gpui/examples/input.rs (Zed Industries, Apache-2.0),
//! gaya disesuaikan dengan .z-input di atoms.css.
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, Keystroke, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, WindowBounds,
    WindowOptions, actions, black, div, fill, hsla, opaque_grey, point, prelude::*, px, relative,
    rgb, rgba, size, white, yellow,
};
use crate::theme::Theme;
use unicode_segmentation::*;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Quit,
        Submit,
        CtrlSubmit,
        PastePlain,
        Newline,
        Tab,
        Escape,
        Up,
        Down,
    ]
);

pub struct TextInput {
    focus_handle: FocusHandle,
    blur_sub: Option<gpui::Subscription>,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    pub size: super::ControlSize,
    pub invalid: bool,
    pub disabled: bool,
    /// Tampilkan isi sebagai bintang (kunci API, sandi).
    pub masked: bool,
    /// Banyak baris (baris keras `\n`, tanpa bungkus lunak). Shift+Enter = baris baru.
    pub multiline: bool,
    /// Bila multiline: Enter juga menyisipkan baris baru (editor), bukan mengirim.
    pub enter_newline: bool,
    /// Pengaturan sendKey = "ctrl-enter": Ctrl/Cmd+Enter mengirim, Enter menyisipkan baris baru.
    pub ctrl_enter_sends: bool,
    /// Pengaturan pasteLongText = "attach": teks panjang yang ditempel dipancarkan sebagai `InputLongPaste`
    /// (dijadikan lampiran oleh pemilik) alih-alih disisipkan. Ctrl+Shift+V = tempel apa adanya.
    pub long_paste_attach: bool,
    last_lines: Vec<(usize, ShapedLine)>,
    /// Bila true, ↑ ↓ Enter Tab Esc dipancarkan sebagai `InputNav` (mis. popup saran terbuka).
    pub intercept: bool,
    /// Tanpa border/latar/padding/ring fokus (port `.composer__input`): dipakai di dalam kartu composer.
    pub bare: bool,
    /// Kursor halus ala Monkeytype (port `.composer__caret`): batang meluncur ke posisi baru, berkedip saat diam,
    /// menyala terus saat mengetik.
    pub smooth_caret: bool,
    caret: CaretAnim,
}

/// Status animasi kursor halus.
#[derive(Default)]
struct CaretAnim {
    /// Posisi asal & tujuan (relatif terhadap bounds elemen) dan awal luncuran.
    from: Option<Point<Pixels>>,
    to: Option<Point<Pixels>>,
    start: Option<std::time::Instant>,
    /// Waktu isi terakhir berubah (mengetik → tidak berkedip).
    typed: Option<std::time::Instant>,
    last_content: SharedString,
    /// Bangun tertunda yang sudah dijadwalkan (fase kedip berikutnya) — hindari frame terus-menerus saat diam.
    wake_at: Option<std::time::Instant>,
}

/// `.composer__caret`: transform calc(--z-duration-fast * 1.1) var(--z-ease).
const CARET_SLIDE_MS: f32 = crate::theme::DURATION_FAST_MS as f32 * 1.1 + 70.;
/// Setelah berhenti mengetik selama ini, kursor mulai berkedip (siklus 1 s: 0–40% nyala, 60–100% padam).
const CARET_TYPING_MS: u128 = 500;

/// Tombol navigasi yang disalurkan ke pemilik input saat `intercept` aktif.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputNav {
    Up,
    Down,
    Enter,
    Tab,
    Escape,
}
impl gpui::EventEmitter<InputNav> for TextInput {}

impl TextInput {
    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.start, window, cx)
        }
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), window, cx);
        } else {
            self.move_to(self.selected_range.end, window, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
        self.select_to(self.content.len(), window, cx)
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), window, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        window.request_virtual_keyboard();

        if event.modifiers.shift {
            self.select_to(if self.multiline { self.line_index_at(event.position, window) } else { self.index_for_mouse_position(event.position) }, window, cx);
        } else {
            self.move_to(if self.multiline { self.line_index_at(event.position, window) } else { self.index_for_mouse_position(event.position) }, window, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(if self.multiline { self.line_index_at(event.position, window) } else { self.index_for_mouse_position(event.position) }, window, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_text(false, window, cx);
    }
    fn paste_plain(&mut self, _: &PastePlain, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_text(true, window, cx);
    }
    fn paste_text(&mut self, plain: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else { return };
        if !plain && self.long_paste_attach && is_long_paste(&text) {
            cx.emit(InputLongPaste(text.into()));
            return;
        }
        // multiline mempertahankan baris baru (dinormalisasi); satu baris: baris baru jadi spasi
        let t = if self.multiline { text.replace("\r\n", "\n").replace('\r', "\n") } else { text.replace("\r\n", " ").replace(['\n', '\r'], " ") };
        self.replace_text_in_range(None, &t, window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn line_index_at(&self, position: Point<Pixels>, window: &Window) -> usize {
        let Some(bounds) = self.last_bounds.as_ref() else { return 0 };
        if self.last_lines.is_empty() {
            return 0;
        }
        let lh = window.line_height();
        let row = ((position.y - bounds.top()) / lh).floor().max(0.) as usize;
        let (start, line) = &self.last_lines[row.min(self.last_lines.len() - 1)];
        start + line.closest_index_for_x(position.x - bounds.left())
    }

    /// (baris, kolom-byte) untuk offset.
    fn row_col(&self, offset: usize) -> (usize, usize) {
        let before = &self.content[..offset.min(self.content.len())];
        let row = before.matches('\n').count();
        let col = before.rfind('\n').map(|i| offset - i - 1).unwrap_or(offset);
        (row, col)
    }

    fn offset_of(&self, row: usize, col: usize) -> usize {
        let mut start = 0;
        for (n, l) in self.content.split('\n').enumerate() {
            if n == row {
                let mut c = col.min(l.len());
                while !l.is_char_boundary(c) {
                    c -= 1;
                }
                return start + c;
            }
            start += l.len() + 1;
        }
        self.content.len()
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        if self.intercept {
            return cx.emit(InputNav::Up);
        }
        if !self.multiline {
            return cx.propagate();
        }
        let (r, c) = self.row_col(self.cursor_offset());
        let o = if r == 0 { 0 } else { self.offset_of(r - 1, c) };
        self.move_to(o, window, cx);
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        if self.intercept {
            return cx.emit(InputNav::Down);
        }
        if !self.multiline {
            return cx.propagate();
        }
        let (r, c) = self.row_col(self.cursor_offset());
        let o = self.offset_of(r + 1, c);
        self.move_to(o, window, cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    pub fn reset(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.clamp_range(range);

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        window.notify_text_input_changed(&self.focus_handle);
        cx.emit(InputChanged(self.content.clone()));
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.clamp_range(range);

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        let insertion_start_utf16 = self.offset_to_utf16(range.start);
        let inserted_length_utf16 = new_text.encode_utf16().count();
        self.selected_range = new_selected_range_utf16
            .map(|selected| {
                self.range_from_utf16(
                    &(insertion_start_utf16 + selected.start.min(inserted_length_utf16)
                        ..insertion_start_utf16 + selected.end.min(inserted_length_utf16)),
                )
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;

        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.text != self.content {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range_utf16, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        if last_layout.text != self.content {
            return None;
        }
        let utf8_index = last_layout.closest_index_for_x(line_point.x);
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Vec<(usize, ShapedLine)>,
    quads: Vec<PaintQuad>,
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl TextElement {
    /// Lukis kursor; bila `smooth_caret`, meluncur dari posisi sebelumnya (ease-out) dan berkedip saat diam.
    fn paint_caret(&self, mut q: PaintQuad, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let (smooth, content) = {
            let i = self.input.read(cx);
            (i.smooth_caret, i.content.clone())
        };
        let reduce = crate::theme::reduced_motion();
        if !smooth || reduce {
            window.paint_quad(q);
            return;
        }
        let now = std::time::Instant::now();
        let target = q.bounds.origin - bounds.origin;
        let (pos, opacity, animating) = self.input.update(cx, |input, _| {
            let a = &mut input.caret;
            if a.last_content != content {
                a.last_content = content.clone();
                a.typed = Some(now);
            }
            let cur = match (a.from, a.to, a.start) {
                (Some(f), Some(t), Some(s)) => {
                    let k = (s.elapsed().as_secs_f32() * 1000. / CARET_SLIDE_MS).min(1.);
                    let e = 1. - (1. - k).powi(3);
                    gpui::point(f.x + (t.x - f.x) * e, f.y + (t.y - f.y) * e)
                }
                _ => target,
            };
            if a.to != Some(target) {
                a.from = Some(if a.to.is_some() { cur } else { target });
                a.to = Some(target);
                a.start = Some(now);
            }
            let sliding = a.start.is_some_and(|s| s.elapsed().as_secs_f32() * 1000. < CARET_SLIDE_MS);
            let typing = a.typed.is_some_and(|t| t.elapsed().as_millis() < CARET_TYPING_MS);
            let base = a.typed.unwrap_or(now);
            let since = base.elapsed().as_millis();
            let ph = (since % 1000) as f32 / 1000.;
            let (op, wake_ms) = if sliding {
                (1., None)
            } else if typing {
                (1., Some(CARET_TYPING_MS - since))
            } else if ph < 0.4 {
                // composer-caret-blink: 0–40% nyala, 40–60% memudar, 60–100% padam (siklus 1 s)
                (1., Some(((0.4 - ph) * 1000.) as u128))
            } else if ph < 0.6 {
                (1. - (ph - 0.4) / 0.2, None)
            } else {
                (0., Some(((1. - ph) * 1000.) as u128 + 1))
            };
            let frame = sliding || (!typing && (0.4..0.6).contains(&ph));
            let wake = wake_ms.map(|ms| now + std::time::Duration::from_millis(ms as u64 + 1));
            let schedule = match (wake, a.wake_at) {
                (Some(w), Some(old)) if old > now && old <= w => None,
                (Some(w), _) => {
                    a.wake_at = Some(w);
                    Some(w)
                }
                _ => None,
            };
            (if sliding { cur } else { target }, op, (frame, schedule))
        });
        q.bounds.origin = bounds.origin + pos;
        q.background = q.background.opacity(opacity);
        window.paint_quad(q);
        let (frame, schedule) = animating;
        if frame {
            window.request_animation_frame();
        } else if let Some(w) = schedule {
            let input = self.input.clone();
            let dur = w.saturating_duration_since(now);
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(dur).await;
                    cx.update(|_, cx| input.update(cx, |_, cx| cx.notify())).ok();
                })
                .detach();
        }
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.read(cx);
        let rows = if input.multiline { input.content.split('\n').count().max(1) } else { 1 };
        style.size.height = (window.line_height() * rows as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        // Isi kosong: kursor selalu di awal (offset lama bisa basi dan terukur terhadap teks placeholder).
        let cursor = if content.is_empty() { 0 } else { input.cursor_offset() };
        let selected_range = if content.is_empty() { 0..0 } else { selected_range };
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), cx.global::<Theme>().colors.fg_subtle)
        } else if input.masked {
            // Topeng sepanjang byte agar offset kursor/seleksi tetap cocok.
            (SharedString::from("*".repeat(content.len())), style.color)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        if input.multiline && !input.content.is_empty() {
            let lh = window.line_height();
            let fg = cx.global::<Theme>().colors.fg;
            let mut sel_c = fg;
            sel_c.a = 0.25;
            let (mut lines, mut quads, mut start) = (vec![], vec![], 0usize);
            let mut mcursor: Option<PaintQuad> = None;
            for (row, l) in display_text.split('\n').enumerate() {
                let end = start + l.len();
                let shaped = window.text_system().shape_line(
                    SharedString::from(l.to_string()),
                    font_size,
                    &[TextRun { len: l.len(), color: text_color, font: style.font(), background_color: None, underline: None, strikethrough: None }],
                    None,
                );
                let top = bounds.top() + lh * row as f32;
                if selected_range.is_empty() {
                    if cursor >= start && cursor <= end && mcursor.is_none() {
                        mcursor = Some(fill(Bounds::new(point(bounds.left() + shaped.x_for_index(cursor - start), top), size(px(2.), lh)), fg));
                    }
                } else if selected_range.start <= end && selected_range.end >= start {
                    let a = selected_range.start.max(start);
                    let b = selected_range.end.min(end);
                    let extra = if selected_range.end > end { px(4.) } else { px(0.) };
                    quads.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + shaped.x_for_index(a - start), top),
                            point(bounds.left() + shaped.x_for_index(b - start) + extra, top + lh),
                        ),
                        sel_c,
                    ));
                }
                lines.push((start, shaped));
                start = end + 1;
            }
            return PrepaintState { lines, quads, line: None, cursor: mcursor, selection: None };
        }
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    cx.global::<Theme>().colors.fg,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    { let mut c = cx.global::<Theme>().colors.fg; c.a = 0.25; c },
                )),
                None,
            )
        };
        PrepaintState {
            lines: vec![],
            quads: vec![],
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if prepaint.line.is_none() {
            let lh = window.line_height();
            let focused = focus_handle.is_focused(window);
            for q in prepaint.quads.drain(..) {
                window.paint_quad(q);
            }
            if focused {
                if let Some(c) = prepaint.cursor.take() {
                    self.paint_caret(c, bounds, window, cx);
                }
            }
            for (row, (_, l)) in prepaint.lines.iter().enumerate() {
                let _ = l.paint(point(bounds.left(), bounds.top() + lh * row as f32), lh, gpui::TextAlign::Left, None, window, cx);
            }
            let lines = std::mem::take(&mut prepaint.lines);
            self.input.update(cx, |input, _cx| {
                input.last_lines = lines;
                input.last_layout = None;
                input.last_bounds = Some(bounds);
            });
            return;
        }
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(
            bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .unwrap();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            self.paint_caret(cursor, bounds, window, cx);
        }

        self.input.update(cx, |input, _cx| {
            input.last_lines = vec![(0, line.clone())];
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.blur_sub.is_none() {
            let fh = self.focus_handle.clone();
            self.blur_sub = Some(cx.on_focus_out(&fh, window, |this, _, _, cx| {
                let v: SharedString = this.text().to_string().into();
                cx.emit(InputBlur(v));
            }));
        }
        let t = cx.global::<Theme>().clone();
        let focused = self.focus_handle.is_focused(window);
        let border = if self.invalid { t.colors.danger } else { t.colors.border_soft };
        let mut el = div()
            .flex()
            .items_center()
            .w_full()
            .when(!self.multiline, |d| d.h(t.rem(self.size.height_rem())))
            .when(self.multiline, |d| d.min_h(t.rem(self.size.height_rem())).py(t.space(2)).items_start())
            .when(!self.bare, |d| d.px(t.space(3)).border_1().border_color(border).rounded(t.rem(crate::theme::rem::RADIUS_MD)).bg(t.colors.surface))
            .when(self.bare && self.multiline, |d| d.py_0().min_h(t.rem(crate::theme::rem::CONTROL_LG * 1.2)))
            .text_color(t.colors.fg)
            .text_size(t.rem(t.ty(crate::theme::P).size))
            .line_height(t.rem(t.ty(crate::theme::P).size * t.ty(crate::theme::P).lh))
            .overflow_hidden();
        if focused && !self.bare {
            el = el.border_color(t.colors.focus);
        }
        if self.disabled {
            return el.opacity(0.5).cursor_not_allowed().child(TextElement { input: cx.entity() });
        }
        el.key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(|this, _: &Submit, w, cx| {
                if this.intercept {
                    cx.emit(InputNav::Enter);
                } else if this.multiline && (this.enter_newline || this.ctrl_enter_sends) {
                    this.replace_text_in_range(None, "\n", w, cx);
                } else {
                    cx.emit(InputSubmit(this.content.clone()))
                }
            }))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::paste_plain))
            .on_action(cx.listener(|this, _: &CtrlSubmit, w, cx| {
                if this.intercept {
                    cx.emit(InputNav::Enter);
                } else if this.multiline && this.enter_newline && !this.ctrl_enter_sends {
                    this.replace_text_in_range(None, "\n", w, cx);
                } else {
                    cx.emit(InputSubmit(this.content.clone()))
                }
            }))
            .on_action(cx.listener(|this, _: &Tab, _, cx| if this.intercept { cx.emit(InputNav::Tab) } else { cx.propagate() }))
            .on_action(cx.listener(|this, _: &Escape, _, cx| if this.intercept { cx.emit(InputNav::Escape) } else { cx.propagate() }))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

/// Dipancarkan setiap isi berubah.
pub struct InputChanged(pub SharedString);
impl gpui::EventEmitter<InputChanged> for TextInput {}

/// Dipancarkan saat Enter ditekan.
pub struct InputSubmit(pub SharedString);
/// Teks panjang ditempel saat `long_paste_attach` aktif.
pub struct InputLongPaste(pub SharedString);
impl gpui::EventEmitter<InputLongPaste> for TextInput {}

/// Ambang teks panjang (port pasted-text.ts): > 2000 karakter atau > 30 baris.
pub fn is_long_paste(text: &str) -> bool {
    let lines = if text.is_empty() { 0 } else { text.split("\r\n").flat_map(|l| l.split(['\n', '\r'])).count() };
    text.chars().count() > 2000 || lines > 30
}
impl gpui::EventEmitter<InputSubmit> for TextInput {}
/// Fokus keluar dari kolom (nilai saat ini).
pub struct InputBlur(pub SharedString);
impl gpui::EventEmitter<InputBlur> for TextInput {}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            blur_sub: None,
            content: "".into(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            size: super::ControlSize::Md,
            invalid: false,
            disabled: false,
            masked: false,
            multiline: false,
            enter_newline: false,
            ctrl_enter_sends: false,
            long_paste_attach: false,
            last_lines: vec![],
            intercept: false,
            bare: false,
            smooth_caret: false,
            caret: CaretAnim::default(),
        }
    }
    /// Rentang byte yang aman: dibatasi panjang isi dan batas karakter (rentang IME/seleksi bisa basi setelah set_text).
    fn clamp_range(&self, r: Range<usize>) -> Range<usize> {
        let fix = |mut i: usize| {
            i = i.min(self.content.len());
            while i > 0 && !self.content.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        let (a, b) = (fix(r.start), fix(r.end));
        a.min(b)..a.max(b)
    }
    pub fn set_placeholder(&mut self, p: impl Into<SharedString>) {
        self.placeholder = p.into();
    }
    /// Posisi kursor (offset byte).
    pub fn caret(&self) -> usize {
        self.cursor_offset()
    }
    /// Ganti isi dan taruh kursor di `caret`.
    pub fn set_text_at(&mut self, text: impl Into<SharedString>, caret: usize, cx: &mut Context<Self>) {
        self.content = text.into();
        let c = caret.min(self.content.len());
        self.selected_range = c..c;
        cx.emit(InputChanged(self.content.clone()));
        cx.notify();
    }
    pub fn text(&self) -> SharedString {
        self.content.clone()
    }
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        cx.emit(InputChanged(self.content.clone()));
        cx.notify();
    }
}

/// Pintasan papan ketik input (Ctrl di Windows/Linux, Cmd di macOS).
pub fn bind_keys(cx: &mut App) {
    let mac = cfg!(target_os = "macos");
    let k = |a: &'static str, b: &'static str| if mac { a } else { b };
    let ctx = Some("TextInput");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new(k("cmd-a", "ctrl-a"), SelectAll, ctx),
        KeyBinding::new(k("cmd-v", "ctrl-v"), Paste, ctx),
        KeyBinding::new(k("cmd-c", "ctrl-c"), Copy, ctx),
        KeyBinding::new(k("cmd-x", "ctrl-x"), Cut, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Submit, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new(k("cmd-enter", "ctrl-enter"), CtrlSubmit, ctx),
        KeyBinding::new(k("cmd-shift-v", "ctrl-shift-v"), PastePlain, ctx),
        KeyBinding::new("tab", Tab, ctx),
        KeyBinding::new("escape", Escape, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
    ]);
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod paste_tests {
    #[test]
    fn ambang_teks_panjang() {
        assert!(!super::is_long_paste("pendek"));
        assert!(super::is_long_paste(&"a".repeat(2001)));
        assert!(!super::is_long_paste(&"a\n".repeat(29)));
        assert!(super::is_long_paste(&"a\r\n".repeat(31)));
    }
}
