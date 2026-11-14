// @vitest-environment jsdom
// Uji menu "Buka dengan" dengan DocHost tiruan: daftar aplikasi + ikon, peluncuran, galat, dan keadaan tak tersedia.
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@zandor/i18n/react";
import { ToastProvider } from "@zandor/ui";
import { afterEach, describe, expect, test, vi } from "vitest";
import { createBrowserDocHost, type DocHost, type OpenApp } from "../doc-host";
import { OpenWithMenu } from "./OpenWithMenu";

afterEach(cleanup);

const APPS: OpenApp[] = [
  { name: "Microsoft Word", path: "C:\\Office\\WINWORD.EXE", icon: "AAAA", isDefault: true },
  { name: "Visual Studio Code", path: "C:\\Code\\Code.exe", icon: "BBBB", isDefault: false },
  { name: "Tanpa ikon", path: "C:\\x\\x.exe", icon: "", isDefault: false },
];

function host(over: Partial<DocHost> = {}): DocHost {
  return {
    ...createBrowserDocHost(() => null),
    native: true,
    openExternal: vi.fn(async () => {}),
    openWithList: vi.fn(async () => APPS),
    openWith: vi.fn(async () => {}),
    reveal: vi.fn(async () => {}),
    ...over,
  };
}

function mount(h: DocHost) {
  return render(
    <I18nProvider locale="id">
      <ToastProvider>
        <OpenWithMenu host={h} root={"C:\\Work"} path="laporan.docx" textLike={false} />
      </ToastProvider>
    </I18nProvider>,
  );
}

describe("OpenWithMenu", () => {
  test("membuka popover, memuat aplikasi dengan ikon, dan meluncurkan yang dipilih", async () => {
    const h = host();
    mount(h);
    fireEvent.click(screen.getByRole("button", { name: "Buka di aplikasi lain" }));
    await screen.findByRole("button", { name: /Microsoft Word/ });
    expect(h.openWithList).toHaveBeenCalledWith("C:\\Work", "laporan.docx", false, false);
    expect(screen.getByText("Bawaan")).toBeTruthy(); // lencana aplikasi bawaan
    const icons = document.querySelectorAll<HTMLImageElement>("img.owmenu__icon");
    expect(icons).toHaveLength(2); // yang tanpa ikon memakai ikon cadangan, bukan <img> kosong
    expect(icons[0]?.src).toBe("data:image/png;base64,AAAA");
    fireEvent.click(screen.getByRole("button", { name: /Visual Studio Code/ }));
    await waitFor(() =>
      expect(h.openWith).toHaveBeenCalledWith("C:\\Work", "laporan.docx", "C:\\Code\\Code.exe", false),
    );
  });

  test("aplikasi bawaan sistem dan tampilkan di File Explorer", async () => {
    const h = host();
    mount(h);
    fireEvent.click(screen.getByRole("button", { name: "Buka di aplikasi lain" }));
    fireEvent.click(await screen.findByRole("button", { name: "Aplikasi bawaan sistem" }));
    await waitFor(() => expect(h.openExternal).toHaveBeenCalledWith("C:\\Work", "laporan.docx"));
    fireEvent.click(screen.getByRole("button", { name: "Buka di aplikasi lain" }));
    fireEvent.click(await screen.findByRole("button", { name: "Tampilkan di File Explorer" }));
    await waitFor(() => expect(h.reveal).toHaveBeenCalledWith("C:\\Work", "laporan.docx"));
  });

  test("galat membaca daftar: pesan jujur dan tombol muat ulang", async () => {
    const list = vi.fn().mockRejectedValueOnce(new Error("registri tidak terbaca")).mockResolvedValue(APPS);
    mount(host({ openWithList: list }));
    fireEvent.click(screen.getByRole("button", { name: "Buka di aplikasi lain" }));
    await screen.findByText(/registri tidak terbaca/);
    fireEvent.click(screen.getByRole("button", { name: "Muat ulang" }));
    await screen.findByRole("button", { name: /Microsoft Word/ });
    expect(list).toHaveBeenLastCalledWith("C:\\Work", "laporan.docx", false, true);
  });

  test("di luar aplikasi desktop tombol nonaktif dengan alasan jujur", () => {
    mount(createBrowserDocHost(() => null));
    const btn = screen.getByRole("button", { name: "Buka di aplikasi lain (hanya di aplikasi desktop)" });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });
});
