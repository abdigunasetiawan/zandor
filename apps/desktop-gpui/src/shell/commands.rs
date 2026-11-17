//! Registry perintah — id, kunci judul, grup, pintasan sama persis dengan CommandCenter.tsx & ShellLayout.tsx.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    App,
    View,
    Help,
    Dev,
}

#[derive(Clone, Copy, Debug)]
pub struct CommandDef {
    pub id: &'static str,
    pub title_key: &'static str,
    pub group: Group,
    pub shortcut: Option<&'static str>,
    pub keywords: &'static [&'static str],
}

const fn c(id: &'static str, title_key: &'static str, group: Group, shortcut: Option<&'static str>) -> CommandDef {
    CommandDef { id, title_key, group, shortcut, keywords: &[] }
}

pub const COMMANDS: &[CommandDef] = &[
    c("app.openSettings", "cmd.openSettings", Group::App, Some("Ctrl+,")),
    c("chat.new", "cmd.newChat", Group::App, Some("Ctrl+N")),
    c("app.providers", "cmd.providers", Group::App, None),
    c("app.memory", "cmd.memory", Group::App, None),
    c("app.automation", "cmd.automation", Group::App, None),
    c("app.appearance", "cmd.appearance", Group::App, None),
    c("app.goHome", "cmd.goHome", Group::App, None),
    c("nav.back", "cmd.back", Group::App, Some("Alt+ArrowLeft")),
    c("nav.forward", "cmd.forward", Group::App, Some("Alt+ArrowRight")),
    c("app.palette", "cmd.palette", Group::App, Some("Ctrl+K")),
    c("app.searchSessions", "cmd.searchSessions", Group::App, Some("Ctrl+Shift+F")),
    c("view.zoomIn", "cmd.zoomIn", Group::View, Some("Ctrl+=")),
    c("view.zoomOut", "cmd.zoomOut", Group::View, Some("Ctrl+-")),
    c("view.zoomReset", "cmd.zoomReset", Group::View, Some("Ctrl+0")),
    c("view.toggleTheme", "cmd.toggleTheme", Group::View, Some("Ctrl+Alt+T")),
    c("view.langId", "cmd.langId", Group::View, None),
    c("view.langEn", "cmd.langEn", Group::View, None),
    c("view.toggleSidebar", "cmd.toggleSidebar", Group::View, Some("Ctrl+1")),
    c("view.toggleChat", "cmd.toggleChat", Group::View, Some("Ctrl+2")),
    c("view.toggleDocs", "cmd.toggleDocs", Group::View, Some("Ctrl+3")),
    c("view.toggleTools", "cmd.toggleTools", Group::View, Some("Ctrl+4")),
    c("view.toggleRail", "cmd.toggleRail", Group::View, Some("Ctrl+5")),
    c("view.smartResize", "cmd.smartResize", Group::View, Some("Ctrl+Alt+0")),
    c("view.resetLayout", "cmd.resetLayout", Group::View, None),
    c("dev.designSystem", "cmd.designSystem", Group::Dev, None),
    c("help.about", "cmd.about", Group::Help, None),
    c("help.tour", "cmd.tour", Group::Help, None),
    CommandDef {
        id: "image.generate",
        title_key: "cmd.generateImage",
        group: Group::App,
        shortcut: None,
        keywords: &["image", "gambar", "generate", "dall-e", "gemini"],
    },
    c("window.close", "cmd.closeWindow", Group::App, None),
];

pub fn find(id: &str) -> Option<&'static CommandDef> {
    COMMANDS.iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn semua_judul_ada_di_i18n() {
        let id = crate::i18n::I18n::new(crate::i18n::Locale::Id);
        for c in super::COMMANDS {
            assert_ne!(id.t(c.title_key), c.title_key, "kunci i18n hilang: {}", c.title_key);
        }
    }
}


/// Pintasan efektif satu perintah (override Tampilan → Shortcut, atau bawaan).
pub fn shortcut_of(id: &str, cx: &gpui::App) -> Option<String> {
    let ov = crate::core::appearance::AppearanceStore::global(cx).read(cx).shortcuts.get(id).cloned();
    ov.or_else(|| find(id).and_then(|c| c.shortcut).map(String::from))
}

/// Perintah yang memakai combo ini (memperhitungkan override).
pub fn command_for_combo(combo: &str, cx: &gpui::App) -> Option<&'static CommandDef> {
    COMMANDS.iter().find(|c| shortcut_of(c.id, cx).as_deref() == Some(combo))
}

/// Ubah pintasan; galat bila bentrok dengan perintah lain.
pub fn set_shortcut(id: &str, combo: Option<&str>, cx: &mut gpui::App) -> Result<(), String> {
    if let Some(c) = combo {
        if let Some(other) = COMMANDS.iter().find(|o| o.id != id && shortcut_of(o.id, cx).as_deref() == Some(c)) {
            return Err(format!("bentrok: {c} sudah dipakai {}", other.id));
        }
    }
    let def = find(id).and_then(|c| c.shortcut);
    let value = combo.filter(|c| Some(*c) != def);
    crate::core::appearance::AppearanceStore::global(cx).update(cx, |s, cx| s.set_shortcut(id, value, cx));
    Ok(())
}
