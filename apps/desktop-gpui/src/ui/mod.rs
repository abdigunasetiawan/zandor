//! Port packages/ui (atom & molekul) ke GPUI.
#![allow(dead_code, unused_imports)]
pub mod button;
pub mod choice;
pub mod color_picker;
pub mod feedback;
pub mod highlight;
pub mod icon;
pub mod markdown;
pub mod math;
pub mod menu;
pub mod mermaid;
pub mod rich;
pub mod molecules;
pub mod select;
pub mod spinner;
pub mod text_input;
pub mod toast;
pub mod tooltip;
pub mod typo;

pub use button::{button, icon_button, Button, ButtonVariant, ControlSize};
pub use icon::{icon, Icon, IconSize};
pub use spinner::spinner;
pub use feedback::{avatar, badge, kbd, skeleton, working, BadgeTone, WorkingVariant};
pub use text_input::{is_long_paste, Escape as InputEscape, InputBlur, InputChanged, InputLongPaste, InputPasteFiles, InputNav, InputSubmit, TextInput};
pub use choice::{checkbox, switch, Slider, SliderChanged};
pub use molecules::{dialog, field, menu_item, popup, separator, tabs, TabItem};
pub use toast::{ToastAction, ToastInput, ToastTone, Toasts};
pub use typo::TypeExt;
pub use menu::{entry, menu_popup, MenuEntry};
pub use markdown::markdown;
pub use select::{select, Select};

thread_local! {
    static OVERLAY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `deferred` untuk lapisan UI (menu, dialog, palet) yang juga menandai bahwa ada overlay di frame ini, sehingga
/// webview native (Browser) disembunyikan agar tidak menutupinya (airspace, sama dengan versi Tauri).
pub fn overlay_deferred(child: impl gpui::IntoElement) -> gpui::Deferred {
    OVERLAY.with(|o| o.set(true));
    gpui::deferred(child)
}

static MODALS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Penanda dialog modal yang sedang terbuka. Berbeda dari `overlay_deferred` (penanda per frame yang bisa terlewat bila
/// view tidak digambar ulang), penjaga ini aktif selama dialog hidup; Browser menyembunyikan webview selama ada.
pub struct ModalGuard;

impl ModalGuard {
    pub fn new() -> Self {
        MODALS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self
    }
}

impl Drop for ModalGuard {
    fn drop(&mut self) {
        MODALS.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn modal_open() -> bool {
    MODALS.load(std::sync::atomic::Ordering::Relaxed) > 0
}

/// Baca lalu reset penanda overlay (dipanggil sekali per frame oleh host Browser).
pub fn take_overlay() -> bool {
    OVERLAY.with(|o| o.replace(false))
}
