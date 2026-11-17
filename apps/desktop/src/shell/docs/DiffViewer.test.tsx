// @vitest-environment jsdom
// DiffViewer: Tolak menulis berkas (sumber yang bisa ditulis), Urungkan memulihkan, Terima tidak menulis.
// Monaco diganti stub (jsdom tidak bisa menjalankannya); yang diuji adalah alur data dan penulisan berkas.
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ActivityLog, createFakeMemoryHost, createLogger, type DocStore, type FileSource } from "@zandor/core";
import { I18nProvider } from "@zandor/i18n/react";
import { ConfirmProvider, ToastProvider } from "@zandor/ui";
import { afterEach, describe, expect, test, vi } from "vitest";
import { createBrowserDocHost } from "../doc-host";
import { createBrowserHost } from "../engine-host";
import { createUnavailableGitHost } from "../git-host";
import { createBrowserNotifyHost } from "../notify-host";
import { createUnavailableOpenVsxHost } from "../openvsx-host";
import { createBrowserProviderHost } from "../provider-host";
import { createServices, ServicesProvider } from "../services";
import DiffViewer from "./DiffViewer";

const models: string[] = [];
vi.mock("./monaco-setup", () => {
  const editor = () => ({
    getAction: () => null,
    getModel: () => null,
    revealLinesInCenter: () => {},
    setPosition: () => {},
  });
  return {
    applyEditorTheme: () => "vs-dark",
    monacoLanguage: (l: string) => l,
    monaco: {
      editor: {
        setTheme: () => {},
        createModel: (text: string) => {
          models.push(text);
          return { dispose: () => {} };
        },
        createDiffEditor: () => ({
          getModel: () => null,
          setModel: () => {},
          updateOptions: () => {},
          dispose: () => {},
          getModifiedEditor: editor,
        }),
      },
    },
  };
});
vi.mock("./CodeViewer", () => ({ editorOptions: () => ({}) }));

const AFTER = "const a = 2;\nconst b = 1;\nconst c = 1;\nconst d = 1;\nconst e = 9;\n";
const PATCH =
  "@@ -1,2 +1,2 @@\n-const a = 1;\n+const a = 2;\n const b = 1;\n@@ -4,2 +4,2 @@\n const d = 1;\n-const e = 5;\n+const e = 9;\n";

function View({ docs, id }: { docs: DocStore; id: string }) {
  const tab = docs.state().tabs.find((t) => t.id === id);
  const bytes = docs.content(id);
  if (!tab || !bytes) return null;
  return <DiffViewer tab={tab} bytes={bytes} onHandle={() => {}} />;
}

async function mount(writable: boolean) {
  document.documentElement.dataset.theme = "dark";
  const disk = { text: AFTER, writes: [] as string[] };
  const browser = createBrowserDocHost(() => null);
  const source: FileSource = {
    read: async () => new TextEncoder().encode(disk.text),
    ...(writable
      ? {
          write: async (_r: string, _p: string, text: string) => {
            disk.text = text;
            disk.writes.push(text);
            return Date.now();
          },
        }
      : {}),
  };
  const services = createServices({
    logger: createLogger("test", { sinks: [] }),
    initialPath: "/",
    host: createBrowserHost(),
    providerHost: createBrowserProviderHost(),
    docHost: { ...browser, source },
    gitHost: createUnavailableGitHost(),
    memoryHost: createFakeMemoryHost({ connected: false }),
    appearanceStorage: { get: () => null, set: () => {} },
    automationStorage: { get: () => null, set: () => {} },
    notifyHost: createBrowserNotifyHost(),
    openVsxHost: createUnavailableOpenVsxHost(),
    activity: new ActivityLog(),
    storage: { get: () => null, set: () => {} },
  });
  const id = services.docs.openDiff("C:/proj", "src/app.ts", PATCH);
  await waitFor(() => expect(services.docs.content(id)).not.toBeNull());
  const ui = () => (
    <I18nProvider locale="id">
      <ServicesProvider services={services}>
        <ToastProvider>
          <ConfirmProvider>
            <View docs={services.docs} id={id} />
          </ConfirmProvider>
        </ToastProvider>
      </ServicesProvider>
    </I18nProvider>
  );
  const r = render(ui());
  services.docs.subscribe(() => r.rerender(ui()));
  return { disk, services, id };
}

afterEach(() => {
  cleanup();
  models.length = 0;
});

describe("DiffViewer", () => {
  test("sisi sebelum = patch dibalik; Terima tidak menulis berkas", async () => {
    const { disk, services, id } = await mount(true);
    expect(screen.getByTestId("diff-pos")).toHaveTextContent("Bagian 1 dari 2");
    expect(models).toContain("const a = 1;\nconst b = 1;\nconst c = 1;\nconst d = 1;\nconst e = 5;\n");
    fireEvent.click(screen.getByRole("button", { name: /^Terima$/ }));
    await waitFor(() => expect(screen.getByTestId("diff-pos")).toHaveTextContent("Bagian 1 dari 1"));
    expect(disk.writes).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Terima semua" }));
    await waitFor(() => expect(services.docs.state().tabs.find((t) => t.id === id)?.diff ?? null).toBeNull());
    expect(disk.writes).toHaveLength(0);
  });

  test("Tolak mengembalikan satu bagian dan menyimpan; Urungkan memulihkan isi dan diff", async () => {
    const { disk } = await mount(true);
    fireEvent.click(screen.getByRole("button", { name: "Bagian berikutnya" }));
    fireEvent.click(screen.getByRole("button", { name: /^Tolak$/ }));
    await waitFor(() => expect(disk.writes).toHaveLength(1));
    expect(disk.text).toBe("const a = 2;\nconst b = 1;\nconst c = 1;\nconst d = 1;\nconst e = 5;\n");
    await waitFor(() => expect(screen.getByTestId("diff-pos")).toHaveTextContent("Bagian 1 dari 1"));
    fireEvent.click(await screen.findByRole("button", { name: "Urungkan" }));
    await waitFor(() => expect(disk.text).toBe(AFTER));
    // bagian yang diurungkan kembali ke daftar dan tetap terpilih
    await waitFor(() => expect(screen.getByTestId("diff-pos")).toHaveTextContent("Bagian 2 dari 2"));
  });

  test("Tolak semua meminta konfirmasi lalu memulihkan seluruh isi semula", async () => {
    const { disk } = await mount(true);
    fireEvent.click(screen.getByRole("button", { name: "Tolak semua" }));
    fireEvent.click((await screen.findAllByRole("button", { name: "Tolak semua" })).at(-1) as HTMLElement);
    await waitFor(() => expect(disk.writes).toHaveLength(1));
    expect(disk.text).toBe("const a = 1;\nconst b = 1;\nconst c = 1;\nconst d = 1;\nconst e = 5;\n");
  });

  test("tanpa penulisan berkas: Tolak nonaktif dengan alasan", async () => {
    await mount(false);
    expect(screen.getByRole("button", { name: /^Tolak$/ })).toBeDisabled();
    expect(screen.getByText(/hanya tersedia di aplikasi desktop/)).toBeInTheDocument();
  });
});
