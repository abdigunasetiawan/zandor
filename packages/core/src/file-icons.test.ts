import { describe, expect, test } from "vitest";
import { type IconManifest, iconFileName, resolveIconName } from "./file-icons";

const manifest: IconManifest = {
  file: "file",
  folder: "folder",
  folderExpanded: "folder-open",
  fileNames: { "package.json": "nodejs", dockerfile: "docker" },
  fileExtensions: { ts: "typescript", "test.ts": "typescript-test", md: "markdown" },
  folderNames: { src: "folder-src", node_modules: "folder-node" },
  folderNamesExpanded: { src: "folder-src-open" },
  iconDefinitions: Object.fromEntries(
    [
      "file",
      "folder",
      "folder-open",
      "nodejs",
      "docker",
      "typescript",
      "typescript-test",
      "markdown",
      "folder-src",
      "folder-src-open",
      "folder-node",
    ].map((n) => [n, { iconPath: `./../icons/${n}.svg` }]),
  ),
};
const file = (name: string) => resolveIconName(manifest, { name, type: "file" });
const dir = (name: string, open = false) => resolveIconName(manifest, { name, type: "directory" }, open);

describe("resolveIconName", () => {
  test("nama persis lebih diutamakan daripada ekstensi; tidak peka huruf besar/kecil", () => {
    expect(file("package.json")).toBe("nodejs");
    expect(file("Dockerfile")).toBe("docker");
  });
  test("ekstensi majemuk lebih dulu daripada ekstensi tunggal", () => {
    expect(file("main.test.ts")).toBe("typescript-test");
    expect(file("main.ts")).toBe("typescript");
    expect(file("a.b.c.md")).toBe("markdown");
  });
  test("tak dikenal → ikon berkas bawaan; tanpa ekstensi aman", () => {
    expect(file("data.xyz")).toBe("file");
    expect(file("LICENSE")).toBe("file");
    expect(file(".gitignore")).toBe("file");
  });
  test("folder: nama khusus, versi terbuka, dan bawaan", () => {
    expect(dir("src")).toBe("folder-src");
    expect(dir("src", true)).toBe("folder-src-open");
    expect(dir("src2")).toBe("folder");
    expect(dir("src2", true)).toBe("folder-open");
    expect(dir("node_modules", true)).toBe("folder-open"); // tak ada varian terbuka → bawaan terbuka
  });
  test("kunci prototipe (constructor, __proto__, toString) tidak dianggap cocok", () => {
    expect(file("constructor")).toBe("file");
    expect(file("a.__proto__")).toBe("file");
    expect(dir("toString")).toBe("folder");
  });
  test("ikon yang tidak ada di definisi jatuh ke bawaan", () => {
    const broken = { ...manifest, fileExtensions: { ts: "hilang" } };
    expect(resolveIconName(broken, { name: "a.ts", type: "file" })).toBe("file");
  });
});

describe("iconFileName", () => {
  test("mengambil nama berkas SVG dari iconPath", () => {
    expect(iconFileName(manifest, "typescript")).toBe("typescript.svg");
    expect(iconFileName(manifest, "tidak-ada")).toBeNull();
  });
});
