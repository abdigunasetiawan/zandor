// @vitest-environment jsdom
// Uji komponen tab Editor (impor berkas, Open VSX dengan host tiruan) dan Pintasan pada halaman Tampilan.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ActivityLog, createFakeMemoryHost, createLogger, type OpenVsxHost, type OpenVsxItem } from "@zandor/core";
import { strToU8, zipSync } from "fflate";
import { afterEach, describe, expect, test } from "vitest";
import { App } from "../App";
import { createBrowserDocHost } from "./doc-host";
import { createBrowserHost } from "./engine-host";
import { createUnavailableGitHost } from "./git-host";
import { createUnavailableOpenVsxHost } from "./openvsx-host";
import { createBrowserProviderHost } from "./provider-host";
import { createServices } from "./services";

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

const THEME = JSON.stringify({
  name: "Senja",
  type: "dark",
  colors: { "editor.background": "#101820", "editor.foreground": "#f2e9dc" },
  tokenColors: [{ scope: "comment", settings: { foreground: "#6a9955" } }],
});

const ITEM: OpenVsxItem = {
  namespace: "dr",
  name: "dracula",
  display_name: "Dracula Resmi",
  version: "1.2.3",
  description: "Tema gelap populer",
  download: "https://open-vsx.org/api/dr/dracula/1.2.3/file/dr.dracula-1.2.3.vsix",
  downloads: 1234,
};

function fakeVsx(files: Record<string, string>, results: OpenVsxItem[] = [ITEM]): OpenVsxHost & { calls: string[] } {
  const calls: string[] = [];
  const bytes = zipSync(Object.fromEntries(Object.entries(files).map(([k, v]) => [k, strToU8(v)])));
  return {
    available: true,
    calls,
    async search(q) {
      calls.push(`search:${q}`);
      return results;
    },
    async download(url) {
      calls.push(`download:${url}`);
      return bytes;
    },
  };
}

function mount(openVsxHost: OpenVsxHost = createUnavailableOpenVsxHost()) {
  const services = createServices({
    logger: createLogger("test", { sinks: [] }),
    initialPath: "/appearance",
    host: createBrowserHost(),
    providerHost: createBrowserProviderHost(),
    docHost: createBrowserDocHost(() => null),
    gitHost: createUnavailableGitHost(),
    memoryHost: createFakeMemoryHost({ connected: false }),
    appearanceStorage: { get: () => null, set: () => {} },
    openVsxHost,
    activity: new ActivityLog(),
    storage: { get: () => JSON.stringify({ language: "id" }), set: () => {} },
  });
  render(<App services={services} />);
  return services;
}

const openTab = async (name: RegExp | string) => fireEvent.click(await screen.findByRole("tab", { name }));

describe("tab Editor", () => {
  test("tanpa host Open VSX: pesan jujur; impor berkas lokal tetap jalan dan memilih tema", async () => {
    const s = mount();
    await openTab("Editor");
    expect(await screen.findByText(/hanya tersedia di aplikasi desktop/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cari" })).toBeNull();
    const input = screen.getByLabelText("Impor tema VS Code (.json)") as HTMLInputElement;
    fireEvent.change(input, { target: { files: [new File([THEME], "senja.json", { type: "application/json" })] } });
    await waitFor(() => expect(s.appearance.state().editorTheme).toBe("senja"));
    expect(s.appearance.state().editorThemes[0]?.colors["editor.background"]).toBe("#101820");
    expect(await screen.findByRole("listitem", { name: "senja" })).toBeInTheDocument();
  });

  test("berkas tema jahat/rusak ditolak dengan pesan, tidak tersimpan", async () => {
    const s = mount();
    await openTab("Editor");
    const input = screen.getByLabelText("Impor tema VS Code (.json)") as HTMLInputElement;
    fireEvent.change(input, { target: { files: [new File(["{rusak"], "x.json")] } });
    expect(await screen.findByRole("alert")).toHaveTextContent(/bukan JSON tema/);
    expect(s.appearance.state().editorThemes).toEqual([]);
  });

  test("Open VSX: cari → pasang → tema terpasang dan terpilih; yang dilewati dilaporkan", async () => {
    const pkg = {
      contributes: {
        themes: [
          { label: "Dracula Gelap", uiTheme: "vs-dark", path: "./t/dark.json" },
          { label: "Hilang", path: "./t/none.json" },
        ],
      },
    };
    const vsx = fakeVsx({
      "extension/package.json": JSON.stringify(pkg),
      "extension/t/dark.json": '{"colors":{"editor.background":"#282a36"},"tokenColors":[]}',
    });
    const s = mount(vsx);
    await openTab("Editor");
    fireEvent.change(screen.getByLabelText("Cari tema di Open VSX", { selector: "input" }), {
      target: { value: "dracula" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Cari" }));
    const card = await screen.findByRole("listitem", { name: "Dracula Resmi" });
    expect(within(card).getByText(/1234 unduhan/)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Pasang Dracula Resmi" }));
    await waitFor(() => expect(s.appearance.state().editorTheme).toBe("dracula-gelap"));
    expect(vsx.calls).toEqual(["search:dracula", `download:${ITEM.download}`]);
    expect(s.appearance.state().editorThemes[0]?.source).toBe("openvsx:dr.dracula@1.2.3");
    expect(await screen.findByRole("alert")).toHaveTextContent(/Hilang.*tidak ada/);
  });

  test("Open VSX: paket tanpa tema / unduhan gagal → galat jelas, tidak ada yang terpasang", async () => {
    const s = mount(fakeVsx({ "extension/package.json": '{"contributes":{}}' }));
    await openTab("Editor");
    fireEvent.change(screen.getByLabelText("Cari tema di Open VSX", { selector: "input" }), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: "Cari" }));
    fireEvent.click(await screen.findByRole("button", { name: "Pasang Dracula Resmi" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/tidak memuat tema/);
    expect(s.appearance.state().editorThemes).toEqual([]);
  });
});

describe("tab Pintasan", () => {
  test("ubah combo lewat tangkap tombol; bentrok ditolak; Esc membatalkan; reset ke bawaan", async () => {
    const s = mount();
    await openTab("Pintasan");
    const key = await screen.findByTestId("key-cmd:app.openSettings");
    expect(key).toHaveTextContent("Ctrl+,");

    fireEvent.click(screen.getByRole("button", { name: "Ubah cmd:app.openSettings" }));
    fireEvent.keyDown(window, { key: "Escape" });
    expect(key).toHaveTextContent("Ctrl+,");

    fireEvent.click(screen.getByRole("button", { name: "Ubah cmd:app.openSettings" }));
    fireEvent.keyDown(window, { key: "x" }); // tanpa modifier → ditolak
    expect(await screen.findByRole("alert")).toHaveTextContent(/Ctrl\/Alt/);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true }); // Ctrl+K dipakai palet → bentrok
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(/bentrok/));
    expect(s.appearance.state().shortcuts).toEqual({});

    fireEvent.keyDown(window, { key: "9", ctrlKey: true, altKey: true });
    await waitFor(() => expect(key).toHaveTextContent("Ctrl+Alt+9"));
    expect(s.appearance.state().shortcuts).toEqual({ "cmd:app.openSettings": "Ctrl+Alt+9" });

    fireEvent.click(screen.getByRole("button", { name: "Bawaan cmd:app.openSettings" }));
    await waitFor(() => expect(key).toHaveTextContent("Ctrl+,"));
    expect(s.appearance.state().shortcuts).toEqual({});
  });
});
