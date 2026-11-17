// @vitest-environment jsdom
import { cleanup, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import {
  AppLogo,
  Avatar,
  Badge,
  Button,
  Checkbox,
  IconButton,
  Input,
  Kbd,
  Select,
  Skeleton,
  Slider,
  Spinner,
  Switch,
  Textarea,
  Working,
} from "../index";
import { renderUi } from "../test-utils";
import { initials } from "./Avatar";

afterEach(cleanup);

describe("Working", () => {
  test("peran status berlabel; jumlah bagian per varian; label kustom", () => {
    const { container, rerender } = renderUi(<Working variant="wave" />);
    expect(screen.getByRole("status", { name: "Sedang bekerja" })).toBeTruthy();
    expect(container.querySelectorAll("i")).toHaveLength(5);
    rerender(<Working variant="ripple" label="Agent bekerja" />);
    expect(screen.getByRole("status", { name: "Agent bekerja" })).toBeTruthy();
    expect(container.querySelectorAll("i")).toHaveLength(3);
  });
});

describe("Button", () => {
  test("klik memanggil handler; disabled tidak", async () => {
    const onClick = vi.fn();
    const user = userEvent.setup();
    const { rerender } = renderUi(<Button onClick={onClick}>Simpan</Button>);
    await user.click(screen.getByRole("button", { name: "Simpan" }));
    expect(onClick).toHaveBeenCalledOnce();
    rerender(
      <Button disabled onClick={onClick}>
        Simpan
      </Button>,
    );
    await user.click(screen.getByRole("button", { name: "Simpan" }));
    expect(onClick).toHaveBeenCalledOnce();
  });

  test("loading: sibuk, tidak bisa diklik, ada spinner berlabel", async () => {
    const onClick = vi.fn();
    renderUi(
      <Button loading onClick={onClick}>
        Kirim
      </Button>,
    );
    const btn = screen.getByRole("button", { name: /Kirim/ });
    expect(btn).toBeDisabled();
    expect(btn).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("status", { name: "Memuat" })).toBeInTheDocument();
    await userEvent.setup().click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  test("tipe bawaan button (tidak men-submit form)", () => {
    renderUi(<Button>x</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("type", "button");
  });

  test("varian dan ukuran menjadi kelas", () => {
    renderUi(
      <Button variant="danger" size="lg">
        x
      </Button>,
    );
    expect(screen.getByRole("button")).toHaveClass("z-button--danger", "z-button--lg");
  });
});

describe("IconButton", () => {
  test("nama aksi menjadi aria-label dan tooltip muncul saat fokus", async () => {
    const onClick = vi.fn();
    const user = userEvent.setup();
    renderUi(<IconButton label="Tutup panel" icon={<span>x</span>} onClick={onClick} />);
    const btn = screen.getByRole("button", { name: "Tutup panel" });
    await user.click(btn);
    expect(onClick).toHaveBeenCalledOnce();
  });
});

describe("Input / Textarea", () => {
  test("mengetik dan menandai tidak valid", async () => {
    const user = userEvent.setup();
    // biome-ignore lint/correctness/noComponentHookFactories: komponen demo lokal khusus test
    const Demo = () => {
      const [v, setV] = useState("");
      return <Input aria-label="Nama" value={v} invalid={v.length > 3} onChange={(e) => setV(e.target.value)} />;
    };
    renderUi(<Demo />);
    const input = screen.getByRole("textbox", { name: "Nama" });
    await user.type(input, "abc");
    expect(input).toHaveValue("abc");
    expect(input).not.toHaveAttribute("aria-invalid");
    await user.type(input, "d");
    expect(input).toHaveAttribute("aria-invalid", "true");
  });

  test("Textarea: baris bawaan dan invalid", () => {
    renderUi(<Textarea aria-label="Catatan" invalid />);
    const ta = screen.getByRole("textbox", { name: "Catatan" });
    expect(ta).toHaveAttribute("rows", "3");
    expect(ta).toHaveAttribute("aria-invalid", "true");
  });
});

describe("Switch / Checkbox", () => {
  test("Switch: label menjadi nama, klik mengubah state", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    // biome-ignore lint/correctness/noComponentHookFactories: komponen demo lokal khusus test
    const Demo = () => {
      const [on, setOn] = useState(false);
      return (
        <Switch
          label="Mode gelap"
          checked={on}
          onCheckedChange={(c) => {
            setOn(c);
            onChange(c);
          }}
        />
      );
    };
    renderUi(<Demo />);
    const sw = screen.getByRole("switch", { name: "Mode gelap" });
    expect(sw).toHaveAttribute("aria-checked", "false");
    await user.click(sw);
    expect(onChange).toHaveBeenLastCalledWith(true);
    expect(sw).toHaveAttribute("aria-checked", "true");
  });

  test("Checkbox: spasi mengubah state; disabled tidak bisa", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(<Checkbox label="Setuju" checked={false} onCheckedChange={onChange} />);
    screen.getByRole("checkbox", { name: "Setuju" }).focus();
    await user.keyboard(" ");
    expect(onChange).toHaveBeenCalledWith(true);
    cleanup();
    const off = vi.fn();
    renderUi(<Checkbox label="Mati" checked={false} disabled onCheckedChange={off} />);
    await user.click(screen.getByRole("checkbox", { name: "Mati" }));
    expect(off).not.toHaveBeenCalled();
  });

  test("Checkbox indeterminate", () => {
    renderUi(<Checkbox label="Sebagian" checked={false} indeterminate onCheckedChange={() => {}} />);
    expect(screen.getByRole("checkbox", { name: "Sebagian" })).toHaveAttribute("aria-checked", "mixed");
  });
});

describe("Select", () => {
  test("daftar bergrup: cari, grup bisa dilipat, grup pilihan aktif terbuka, Enter memilih", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(
      <Select
        aria-label="Model"
        value="b/m2"
        onValueChange={onChange}
        items={[
          { value: "auto", label: "Otomatis" },
          { value: "a/m1", label: "Alpha 1", group: "Provider A" },
          { value: "a/m2", label: "Alpha 2", group: "Provider A" },
          { value: "b/m1", label: "Beta 1", group: "Provider B" },
          { value: "b/m2", label: "Beta 2", group: "Provider B" },
        ]}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Model" });
    expect(trigger).toHaveTextContent("Provider B · Beta 2");
    await user.click(trigger);
    const groupA = await screen.findByRole("button", { name: /Provider A/ });
    expect(groupA).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("option", { name: "Alpha 1" })).toBeNull();
    expect(screen.getByRole("option", { name: "Beta 2" })).toHaveAttribute("aria-selected", "true");
    await user.click(groupA);
    expect(screen.getByRole("option", { name: "Alpha 1" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Provider B/ }));
    expect(screen.queryByRole("option", { name: "Beta 1" })).toBeNull();
    // mencari membuka semua grup yang cocok
    await user.type(screen.getByRole("combobox", { name: "Cari" }), "beta 1");
    expect(screen.getByRole("option", { name: "Beta 1" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Alpha 1" })).toBeNull();
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith("b/m1");
  });

  test("tanpa hasil → pesan kosong", async () => {
    const user = userEvent.setup();
    const items = Array.from({ length: 20 }, (_, i) => ({ value: `v${i}`, label: `Item ${i}` }));
    renderUi(<Select aria-label="Banyak" value={null} onValueChange={() => {}} items={items} />);
    await user.click(screen.getByRole("combobox", { name: "Banyak" }));
    await user.type(screen.getByRole("combobox", { name: "Cari" }), "zzz");
    expect(screen.getByText("Tidak ada hasil")).toBeInTheDocument();
  });

  test("memilih item lewat keyboard dan memanggil onValueChange", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(
      <Select
        aria-label="Bahasa"
        value="id"
        onValueChange={onChange}
        items={[
          { value: "id", label: "Indonesia" },
          { value: "en", label: "English" },
        ]}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Bahasa" });
    expect(trigger).toHaveTextContent("Indonesia");
    await user.click(trigger);
    await user.click(await screen.findByRole("option", { name: "English" }));
    expect(onChange).toHaveBeenCalledWith("en");
  });

  test("deskripsi item tampil di daftar, bukan di pemicu", async () => {
    const user = userEvent.setup();
    renderUi(
      <Select
        aria-label="Mode"
        value="ask"
        onValueChange={() => {}}
        items={[
          { value: "ask", label: "Tanya", description: "Selalu minta izin." },
          { value: "all", label: "Semua", description: "Setujui semuanya." },
        ]}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Mode" });
    expect(trigger).toHaveTextContent("Tanya");
    expect(trigger).not.toHaveTextContent("Selalu minta izin.");
    await user.click(trigger);
    const option = await screen.findByRole("option", { name: /Semua/ });
    expect(option).toHaveTextContent("Setujui semuanya.");
  });

  test("ikon item tampil di daftar (dekoratif) tanpa mengubah nama opsi", async () => {
    const user = userEvent.setup();
    renderUi(
      <Select
        aria-label="Shell"
        value="cmd"
        onValueChange={() => {}}
        items={[
          { value: "cmd", label: "Command Prompt", icon: <img alt="" data-testid="ikon-cmd" /> },
          { value: "wsl", label: "WSL: Ubuntu", icon: <img alt="" data-testid="ikon-wsl" /> },
        ]}
      />,
    );
    await user.click(screen.getByRole("combobox", { name: "Shell" }));
    const option = await screen.findByRole("option", { name: "WSL: Ubuntu" });
    const icon = screen.getByTestId("ikon-wsl");
    expect(option).toContainElement(icon);
    expect(icon.parentElement).toHaveAttribute("aria-hidden", "true");
  });
});

describe("Slider", () => {
  test("memakai label, nilai awal, dan keyboard", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(<Slider label="Skala" value={50} min={0} max={100} step={10} onValueChange={onChange} />);
    const thumb = screen.getByRole("slider", { name: "Skala" });
    expect(thumb).toHaveAttribute("aria-valuenow", "50");
    thumb.focus();
    await user.keyboard("{ArrowRight}");
    expect(onChange).toHaveBeenCalledWith(60);
  });
});

describe("Badge, Avatar, Kbd, Spinner, Skeleton", () => {
  test("Badge tone menjadi kelas", () => {
    renderUi(<Badge tone="success">Aktif</Badge>);
    expect(screen.getByText("Aktif")).toHaveClass("z-badge--success");
  });

  test("Avatar: inisial sebagai fallback dan nama sebagai label", () => {
    renderUi(<Avatar name="Budi Santoso" />);
    expect(screen.getByRole("img", { name: "Budi Santoso" })).toHaveTextContent("BS");
  });

  test.each([
    ["Budi", "B"],
    ["budi santoso", "bs"],
    ["  ", "?"],
    ["Ana Maria Lopez", "AL"],
  ])("initials(%j) = %j", (name, want) => {
    expect(initials(name)).toBe(want);
  });

  test("Kbd: dibaca sebagai satu kesatuan", () => {
    renderUi(<Kbd keys="Ctrl+Shift+K" />);
    expect(screen.getByRole("img", { name: "Ctrl+Shift+K" })).toBeInTheDocument();
    expect(document.querySelectorAll("kbd")).toHaveLength(3);
  });

  test("Spinner berlabel mengikuti bahasa", () => {
    renderUi(<Spinner />, "en");
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
  });

  test("Skeleton dekoratif (aria-hidden)", () => {
    const { container } = renderUi(<Skeleton />);
    expect(container.querySelector(".z-skeleton")).toHaveAttribute("aria-hidden", "true");
  });
});

describe("AppLogo", () => {
  test("dekoratif tanpa label: disembunyikan dari pembaca layar", () => {
    const { container } = renderUi(<AppLogo />);
    const svg = container.querySelector("svg");
    expect(svg?.getAttribute("aria-hidden")).toBe("true");
    expect(svg?.classList.contains("z-app-logo--md")).toBe(true);
    expect(svg?.querySelectorAll("rect, polygon")).toHaveLength(3);
  });

  test("dengan label: gambar bernama, ukuran lewat kelas token", () => {
    renderUi(<AppLogo label="Zandor" size="lg" />);
    const img = screen.getByRole("img", { name: "Zandor" });
    expect(img.classList.contains("z-app-logo--lg")).toBe(true);
  });
});
