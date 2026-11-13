import { formatCombo } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  applyScale,
  applyTheme,
  Button,
  Kbd,
  Select,
  Slider,
  stepScale,
  storedScale,
  storedTheme,
  THEMES,
  type Theme,
  UI_SCALES,
  type UiScale,
  useShortcut,
  useShortcutList,
} from "@zandor/ui";
import { type ReactNode, useState } from "react";
import { ATOM_DEMOS, MOLECULE_DEMOS } from "./demos";
import { TokensSection } from "./TokensSection";

/** Menus dan Modals dipecah dari molecules agar urutan halaman sama dengan roadmap. */
export const MENU_COMPONENTS = ["Menu", "ContextMenu"] as const;
export const MODAL_COMPONENTS = ["Dialog", "ConfirmDialog"] as const;
const SPECIAL = new Set<string>([...MENU_COMPONENTS, ...MODAL_COMPONENTS]);

export const SECTIONS = [
  { id: "tokens", label: "Tokens" },
  { id: "atoms", label: "Atoms" },
  { id: "molecules", label: "Molecules" },
  { id: "organisms", label: "Organisms" },
  { id: "panels", label: "Panels" },
  { id: "menus", label: "Menus" },
  { id: "modals", label: "Modals" },
  { id: "pages", label: "Pages" },
  { id: "shortcuts", label: "Shortcut" },
] as const;

function Demo({ name, children }: { name: string; children: ReactNode }) {
  return (
    <section className="ds-demo" data-demo={name} aria-labelledby={`demo-${name}`}>
      <h3 id={`demo-${name}`} className="z-h4">
        {name}
      </h3>
      <div className="ds-demo__body">{children}</div>
    </section>
  );
}

function NotYet({ phase, what }: { phase: string; what: string }) {
  return (
    <div className="ds-empty" role="note">
      <p className="z-label">Belum ada</p>
      <p className="z-small">
        {what} Diisi mulai {phase}. Tidak ada contoh palsu di sini.
      </p>
    </div>
  );
}

function ShortcutsSection() {
  const bindings = useShortcutList();
  return (
    <div className="ds-stack">
      <p className="z-small">
        Pintasan terdaftar saat ini (global dan per scope). Scope ditentukan dari panel/popup tempat fokus berada.
      </p>
      <table className="ds-table">
        <thead>
          <tr>
            <th scope="col">Pintasan</th>
            <th scope="col">Scope</th>
            <th scope="col">ID</th>
          </tr>
        </thead>
        <tbody>
          {bindings.map((b) => (
            <tr key={b.id}>
              <td>
                <Kbd keys={formatCombo(b.combo)} />
              </td>
              <td>{b.scope}</td>
              <td>
                <code className="z-code">{b.id}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function DesignSystemPage() {
  const { t } = useI18n();
  const [theme, setTheme] = useState<Theme>(storedTheme());
  const [scale, setScale] = useState<UiScale>(storedScale());

  const changeTheme = (next: Theme) => {
    setTheme(next);
    applyTheme(next);
  };
  const changeScale = (next: UiScale) => {
    setScale(next);
    applyScale(next);
  };

  // Pintasan nyata (bukan dekorasi): ganti tema dan zoom UI.
  useShortcut({
    id: "view.toggleTheme",
    combo: "Ctrl+Alt+T",
    scope: "global",
    description: "theme.label",
    allowInInput: true,
    run: () => changeTheme(theme === "dark" ? "light" : "dark"),
  });
  useShortcut({
    id: "view.zoomIn",
    combo: "Ctrl+=",
    scope: "global",
    description: "scale.label",
    allowInInput: true,
    run: () => changeScale(stepScale(scale, 1)),
  });
  useShortcut({
    id: "view.zoomOut",
    combo: "Ctrl+-",
    scope: "global",
    description: "scale.label",
    allowInInput: true,
    run: () => changeScale(stepScale(scale, -1)),
  });
  useShortcut({
    id: "view.zoomReset",
    combo: "Ctrl+0",
    scope: "global",
    description: "scale.label",
    allowInInput: true,
    run: () => changeScale(1),
  });

  const molecules = Object.entries(MOLECULE_DEMOS).filter(([n]) => !SPECIAL.has(n));
  const pick = (names: readonly string[]) => Object.entries(MOLECULE_DEMOS).filter(([n]) => names.includes(n));

  return (
    <div className="ds">
      <header className="ds-header">
        <h1 className="z-h2">Design system Zandor</h1>
        <div className="ds-controls">
          <div className="ds-control">
            <span className="z-label">{t("theme.label")}</span>
            <Select
              aria-label={t("theme.label")}
              value={theme}
              onValueChange={(v) => changeTheme(v as Theme)}
              items={THEMES.map((th) => ({ value: th, label: t(`theme.${th}`) }))}
            />
          </div>
          <div className="ds-control">
            <span className="z-label">
              {t("scale.label")}: {Math.round(scale * 100)}%
            </span>
            <Slider
              label={t("scale.label")}
              value={scale * 100}
              min={UI_SCALES[0] * 100}
              max={(UI_SCALES.at(-1) ?? 1.5) * 100}
              step={12.5}
              onValueChange={(v) => changeScale((UI_SCALES.find((s) => s * 100 === v) ?? 1) as UiScale)}
            />
          </div>
          <Button size="sm" onClick={() => changeScale(1)}>
            100%
          </Button>
          <a className="z-link" href="#/">
            ← Aplikasi
          </a>
        </div>
      </header>

      <div className="ds-layout">
        <nav className="ds-nav" aria-label="Bagian design system">
          <ul>
            {SECTIONS.map((s) => (
              <li key={s.id}>
                <a href={`#/design-system/${s.id}`} className="ds-nav__link">
                  {s.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>

        <main className="ds-content">
          <section id="tokens" className="ds-section" aria-labelledby="h-tokens">
            <h2 id="h-tokens" className="z-h3">
              Tokens
            </h2>
            <TokensSection />
          </section>

          <section id="atoms" className="ds-section" aria-labelledby="h-atoms">
            <h2 id="h-atoms" className="z-h3">
              Atoms
            </h2>
            {Object.entries(ATOM_DEMOS).map(([name, render]) => (
              <Demo key={name} name={name}>
                {render()}
              </Demo>
            ))}
          </section>

          <section id="molecules" className="ds-section" aria-labelledby="h-molecules">
            <h2 id="h-molecules" className="z-h3">
              Molecules
            </h2>
            {molecules.map(([name, render]) => (
              <Demo key={name} name={name}>
                {render()}
              </Demo>
            ))}
          </section>

          <section id="organisms" className="ds-section" aria-labelledby="h-organisms">
            <h2 id="h-organisms" className="z-h3">
              Organisms
            </h2>
            <NotYet phase="Fase 3" what="Top bar, command palette, dan daftar project belum dibuat." />
          </section>

          <section id="panels" className="ds-section" aria-labelledby="h-panels">
            <h2 id="h-panels" className="z-h3">
              Panels
            </h2>
            <NotYet phase="Fase 3" what="Kelima panel belum dibuat." />
          </section>

          <section id="menus" className="ds-section" aria-labelledby="h-menus">
            <h2 id="h-menus" className="z-h3">
              Menus
            </h2>
            {pick(MENU_COMPONENTS).map(([name, render]) => (
              <Demo key={name} name={name}>
                {render()}
              </Demo>
            ))}
          </section>

          <section id="modals" className="ds-section" aria-labelledby="h-modals">
            <h2 id="h-modals" className="z-h3">
              Modals
            </h2>
            {pick(MODAL_COMPONENTS).map(([name, render]) => (
              <Demo key={name} name={name}>
                {render()}
              </Demo>
            ))}
          </section>

          <section id="pages" className="ds-section" aria-labelledby="h-pages">
            <h2 id="h-pages" className="z-h3">
              Pages
            </h2>
            <NotYet phase="Fase 3" what="Halaman Pengaturan belum dibuat." />
          </section>

          <section id="shortcuts" className="ds-section" aria-labelledby="h-shortcuts">
            <h2 id="h-shortcuts" className="z-h3">
              Shortcut
            </h2>
            <ShortcutsSection />
          </section>
        </main>
      </div>
    </div>
  );
}
