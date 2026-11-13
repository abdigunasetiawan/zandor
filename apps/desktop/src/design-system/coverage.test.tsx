// @vitest-environment jsdom
// Penjaga kriteria keluar Fase 2: setiap komponen terdaftar di katalog, punya demo di halaman design system,
// dan halaman memuat semua bagian sesuai urutan roadmap.
import { cleanup, render, screen, within } from "@testing-library/react";
import { createLogger } from "@zandor/core";
import * as ui from "@zandor/ui";
import { ATOMS, INFRASTRUCTURE, MOLECULES } from "@zandor/ui";
import { afterEach, describe, expect, test } from "vitest";
import { App, hashToPath, parseRoute } from "../App";
import { createServices } from "../shell/services";
import { SECTIONS } from "./DesignSystemPage";
import { ATOM_DEMOS, MOLECULE_DEMOS } from "./demos";

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

/** Nama katalog yang diekspor dengan nama lain. */
const EXPORT_ALIAS: Record<string, string[]> = {
  Toast: ["ToastProvider", "useToast"],
  ConfirmDialog: ["ConfirmProvider", "useConfirm"],
};

describe("katalog komponen", () => {
  test("setiap ekspor komponen (PascalCase) terdaftar di katalog", () => {
    const registered = new Set<string>([...ATOMS, ...MOLECULES, ...INFRASTRUCTURE]);
    const components = Object.entries(ui)
      .filter(([name, value]) => /^[A-Z][a-z]/.test(name) && !name.endsWith("Icon") && typeof value !== "undefined")
      .map(([name]) => name);
    const missing = components.filter((n) => !registered.has(n));
    expect(missing).toEqual([]);
  });

  test("setiap nama katalog benar-benar diekspor", () => {
    const exported = new Set(Object.keys(ui));
    for (const name of [...ATOMS, ...MOLECULES, ...INFRASTRUCTURE]) {
      const names = EXPORT_ALIAS[name] ?? [name];
      expect(
        names.some((n) => exported.has(n)),
        name,
      ).toBe(true);
    }
  });

  test("demo == katalog (tidak ada yang kurang, tidak ada yang lebih)", () => {
    expect(Object.keys(ATOM_DEMOS).sort()).toEqual([...ATOMS].sort());
    expect(Object.keys(MOLECULE_DEMOS).sort()).toEqual([...MOLECULES].sort());
  });
});

describe("halaman design system", () => {
  const renderPage = () => {
    const services = createServices({
      logger: createLogger("test", { sinks: [] }),
      initialPath: "/design-system",
      storage: { get: () => null, set: () => {} },
    });
    return render(<App services={services} />);
  };

  test("bagian sesuai urutan roadmap: Tokens → Atoms → Molecules → Organisms → Panels → Menus → Modals → Pages", () => {
    const { container } = renderPage();
    const ids = [...container.querySelectorAll("main section.ds-section")].map((s) => s.id);
    expect(ids).toEqual(SECTIONS.map((s) => s.id));
    expect(ids.slice(0, 8)).toEqual([
      "tokens",
      "atoms",
      "molecules",
      "organisms",
      "panels",
      "menus",
      "modals",
      "pages",
    ]);
  });

  test("setiap komponen punya demo di halaman", () => {
    const { container } = renderPage();
    const shown = new Set([...container.querySelectorAll("[data-demo]")].map((e) => e.getAttribute("data-demo")));
    for (const name of [...ATOMS, ...MOLECULES]) expect(shown.has(name), `demo ${name}`).toBe(true);
  });

  test("Menu dan ContextMenu ada di Menus, Dialog dan ConfirmDialog di Modals", () => {
    const { container } = renderPage();
    const demosIn = (id: string) =>
      [...(container.querySelector(`#${id}`)?.querySelectorAll("[data-demo]") ?? [])].map((e) =>
        e.getAttribute("data-demo"),
      );
    expect(demosIn("menus")).toEqual(["Menu", "ContextMenu"]);
    expect(demosIn("modals")).toEqual(["Dialog", "ConfirmDialog"]);
  });

  test("tingkat yang belum ada ditandai jujur, tanpa contoh palsu", () => {
    const { container } = renderPage();
    for (const id of ["organisms", "panels", "pages"]) {
      const section = container.querySelector(`#${id}`) as HTMLElement;
      expect(within(section).getByText("Belum ada")).toBeInTheDocument();
      expect(section.querySelectorAll("[data-demo]")).toHaveLength(0);
    }
  });

  test("Shortcut menampilkan binding nyata yang terdaftar", () => {
    renderPage();
    const section = document.getElementById("shortcuts") as HTMLElement;
    for (const id of [
      "cmd:view.toggleTheme",
      "cmd:view.zoomIn",
      "cmd:view.zoomOut",
      "cmd:view.zoomReset",
      "cmd:app.palette",
    ]) {
      expect(within(section).getByText(id)).toBeInTheDocument();
    }
  });

  test("token warna ditampilkan dari CSS (tanpa nilai tertulis di halaman)", () => {
    renderPage();
    expect(screen.getByTestId("swatches").querySelectorAll("figure")).toHaveLength(16);
  });
});

describe("routing", () => {
  test.each([
    ["/", { page: "home" }],
    ["/settings", { page: "settings" }],
    ["/settings/", { page: "settings" }],
    ["/design-system", { page: "design-system", section: null }],
    ["/design-system/atoms", { page: "design-system", section: "atoms" }],
    ["/design-system/atoms/", { page: "design-system", section: "atoms" }],
    ["/design-systemx", { page: "home" }],
    ["/lain", { page: "home" }],
  ])("%j", (path, want) => {
    expect(parseRoute(path)).toEqual(want);
  });

  test.each([
    ["", "/"],
    ["#", "/"],
    ["#/", "/"],
    ["#/settings", "/settings"],
    ["#/design-system/atoms", "/design-system/atoms"],
    ["#acak", "/"],
  ])("hashToPath(%j) = %j", (hash, want) => {
    expect(hashToPath(hash)).toBe(want);
  });
});
