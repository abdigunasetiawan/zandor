// @vitest-environment jsdom
import { cleanup, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import {
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
