// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import * as ui from "./index";

afterEach(cleanup);

const ICONS = Object.entries(ui).filter(([name]) => name.endsWith("Icon")) as Array<
  [string, (p: { size?: "sm" | "md" | "lg" }) => React.JSX.Element]
>;

describe("ikon (Lucide)", () => {
  test("ada ikon yang diekspor", () => {
    expect(ICONS.length).toBeGreaterThanOrEqual(20);
  });

  test.each(ICONS)("%s: dekoratif (aria-hidden), tidak fokus, ketebalan seragam, kelas ukuran", (_name, Icon) => {
    const { container } = render(<Icon size="lg" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveAttribute("focusable", "false");
    expect(svg).toHaveAttribute("stroke-width", "1.5");
    expect(svg).toHaveClass("z-icon", "z-icon--lg");
  });

  test("ukuran bawaan md dan className tambahan diteruskan", () => {
    const { container } = render(<ui.SearchIcon className="ekstra" />);
    expect(container.querySelector("svg")).toHaveClass("z-icon--md", "ekstra");
  });
});
