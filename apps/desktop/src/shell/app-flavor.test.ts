import { describe, expect, test } from "vitest";
import { isDevProductName } from "./app-flavor";

describe("isDevProductName", () => {
  test("hanya produk berakhiran Dev", () => {
    expect(isDevProductName("Zandor Dev")).toBe(true);
    expect(isDevProductName("zandor dev ")).toBe(true);
    expect(isDevProductName("Zandor")).toBe(false);
    expect(isDevProductName("Zandor Developer Tools")).toBe(false);
    expect(isDevProductName("Dev")).toBe(false);
  });
});
