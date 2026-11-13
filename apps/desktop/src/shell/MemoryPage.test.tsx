// @vitest-environment jsdom
// Uji komponen halaman Memori & Skill terhadap host tiruan yang meniru aturan host Rust.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  ActivityLog,
  createFakeMemoryHost,
  createLogger,
  type FakeMemoryHost,
  parseQueue,
  serializeQueue,
  skillMarkdown,
} from "@zandor/core";
import { afterEach, describe, expect, test } from "vitest";
import { App } from "../App";
import { createBrowserDocHost } from "./doc-host";
import { createBrowserHost } from "./engine-host";
import { createUnavailableGitHost } from "./git-host";
import { createBrowserProviderHost } from "./provider-host";
import { createServices } from "./services";

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

const GOOD = {
  name: "tulis-changelog",
  description: "Dipakai saat membuat catatan rilis dari git log.",
  body: "1. Baca git log sejak tag terakhir.\n2. Kelompokkan per tipe commit.",
};
const entry = (id: string, extra: object = {}) => ({
  id,
  t: 1_700_000_000_000,
  status: "pending",
  trigger: "correction",
  excerpt: "salah, bukan begitu",
  ...GOOD,
  ...extra,
});

function mount(seed: (h: FakeMemoryHost) => void = () => {}, connected = true) {
  const memoryHost = createFakeMemoryHost({ connected });
  seed(memoryHost);
  const services = createServices({
    logger: createLogger("test", { sinks: [] }),
    initialPath: "/memory",
    host: createBrowserHost(),
    providerHost: createBrowserProviderHost(),
    docHost: createBrowserDocHost(() => null),
    gitHost: createUnavailableGitHost(),
    memoryHost,
    activity: new ActivityLog(),
    storage: { get: () => JSON.stringify({ language: "id" }), set: () => {} },
  });
  render(<App services={services} />);
  return { host: memoryHost, services };
}

const openTab = async (name: RegExp) => {
  fireEvent.click(await screen.findByRole("tab", { name }));
};

describe("halaman Memori & Skill", () => {
  test("pratinjau browser diberi label jujur; terhubung → tanpa label", async () => {
    mount(() => {}, false);
    expect(await screen.findByText(/TIDAK dibaca engine/)).toBeInTheDocument();
    cleanup();
    mount(() => {}, true);
    await screen.findByRole("tab", { name: "Pengaturan" });
    expect(screen.queryByText(/TIDAK dibaca engine/)).toBeNull();
  });

  test("pengaturan: opt-in bawaan mati; mengaktifkan menulis settings.json", async () => {
    const { host } = mount();
    const sw = await screen.findByRole("switch", { name: "Aktifkan memori secara bawaan" });
    expect(sw).toHaveAttribute("aria-checked", "false");
    fireEvent.click(sw);
    await waitFor(() => expect(JSON.parse(host.files.get("settings.json") ?? "{}").defaultEnabled).toBe(true));
    fireEvent.click(screen.getByRole("switch", { name: "Aktifkan skill creator" }));
    await waitFor(() => expect(JSON.parse(host.files.get("settings.json") ?? "{}").skillCreator).toBe(true));
  });

  test("memori: simpan teks biasa; teks berisi secret ditolak dan tidak ditulis", async () => {
    const { host } = mount((h) => h.files.set("global.md", "- awal\n"));
    await openTab(/^Memori$/);
    const box = await screen.findByRole("textbox", { name: "Memori global" });
    await waitFor(() => expect(box).toHaveValue("- awal\n"));
    fireEvent.change(box, { target: { value: "- kunci sk-CONTOHPALSU0123456789abcdefghij\n" } });
    fireEvent.click(screen.getByRole("button", { name: "Simpan" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/memuat secret/);
    expect(host.files.get("global.md")).toBe("- awal\n");
    fireEvent.change(box, { target: { value: "- Jawab singkat\n" } });
    fireEvent.click(screen.getByRole("button", { name: "Simpan" }));
    await waitFor(() => expect(host.files.get("global.md")).toBe("- Jawab singkat\n"));
  });

  test("antrean: draf pending tampil; Setujui memasang SKILL.md dan menandai Disetujui", async () => {
    const { host } = mount((h) => h.files.set("skill-queue.jsonl", serializeQueue([entry("a")] as never)));
    await openTab(/^Antrean skill/);
    const card = await screen.findByRole("listitem", { name: "tulis-changelog" });
    expect(within(card).getByText("Menunggu persetujuan")).toBeInTheDocument();
    expect(within(card).getByText(/Koreksi dari Anda/)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Setujui & pasang" }));
    await waitFor(() => expect(host.skills.get("global||tulis-changelog")).toBe(skillMarkdown(GOOD)));
    await waitFor(() => expect(parseQueue(host.files.get("skill-queue.jsonl") ?? "")[0]?.status).toBe("approved"));
  });

  test("antrean: draf berisi secret tidak bisa dipasang; Tolak dan Hapus entri bekerja", async () => {
    const { host } = mount((h) =>
      h.files.set(
        "skill-queue.jsonl",
        serializeQueue([
          entry("a", { name: "bocor", body: "Gunakan Authorization: Bearer abcdefghijklmnop12345 di semua request" }),
          entry("b", { name: "aman-saja", t: 1_700_000_000_001 }),
        ] as never),
      ),
    );
    await openTab(/^Antrean skill/);
    const bad = await screen.findByRole("listitem", { name: "bocor" });
    fireEvent.click(within(bad).getByRole("button", { name: "Setujui & pasang" }));
    expect(await within(bad).findByRole("alert")).toHaveTextContent(/secret/);
    expect(host.skills.size).toBe(0);

    const ok = screen.getByRole("listitem", { name: "aman-saja" });
    fireEvent.click(within(ok).getByRole("button", { name: "Tolak" }));
    await waitFor(() => expect(within(ok).getByText("Ditolak")).toBeInTheDocument());
    fireEvent.click(within(ok).getByRole("button", { name: "Hapus entri" }));
    await waitFor(() => expect(parseQueue(host.files.get("skill-queue.jsonl") ?? "").map((e) => e.id)).toEqual(["a"]));
  });

  test("antrean: kandidat tanpa draf tidak punya tombol setujui; kosong → pesan jujur", async () => {
    mount((h) =>
      h.files.set(
        "skill-queue.jsonl",
        serializeQueue([{ id: "c", t: 1, status: "candidate", trigger: "tool-count", count: 5 }] as never),
      ),
    );
    await openTab(/^Antrean skill/);
    expect(await screen.findByText("Menunggu draf dari model")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Setujui & pasang" })).toBeNull();
    cleanup();
    mount();
    await openTab(/^Antrean skill/);
    expect(await screen.findByText("Belum ada usulan skill.")).toBeInTheDocument();
  });

  test("skill terpasang: ubah (divalidasi) dan hapus (dengan konfirmasi)", async () => {
    const { host } = mount((h) => h.skills.set("global||tulis-changelog", skillMarkdown(GOOD)));
    await openTab(/^Skill terpasang/);
    const row = await screen.findByRole("listitem", { name: "tulis-changelog" });
    fireEvent.click(within(row).getByRole("button", { name: "Ubah tulis-changelog" }));
    const box = await within(row).findByRole("textbox", { name: "tulis-changelog" });
    await waitFor(() => expect(box).toHaveValue(skillMarkdown(GOOD)));
    fireEvent.change(box, { target: { value: skillMarkdown({ ...GOOD, name: "nama-lain" }) } });
    fireEvent.click(within(row).getByRole("button", { name: "Simpan skill" }));
    expect(await within(row).findByRole("alert")).toHaveTextContent(/sama dengan nama folder/);
    const edited = skillMarkdown({ ...GOOD, body: `${GOOD.body}\n3. Tulis ke CHANGELOG.md.` });
    fireEvent.change(box, { target: { value: edited } });
    fireEvent.click(within(row).getByRole("button", { name: "Simpan skill" }));
    await waitFor(() => expect(host.skills.get("global||tulis-changelog")).toBe(edited));

    fireEvent.click(await screen.findByRole("button", { name: "Hapus skill tulis-changelog" }));
    fireEvent.click(await screen.findByRole("button", { name: "Hapus skill" }));
    await waitFor(() => expect(host.skills.size).toBe(0));
    expect(await screen.findByText("Belum ada skill terpasang.")).toBeInTheDocument();
  });
});
