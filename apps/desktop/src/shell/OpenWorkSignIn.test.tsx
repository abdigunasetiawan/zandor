// @vitest-environment jsdom
// Uji kartu inline "Masuk dengan OpenWork" (tab Sign-in) dengan host tiruan; penjadwal polling memakai timer palsu vitest.
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ActivityLog, createFakeMemoryHost, createLogger, type OpenWorkPoll, type ProviderHost } from "@zandor/core";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { App } from "../App";
import { createBrowserDocHost } from "./doc-host";
import { createBrowserHost } from "./engine-host";
import { createUnavailableGitHost } from "./git-host";
import { createBrowserNotifyHost } from "./notify-host";
import { createUnavailableOpenVsxHost } from "./openvsx-host";
import { createBrowserProviderHost } from "./provider-host";
import { createServices } from "./services";

beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  window.location.hash = "";
});

function mount(script: OpenWorkPoll[], withOpenWork = true) {
  const calls: string[] = [];
  const opened: string[] = [];
  const queue = [...script];
  const base = createBrowserProviderHost();
  const providerHost: ProviderHost = {
    ...base,
    ...(withOpenWork && {
      openwork: {
        async start() {
          calls.push("start");
          return {
            userCode: "ABCD-1234",
            verificationUri: "https://app.openworklabs.com/device?user_code=ABCD1234",
            interval: 5,
            expiresIn: 600,
          };
        },
        async poll() {
          calls.push("poll");
          return queue.shift() ?? { status: "pending" };
        },
        async cancel() {
          calls.push("cancel");
        },
      },
    }),
    async openExternal(url) {
      opened.push(url);
    },
  };
  const services = createServices({
    logger: createLogger("test", { sinks: [] }),
    initialPath: "/settings/models",
    host: createBrowserHost(),
    providerHost,
    docHost: createBrowserDocHost(() => null),
    gitHost: createUnavailableGitHost(),
    memoryHost: createFakeMemoryHost({ connected: false }),
    appearanceStorage: { get: () => null, set: () => {} },
    automationStorage: { get: () => null, set: () => {} },
    notifyHost: createBrowserNotifyHost(),
    openVsxHost: createUnavailableOpenVsxHost(),
    activity: new ActivityLog(),
    storage: { get: () => JSON.stringify({ language: "id" }), set: () => {} },
  });
  render(<App services={services} />);
  return { calls, opened, services };
}

async function openSignIn() {
  fireEvent.click(await screen.findByRole("button", { name: "Tambah koneksi" }));
  fireEvent.click(await screen.findByRole("tab", { name: "Sign-in" }));
}

describe("Masuk dengan OpenWork (kartu inline di tab Sign-in)", () => {
  test("build web (tanpa host OpenWork): dijelaskan hanya untuk aplikasi desktop, tanpa tombol masuk", async () => {
    mount([], false);
    await openSignIn();
    expect(await screen.findByText(/hanya tersedia di aplikasi desktop/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Masuk dengan OpenWork/ })).toBeNull();
  });

  test("alur: kode tampil, halaman verifikasi dibuka, polling sampai tersambung, tanpa popup", async () => {
    const { calls, opened } = mount([{ status: "pending" }, { status: "connected", account: "Budi · Tim", models: 7 }]);
    await openSignIn();
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: /Masuk dengan OpenWork/ }));
    const code = await screen.findByTestId("openwork-code");
    expect(code).toHaveTextContent("ABCD-1234");
    expect(screen.getByText(/Menunggu persetujuan/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Buka halaman OpenWork" }));
    expect(opened).toEqual(["https://app.openworklabs.com/device?user_code=ABCD1234"]);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(screen.getByTestId("openwork-code")).toBeInTheDocument(); // masih menunggu
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(await screen.findByText(/Tersambung: Budi · Tim\. 7 model tersedia\./)).toBeInTheDocument();
    expect(screen.queryByTestId("openwork-code")).toBeNull();
    expect(calls.filter((c) => c === "poll")).toHaveLength(2);
  });

  test("ditolak menampilkan pesan dan tombol Coba lagi yang meminta kode baru", async () => {
    const { calls } = mount([{ status: "denied", message: "Anda menolak permintaan masuk" }]);
    await openSignIn();
    fireEvent.click(await screen.findByRole("button", { name: /Masuk dengan OpenWork/ }));
    await screen.findByTestId("openwork-code");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("Anda menolak permintaan masuk");
    fireEvent.click(screen.getByRole("button", { name: /Coba lagi/ }));
    expect(await screen.findByTestId("openwork-code")).toBeInTheDocument();
    expect(calls.filter((c) => c === "start")).toHaveLength(2);
  });

  test("Batal membatalkan alur di host dan menyembunyikan kode", async () => {
    const { calls } = mount([]);
    await openSignIn();
    fireEvent.click(await screen.findByRole("button", { name: /Masuk dengan OpenWork/ }));
    await screen.findByTestId("openwork-code");
    fireEvent.click(screen.getByRole("button", { name: "Batal" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(calls).toContain("cancel");
    expect(screen.queryByTestId("openwork-code")).toBeNull();
  });
});
