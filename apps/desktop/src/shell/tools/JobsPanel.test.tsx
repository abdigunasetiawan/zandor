// @vitest-environment jsdom
// Uji panel Jobs (ADR-072) terhadap host tiruan yang meniru berkas job dari plugin engine.
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ActivityLog, createFakeMemoryHost, createLogger } from "@zandor/core";
import { I18nProvider } from "@zandor/i18n/react";
import { afterEach, describe, expect, test } from "vitest";
import { createBrowserDocHost } from "../doc-host";
import { createBrowserHost } from "../engine-host";
import { createUnavailableGitHost } from "../git-host";
import { type JobAction, type JobInfo, type JobsHost, toJob } from "../jobs-host";
import { createBrowserNotifyHost } from "../notify-host";
import { createUnavailableOpenVsxHost } from "../openvsx-host";
import { createBrowserProviderHost } from "../provider-host";
import { createServices, ServicesProvider } from "../services";
import { JobsPanel } from "./JobsPanel";

afterEach(cleanup);

const job = (extra: Partial<JobInfo>): JobInfo => ({
  id: "job-aaaaaa11",
  name: "vite dev",
  command: "npm run dev",
  cwd: "C:\\proj",
  sessionID: "s1",
  directory: "C:\\proj",
  pid: 1,
  status: "running",
  exitCode: null,
  ports: [5173],
  startedAt: 1,
  endedAt: null,
  restarts: 0,
  ...extra,
});

function mount(host: JobsHost | null) {
  const services = createServices({
    logger: createLogger("test", { sinks: [] }),
    initialPath: "/",
    host: createBrowserHost(),
    providerHost: createBrowserProviderHost(),
    docHost: createBrowserDocHost(() => null),
    gitHost: createUnavailableGitHost(),
    memoryHost: createFakeMemoryHost({ connected: false }),
    appearanceStorage: { get: () => null, set: () => {} },
    automationStorage: { get: () => null, set: () => {} },
    notifyHost: createBrowserNotifyHost(),
    openVsxHost: createUnavailableOpenVsxHost(),
    activity: new ActivityLog(),
    storage: { get: () => null, set: () => {} },
  });
  render(
    <I18nProvider locale="id">
      <ServicesProvider services={services}>
        <JobsPanel host={host} />
      </ServicesProvider>
    </I18nProvider>,
  );
}

function fakeHost(jobs: JobInfo[]) {
  const calls: Array<[string, JobAction]> = [];
  const host: JobsHost = {
    list: async () => jobs,
    log: async () => "ready on http://localhost:5173",
    control: async (id, action) => void calls.push([id, action]),
  };
  return { host, calls };
}

describe("panel Jobs", () => {
  test("di luar aplikasi desktop menjelaskan, tanpa data palsu", () => {
    mount(null);
    expect(screen.getByText(/aplikasi desktop/)).toBeInTheDocument();
  });

  test("kosong → petunjuk cara memakai", async () => {
    mount(fakeHost([]).host);
    expect(await screen.findByText(/Belum ada background job/)).toBeInTheDocument();
  });

  test("job berjalan: status, port, log, dan stop", async () => {
    const { host, calls } = fakeHost([
      job({}),
      job({ id: "job-bbbbbb22", name: "tsc", status: "failed", exitCode: 2, ports: [] }),
    ]);
    mount(host);
    expect(await screen.findByText("vite dev")).toBeInTheDocument();
    expect(screen.getByText("Gagal (kode 2)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Buka localhost:5173 di Browser" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: /vite dev/ }));
    expect(await screen.findByText("ready on http://localhost:5173")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Hentikan job" }));
    fireEvent.click(screen.getByRole("button", { name: "Hapus job dari daftar" }));
    await waitFor(() =>
      expect(calls).toEqual([
        ["job-aaaaaa11", "stop"],
        ["job-bbbbbb22", "remove"],
      ]),
    );
  });

  test("meta rusak dari berkas ditolak atau dinormalkan", () => {
    expect(toJob({ id: 1 })).toBeNull();
    expect(toJob({ id: "job-x", command: "a", status: "aneh", ports: [1, "2"] })).toMatchObject({
      status: "lost",
      ports: [1],
      name: "a",
    });
  });
});
