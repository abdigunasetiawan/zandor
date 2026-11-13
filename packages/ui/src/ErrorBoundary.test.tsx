// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createLogger, createMemorySink } from "@zandor/core";
import { I18nProvider } from "@zandor/i18n/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

afterEach(cleanup);

let shouldThrow = true;
function Bom() {
  if (shouldThrow) throw new Error("bom token sk-abcdefghijklmnop");
  return <p>isi panel</p>;
}

describe("ErrorBoundary", () => {
  test("mengisolasi galat ke satu panel, mencatat log tersamar, dan bisa dicoba lagi", () => {
    vi.spyOn(console, "error").mockImplementation(() => {}); // React mencetak galat render ke konsol
    shouldThrow = true;
    const mem = createMemorySink();
    const logger = createLogger("ui", { sinks: [mem.sink] });
    render(
      <I18nProvider locale="id">
        <ErrorBoundary name="panel.chat" logger={logger}>
          <Bom />
        </ErrorBoundary>
        <ErrorBoundary name="panel.dokumen" logger={logger}>
          <p>panel lain tetap hidup</p>
        </ErrorBoundary>
      </I18nProvider>,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Bagian ini mengalami galat");
    expect(screen.getByText("panel lain tetap hidup")).toBeInTheDocument();
    const entry = mem.entries().find((e) => e.level === "error");
    expect(entry?.msg).toBe("render gagal di panel.chat");
    expect(JSON.stringify(entry?.data)).not.toContain("sk-abcdefghijklmnop");

    shouldThrow = false;
    act(() => fireEvent.click(screen.getByRole("button", { name: "Coba lagi" })));
    expect(screen.getByText("isi panel")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  test("teks fallback mengikuti bahasa", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    shouldThrow = true;
    render(
      <I18nProvider locale="en">
        <ErrorBoundary name="p" logger={createLogger("ui", { sinks: [] })}>
          <Bom />
        </ErrorBoundary>
      </I18nProvider>,
    );
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });
});
