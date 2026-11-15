// @vitest-environment jsdom
// Tautan di respons model: klik membuka menu (browser Zandor / eksternal / salin), bukan navigasi langsung.
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@zandor/i18n/react";
import { ToastProvider } from "@zandor/ui";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { PanelBridge } from "../services";
import { BROWSER_OPEN_EVENT, takePendingBrowserUrl } from "../tools/browser-bridge";

const tauri = vi.hoisted(() => ({ isTauri: vi.fn(() => true), invoke: vi.fn(async () => undefined) }));
vi.mock("@tauri-apps/api/core", () => tauri);
const panels = new PanelBridge();
vi.mock("../services", async (orig) => ({
  ...(await orig<typeof import("../services")>()),
  useServices: () => ({ panels }),
}));

const { ChatLink, isWebLink } = await import("./ChatLink");

const actions = {
  toggle: vi.fn(),
  close: vi.fn(),
  openTool: vi.fn(),
  reveal: vi.fn(),
  reset: vi.fn(),
  smartResize: vi.fn(),
};

function mount(href = "https://contoh.dev/dok") {
  return render(
    <I18nProvider locale="id">
      <ToastProvider>
        <p>
          Lihat <ChatLink href={href}>dokumentasi</ChatLink>.
        </p>
      </ToastProvider>
    </I18nProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  tauri.isTauri.mockReturnValue(true);
  panels.setActions(actions);
  panels.update({ activeTool: "terminal" });
  takePendingBrowserUrl();
});
afterEach(cleanup);

describe("ChatLink", () => {
  test("hanya tautan http(s) yang memakai menu", () => {
    expect(isWebLink("https://a.dev")).toBe(true);
    expect(isWebLink("http://a.dev")).toBe(true);
    expect(isWebLink("mailto:a@b.c")).toBe(false);
    expect(isWebLink("#bagian")).toBe(false);
  });

  test("klik membuka menu; Buka di browser Zandor menampilkan Panel 4 dan meminta URL", async () => {
    const heard = vi.fn();
    window.addEventListener(BROWSER_OPEN_EVENT, heard);
    mount();
    const link = screen.getByRole("button", { name: "dokumentasi" });
    expect(link).toHaveAttribute("href", "https://contoh.dev/dok");
    fireEvent.click(link);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Buka di browser Zandor" }));
    expect(actions.openTool).toHaveBeenCalledWith("browser");
    expect(heard).toHaveBeenCalledOnce();
    expect(takePendingBrowserUrl()).toBe("https://contoh.dev/dok");
    window.removeEventListener(BROWSER_OPEN_EVENT, heard);
  });

  test("Browser sudah aktif: panel hanya dibuka, tidak di-toggle tertutup", async () => {
    panels.update({ activeTool: "browser" });
    mount();
    fireEvent.click(screen.getByRole("button", { name: "dokumentasi" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Buka di browser Zandor" }));
    expect(actions.reveal).toHaveBeenCalledWith("tools");
    expect(actions.openTool).not.toHaveBeenCalled();
  });

  test("Buka di browser eksternal memanggil open_external", async () => {
    mount();
    fireEvent.click(screen.getByRole("button", { name: "dokumentasi" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Buka di browser eksternal" }));
    await waitFor(() => expect(tauri.invoke).toHaveBeenCalledWith("open_external", { url: "https://contoh.dev/dok" }));
  });

  test("http di desktop: browser eksternal dinonaktifkan dengan alasan; di web browser Zandor dinonaktifkan", async () => {
    mount("http://contoh.dev");
    fireEvent.click(screen.getByRole("button", { name: "dokumentasi" }));
    expect(await screen.findByRole("menuitem", { name: /hanya tautan https/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    cleanup();
    tauri.isTauri.mockReturnValue(false);
    mount();
    fireEvent.click(screen.getByRole("button", { name: "dokumentasi" }));
    expect(await screen.findByRole("menuitem", { name: /hanya di aplikasi desktop/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });
});
