// Halaman Tampilan (Fase 10): preset + tema kustom (impor/ekspor), tipografi per variabel, font, panel chat, gaya section
// project. Semua nilai divalidasi di AppearanceStore (core); halaman ini hanya menampilkan galatnya apa adanya.
import {
  type Appearance,
  BUILTIN_EDITOR_THEMES,
  COLOR_KEYS,
  type ColorKey,
  type ColorMap,
  comboFromEvent,
  contrastRatio,
  convertVsCodeTheme,
  FONT_SLOTS,
  type FontSlot,
  fetchOpenVsxThemes,
  formatCombo,
  isColor,
  type OpenVsxItem,
  PRESETS,
  type Project,
  TYPE_PROPS,
  TYPE_VARS,
  type TypeProp,
  type TypeVar,
  themesFromVsix,
  type VsixThemeResult,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Badge,
  Button,
  Field,
  Input,
  Select,
  Tabs,
  useConfirm,
  useShortcutList,
  useShortcutRegistry,
  useToast,
} from "@zandor/ui";
import { useEffect, useRef, useState } from "react";
import { ChatStyleControls, FONTS } from "./ChatStyleControls";
import { FontPicker } from "./FontPicker";
import { PageFrame } from "./PageFrame";
import { ProjectStyleDialog } from "./ProjectStyleDialog";
import { useAppearance, useProjects, useServices, useSettings } from "./services";

/** Input teks yang menerapkan nilai saat selesai (Enter/blur). Kosong = kembali ke default. Menampilkan galat validasi. */
function ValueField({
  label,
  value,
  placeholder,
  onCommit,
  description,
}: {
  label: string;
  value: string;
  placeholder?: string;
  description?: string;
  onCommit: (v: string | null) => string | null;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const shown = draft ?? value;
  const commit = () => {
    if (draft === null) return;
    const err = onCommit(draft.trim() === "" ? null : draft.trim());
    setError(err);
    if (!err) setDraft(null);
  };
  return (
    <Field label={label} description={description} error={error ?? undefined}>
      {(c) => (
        <Input
          {...c}
          value={shown}
          placeholder={placeholder}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
          }}
        />
      )}
    </Field>
  );
}

export function AppearancePage({ embedded = false }: { embedded?: boolean }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const a = useAppearance();
  const confirm = useConfirm();
  const reset = async () => {
    const ok = await confirm({
      title: t("ap.resetTitle"),
      description: t("ap.resetBody"),
      confirmLabel: t("ap.reset"),
      tone: "danger",
    });
    if (ok) store.resetAll();
  };
  return (
    <PageFrame
      title={t("ap.title")}
      embedded={embedded}
      actions={
        <Button variant="danger" onClick={() => void reset()}>
          {t("ap.reset")}
        </Button>
      }
    >
      <Tabs
        aria-label={t("ap.title")}
        items={[
          { value: "themes", label: t("ap.tab.themes"), content: <ThemesTab a={a} /> },
          { value: "type", label: t("ap.tab.type"), content: <TypeTab a={a} /> },
          { value: "chat", label: t("ap.tab.chat"), content: <ChatTab a={a} /> },
          { value: "projects", label: t("ap.tab.projects"), content: <ProjectsTab a={a} /> },
          { value: "editor", label: t("ap.tab.editor"), content: <EditorTab a={a} /> },
          { value: "keys", label: t("ap.tab.keys"), content: <KeysTab a={a} /> },
        ]}
      />
    </PageFrame>
  );
}

// ---------- Tema ----------

const SWATCH_KEYS: ColorKey[] = ["bg", "raised", "fg", "accent", "border-strong"];

function useEffectiveMode(): "dark" | "light" {
  const settings = useSettings();
  if (settings.theme !== "system") return settings.theme;
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function ThemesTab({ a }: { a: Appearance }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const toast = useToast();
  const mode = useEffectiveMode();
  const fileRef = useRef<HTMLInputElement>(null);
  const [editing, setEditing] = useState<string | null | "new">(null);
  const [importErrors, setImportErrors] = useState<string[]>([]);
  const themes = store.themes();

  const colorsOf = (id: string): ColorMap => {
    const src = PRESETS.find((p) => p.id === id) ?? a.customThemes.find((c) => c.id === id);
    return src?.[mode] ?? {};
  };

  const exportTheme = (id: string) => {
    const blob = new Blob([store.exportTheme(id)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `zandor-tema-${id}.json`;
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  const importFile = async (file: File) => {
    setImportErrors([]);
    if (file.size > 100_000) return setImportErrors([t("ap.import.tooBig")]);
    const r = store.importTheme(await file.text());
    if (r.ok) toast.show({ title: t("ap.import.ok", { id: r.id }), tone: "success" });
    else setImportErrors(r.errors);
  };

  return (
    <section className="mempage__section" aria-label={t("ap.tab.themes")}>
      <p className="z-small mempage__muted">{t("ap.themes.intro")}</p>
      <div className="mempage__actions">
        <Button variant="outline" onClick={() => fileRef.current?.click()}>
          {t("ap.import")}
        </Button>
        <Button variant="outline" onClick={() => setEditing("new")}>
          {t("ap.new")}
        </Button>
        <input
          ref={fileRef}
          type="file"
          accept="application/json,.json"
          hidden
          aria-label={t("ap.import")}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void importFile(f);
            e.target.value = "";
          }}
        />
      </div>
      {importErrors.length > 0 && (
        <ul className="chat__note chat__note--error z-small" role="alert">
          {importErrors.map((e) => (
            <li key={e}>{e}</li>
          ))}
        </ul>
      )}
      <ul className="mempage__list ap__grid" aria-label={t("ap.tab.themes")}>
        {themes.map((th) => {
          const c = colorsOf(th.id);
          return (
            <li
              key={th.id}
              className="mempage__card"
              aria-label={th.name}
              data-active={a.preset === th.id ? "" : undefined}
            >
              <div className="ap__swatches" aria-hidden="true">
                {SWATCH_KEYS.map((k) => (
                  <span key={k} className="ap__swatch" style={{ background: c[k] ?? `var(--z-color-${k})` }} />
                ))}
              </div>
              <div className="mempage__cardhead">
                <span className="z-label mempage__grow">{th.name}</span>
                {a.preset === th.id && <Badge tone="success">{t("ap.active")}</Badge>}
                {!th.builtin && <Badge tone="outline">{t("ap.custom")}</Badge>}
              </div>
              <div className="mempage__actions">
                <Button size="sm" variant="solid" disabled={a.preset === th.id} onClick={() => store.setPreset(th.id)}>
                  {t("ap.use")}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => exportTheme(th.id)}
                  aria-label={`${t("ap.export")} ${th.name}`}
                >
                  {t("ap.export")}
                </Button>
                {!th.builtin && (
                  <>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setEditing(th.id)}
                      aria-label={`${t("ap.edit")} ${th.name}`}
                    >
                      {t("ap.edit")}
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => store.removeCustomTheme(th.id)}
                      aria-label={`${t("ap.delete")} ${th.name}`}
                    >
                      {t("ap.delete")}
                    </Button>
                  </>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {editing !== null && (
        <ThemeEditor a={a} editingId={editing === "new" ? null : editing} onClose={() => setEditing(null)} />
      )}
    </section>
  );
}

const slug = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);

function ThemeEditor({ a, editingId, onClose }: { a: Appearance; editingId: string | null; onClose: () => void }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const existing = editingId ? a.customThemes.find((c) => c.id === editingId) : undefined;
  const [name, setName] = useState(existing?.name ?? "");
  const [dark, setDark] = useState<ColorMap>(existing?.dark ?? {});
  const [light, setLight] = useState<ColorMap>(existing?.light ?? {});
  const [errors, setErrors] = useState<string[]>([]);
  const [from, setFrom] = useState("");

  const copyFrom = (id: string) => {
    setFrom(id);
    const src = PRESETS.find((p) => p.id === id) ?? a.customThemes.find((c) => c.id === id);
    if (src) {
      setDark({ ...(src.dark ?? {}) });
      setLight({ ...(src.light ?? {}) });
    }
  };

  const save = () => {
    const id = existing?.id ?? slug(name);
    const clean = (m: ColorMap) => Object.fromEntries(Object.entries(m).filter(([, v]) => v !== undefined && v !== ""));
    const r = store.saveCustomTheme({ id, name, dark: clean(dark), light: clean(light) });
    if (r) return setErrors(r);
    store.setPreset(id);
    onClose();
  };

  const modeColumn = (title: string, value: ColorMap, set: (m: ColorMap) => void) => {
    const ratio = value.fg && value.bg ? contrastRatio(value.fg, value.bg) : null;
    return (
      <fieldset className="ap__col">
        <legend className="z-label">{title}</legend>
        {COLOR_KEYS.map((k) => (
          <div key={k} className="ap__color">
            <span className="z-small ap__colorname">{k}</span>
            <span
              className="ap__swatch"
              style={{ background: isColor(value[k]) ? value[k] : "transparent" }}
              aria-hidden="true"
            />
            <Input
              aria-label={`${title} ${k}`}
              value={value[k] ?? ""}
              invalid={value[k] !== undefined && value[k] !== "" && !isColor(value[k])}
              placeholder={t("ap.editor.default")}
              onChange={(e) => set({ ...value, [k]: e.target.value })}
            />
          </div>
        ))}
        {ratio !== null && (
          <p className="z-small" data-testid={`contrast-${title}`} data-low={ratio < 4.5 ? "" : undefined}>
            {t("ap.editor.contrast", { ratio: ratio.toFixed(2) })} {ratio < 4.5 ? t("ap.editor.contrastLow") : ""}
          </p>
        )}
      </fieldset>
    );
  };

  return (
    <section className="mempage__card" aria-label={t("ap.editor.title")}>
      <h2 className="z-h4">{t("ap.editor.title")}</h2>
      <Field label={t("ap.editor.name")}>
        {(c) => <Input {...c} value={name} onChange={(e) => setName(e.target.value)} />}
      </Field>
      {!existing && (
        <Field label={t("ap.editor.from")}>
          {() => (
            <Select
              aria-label={t("ap.editor.from")}
              value={from}
              onValueChange={copyFrom}
              placeholder={t("ap.editor.fromNone")}
              items={store.themes().map((th) => ({ value: th.id, label: th.name }))}
            />
          )}
        </Field>
      )}
      <div className="ap__cols">
        {modeColumn(t("ap.editor.dark"), dark, setDark)}
        {modeColumn(t("ap.editor.light"), light, setLight)}
      </div>
      {errors.length > 0 && (
        <ul className="chat__note chat__note--error z-small" role="alert">
          {errors.map((e) => (
            <li key={e}>{e}</li>
          ))}
        </ul>
      )}
      <div className="mempage__actions">
        <Button variant="solid" onClick={save}>
          {t("ap.editor.save")}
        </Button>
        <Button variant="ghost" onClick={onClose}>
          {t("ap.editor.cancel")}
        </Button>
      </div>
    </section>
  );
}

// ---------- Tipografi ----------

function TypeTab({ a }: { a: Appearance }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  return (
    <section className="mempage__section" aria-label={t("ap.tab.type")}>
      <p className="z-small mempage__muted">{t("ap.type.intro")}</p>
      <h2 className="z-h4">{t("ap.type.fonts")}</h2>
      {FONT_SLOTS.map((slot: FontSlot) => (
        <div key={slot} className="cstyle__row">
          <span className="z-label">{t(`ap.font.${slot}` as never)}</span>
          <FontPicker
            label={t(`ap.font.${slot}` as never)}
            value={a.fonts[slot] ?? null}
            presets={FONTS}
            onChange={(v) => store.setFont(slot, v)}
          />
        </div>
      ))}
      <h2 className="z-h4">{t("ap.type.vars")}</h2>
      <p className="z-small mempage__muted">{t("ap.type.varsHint")}</p>
      {TYPE_VARS.map((v: TypeVar) => (
        <fieldset key={v} className="ap__typerow">
          <legend className="z-label">{v}</legend>
          {TYPE_PROPS.map((p: TypeProp) => (
            <ValueField
              key={p}
              label={`${v} · ${t(`ap.prop.${p}` as never)}`}
              value={a.typography[v]?.[p] ?? ""}
              placeholder={t("ap.editor.default")}
              onCommit={(val) => store.setTypography(v, p, val)}
            />
          ))}
        </fieldset>
      ))}
    </section>
  );
}

// ---------- Chat ----------

function ChatTab(_: { a: Appearance }) {
  const { t } = useI18n();
  return (
    <section className="mempage__section" aria-label={t("ap.tab.chat")}>
      <p className="z-small mempage__muted">{t("ap.chat.intro")}</p>
      <ChatStyleControls />
      <div className="ap__chatpreview" data-testid="chat-preview">
        <p className="msg__text">{t("ap.chat.preview")}</p>
      </div>
    </section>
  );
}

// ---------- Project ----------

function ProjectsTab({ a }: { a: Appearance }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const list = useProjects().projects;
  const [styling, setStyling] = useState<Project | null>(null);
  if (list.length === 0) return <p className="z-small mempage__muted">{t("mem.settings.projectsEmpty")}</p>;
  return (
    <section className="mempage__section" aria-label={t("ap.tab.projects")}>
      <p className="z-small mempage__muted">{t("ap.projects.intro")}</p>
      <ul className="mempage__list">
        {list.map((p) => (
          <li key={p.id} className="mempage__card" aria-label={p.name}>
            <h2 className="z-h4">{p.name}</h2>
            <Button size="sm" onClick={() => setStyling(p)}>
              {t("sb.customize")}
            </Button>
            <ValueField
              label={`${p.name} · ${t("ap.projects.logo")}`}
              value={a.projects[p.id]?.logo ?? ""}
              placeholder="🚀"
              onCommit={(v) => store.setProjectStyle(p.id, "logo", v)}
            />
            <ValueField
              label={`${p.name} · ${t("ap.projects.border")}`}
              value={a.projects[p.id]?.border ?? ""}
              placeholder={t("ap.projects.colorHint")}
              onCommit={(v) => store.setProjectStyle(p.id, "border", v)}
            />
            <ValueField
              label={`${p.name} · ${t("ap.projects.background")}`}
              value={a.projects[p.id]?.background ?? ""}
              placeholder={t("ap.projects.colorHint")}
              onCommit={(v) => store.setProjectStyle(p.id, "background", v)}
            />
          </li>
        ))}
      </ul>
      {styling && <ProjectStyleDialog project={styling} onClose={() => setStyling(null)} />}
    </section>
  );
}

// ---------- Editor ----------

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

function EditorTab({ a }: { a: Appearance }) {
  const { t } = useI18n();
  const { appearance: store, openVsx } = useServices();
  const toast = useToast();
  const fileRef = useRef<HTMLInputElement>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<OpenVsxItem[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const items = [
    { value: "auto", label: t("ap.editor2.auto") },
    ...BUILTIN_EDITOR_THEMES.map((id) => ({ value: id, label: t("ap.editor2.builtin", { name: id }) })),
    ...a.editorThemes.map((th) => ({ value: th.id, label: th.name })),
  ];

  const addThemes = (r: VsixThemeResult, label: string) => {
    const problems = [...r.errors];
    let first: string | null = null;
    for (const th of r.themes) {
      const err = store.addEditorTheme(th);
      if (err) problems.push(`${th.name}: ${err}`);
      else first ??= th.id;
    }
    if (r.skipped.length) problems.push(t("ap.editor2.skipped", { list: r.skipped.join("; ") }));
    if (first) {
      store.setEditorTheme(first);
      toast.show({ title: t("ap.editor2.imported", { name: label }), tone: "success" });
    }
    setErrors(problems);
  };

  const importFile = async (file: File) => {
    setErrors([]);
    if (/\.vsix$/i.test(file.name)) {
      const bytes = new Uint8Array(await file.arrayBuffer());
      return addThemes(themesFromVsix(bytes, `berkas:${file.name}`), file.name.replace(/\.vsix$/i, ""));
    }
    const r = convertVsCodeTheme(await file.text(), { name: file.name.replace(/\.jsonc?$/i, ""), source: "berkas" });
    if (!r.theme) return setErrors(r.errors);
    const err = store.addEditorTheme(r.theme);
    if (err) return setErrors([err]);
    store.setEditorTheme(r.theme.id);
    toast.show({ title: t("ap.editor2.imported", { name: r.theme.name }), tone: "success" });
  };

  const search = async () => {
    setErrors([]);
    setBusy("search");
    try {
      setResults(await openVsx.search(query));
    } catch (e) {
      setErrors([errMsg(e)]);
    } finally {
      setBusy(null);
    }
  };

  const install = async (item: OpenVsxItem) => {
    setErrors([]);
    setBusy(item.download);
    try {
      addThemes(await fetchOpenVsxThemes(openVsx, item), item.display_name);
    } catch (e) {
      setErrors([errMsg(e)]);
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="mempage__section" aria-label={t("ap.tab.editor")}>
      <p className="z-small mempage__muted">{t("ap.editor2.intro")}</p>
      <Field label={t("ap.editor2.select")}>
        {() => (
          <Select
            aria-label={t("ap.editor2.select")}
            value={a.editorTheme}
            onValueChange={(v) => store.setEditorTheme(v)}
            items={items}
          />
        )}
      </Field>
      <div className="mempage__actions">
        <Button variant="outline" onClick={() => fileRef.current?.click()}>
          {t("ap.editor2.importFile")}
        </Button>
        <input
          ref={fileRef}
          type="file"
          accept="application/json,.json,.jsonc,.vsix"
          hidden
          aria-label={t("ap.editor2.importFile")}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void importFile(f);
            e.target.value = "";
          }}
        />
      </div>
      <h2 className="z-h4">{t("ap.editor2.installed")}</h2>
      {a.editorThemes.length === 0 ? (
        <p className="z-small mempage__muted">{t("ap.editor2.none")}</p>
      ) : (
        <ul className="mempage__list">
          {a.editorThemes.map((th) => (
            <li key={th.id} className="mempage__row" aria-label={th.name}>
              <span className="mempage__grow">
                {th.name} <span className="z-small mempage__muted">{th.source}</span>
              </span>
              <Button
                size="sm"
                variant="danger"
                aria-label={`${t("ap.editor2.remove")} ${th.name}`}
                onClick={() => store.removeEditorTheme(th.id)}
              >
                {t("ap.editor2.remove")}
              </Button>
            </li>
          ))}
        </ul>
      )}
      <h2 className="z-h4">{t("ap.editor2.search")}</h2>
      {!openVsx.available ? (
        <p className="z-small mempage__muted">{t("ap.editor2.unavailable")}</p>
      ) : (
        <>
          <p className="z-small mempage__muted">{t("ap.editor2.license")}</p>
          <div className="mempage__row">
            <Input
              aria-label={t("ap.editor2.search")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && query.trim()) void search();
              }}
            />
            <Button variant="solid" loading={busy === "search"} disabled={!query.trim()} onClick={() => void search()}>
              {t("ap.editor2.searchBtn")}
            </Button>
          </div>
          {results && (
            <ul className="mempage__list" aria-label={t("ap.editor2.results")}>
              {results.length === 0 && <li className="z-small mempage__muted">{t("ap.editor2.noResults")}</li>}
              {results.map((r) => (
                <li key={r.download} className="mempage__card" aria-label={r.display_name}>
                  <div className="mempage__cardhead">
                    <span className="z-label mempage__grow">
                      {r.display_name}{" "}
                      <span className="z-small mempage__muted">
                        {r.namespace}.{r.name}@{r.version}
                      </span>
                    </span>
                    <span className="z-small mempage__muted">{t("ap.editor2.downloads", { n: r.downloads })}</span>
                    <Button
                      size="sm"
                      variant="solid"
                      loading={busy === r.download}
                      onClick={() => void install(r)}
                      aria-label={`${t("ap.editor2.install")} ${r.display_name}`}
                    >
                      {t("ap.editor2.install")}
                    </Button>
                  </div>
                  <p className="z-small mempage__muted">{r.description}</p>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
      {errors.length > 0 && (
        <ul className="chat__note chat__note--error z-small" role="alert">
          {errors.map((e) => (
            <li key={e}>{e}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

// ---------- Pintasan ----------

function KeysTab({ a }: { a: Appearance }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const registry = useShortcutRegistry();
  const bindings = useShortcutList();
  const [capturing, setCapturing] = useState<string | null>(null);
  const [error, setError] = useState<{ id: string; text: string } | null>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `a.shortcuts` hanya memicu render ulang daftar
  useEffect(() => {
    if (!capturing) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
      if (e.key === "Escape") return setCapturing(null);
      const combo = comboFromEvent(e);
      if (!(combo.ctrl || combo.alt || combo.meta || /^F\d+$/.test(combo.key))) {
        return setError({ id: capturing, text: t("ap.keys.needModifier") });
      }
      const text = formatCombo(combo);
      const err = registry.setOverride(capturing, text);
      if (err) return setError({ id: capturing, text: err });
      store.setShortcut(capturing, text);
      setError(null);
      setCapturing(null);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturing, registry, store, t, a.shortcuts]);

  const reset = (id: string) => {
    registry.setOverride(id, null);
    store.setShortcut(id, null);
    setError(null);
  };

  if (bindings.length === 0) return <p className="z-small mempage__muted">{t("ap.keys.empty")}</p>;
  return (
    <section className="mempage__section" aria-label={t("ap.tab.keys")}>
      <p className="z-small mempage__muted">{t("ap.keys.intro")}</p>
      <ul className="mempage__list">
        {bindings.map((b) => {
          const current = registry.comboOf(b.id) ?? b.combo;
          const def = registry.defaultComboOf(b.id) ?? b.combo;
          const changed = current !== def;
          return (
            <li key={b.id} className="mempage__card" aria-label={b.id}>
              <div className="mempage__cardhead">
                <span className="mempage__grow">{t(b.description as never)}</span>
                <kbd className="z-kbd" data-testid={`key-${b.id}`}>
                  {capturing === b.id ? t("ap.keys.press") : current}
                </kbd>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    setError(null);
                    setCapturing(b.id);
                  }}
                  aria-label={`${t("ap.keys.change")} ${b.id}`}
                >
                  {t("ap.keys.change")}
                </Button>
                {changed && (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => reset(b.id)}
                    aria-label={`${t("ap.keys.reset")} ${b.id}`}
                  >
                    {t("ap.keys.reset")}
                  </Button>
                )}
              </div>
              {changed && <p className="z-small mempage__muted">{t("ap.keys.default", { combo: def })}</p>}
              {error?.id === b.id && (
                <p className="chat__note chat__note--error z-small" role="alert">
                  {error.text}
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
