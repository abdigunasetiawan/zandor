// @vitest-environment jsdom
import { cleanup, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { Palette, type PaletteItem } from "../index";
import { renderUi } from "../test-utils";

afterEach(cleanup);

const ITEMS: PaletteItem[] = [
  { id: "a", label: "Buka Pengaturan", group: "app", shortcut: "Ctrl+," },
  { id: "b", label: "Ganti tema", group: "tampilan", disabled: true },
  { id: "c", label: "Perbesar UI", group: "tampilan", matchIndices: [0, 1] },
];

function Harness({ items = ITEMS, onSelect }: { items?: PaletteItem[]; onSelect: (id: string) => void }) {
  const [open, setOpen] = useState(true);
  const [q, setQ] = useState("");
  return (
    <Palette
      aria-label="Command palette"
      open={open}
      onOpenChange={setOpen}
      query={q}
      onQueryChange={setQ}
      items={items}
      onSelect={onSelect}
      placeholder="Cari perintah…"
    />
  );
}

describe("Palette", () => {
  test("combobox + listbox; item pertama yang aktif menjadi aria-activedescendant", async () => {
    renderUi(<Harness onSelect={() => {}} />);
    const input = await screen.findByRole("combobox", { name: "Command palette" });
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(3);
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    expect(input).toHaveAttribute("aria-activedescendant", options[0]?.id);
    expect(options[1]).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("img", { name: "Ctrl+," })).toBeInTheDocument();
  });

  test("panah bawah melewati item nonaktif; panah atas memutar; Home/End", async () => {
    const user = userEvent.setup();
    renderUi(<Harness onSelect={() => {}} />);
    const input = await screen.findByRole("combobox");
    const opts = () => screen.getAllByRole("option");
    await user.keyboard("{ArrowDown}");
    expect(opts()[2]).toHaveAttribute("aria-selected", "true"); // b dilewati
    await user.keyboard("{ArrowDown}");
    expect(opts()[0]).toHaveAttribute("aria-selected", "true"); // memutar
    await user.keyboard("{ArrowUp}");
    expect(opts()[2]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Home}");
    expect(opts()[0]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{End}");
    expect(opts()[2]).toHaveAttribute("aria-selected", "true");
    expect(input).toHaveFocus(); // fokus tetap di input
  });

  test("Enter memilih item aktif lalu menutup", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderUi(<Harness onSelect={onSelect} />);
    await screen.findByRole("combobox");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onSelect).toHaveBeenCalledWith("c");
    await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
  });

  test("klik memilih; item nonaktif tidak bisa dipilih", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderUi(<Harness onSelect={onSelect} />);
    await screen.findByRole("combobox");
    await user.click(screen.getByRole("option", { name: /Ganti tema/ }));
    expect(onSelect).not.toHaveBeenCalled();
    await user.click(screen.getByRole("option", { name: /Buka Pengaturan/ }));
    expect(onSelect).toHaveBeenCalledWith("a");
  });

  test("Esc menutup tanpa memilih", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderUi(<Harness onSelect={onSelect} />);
    await screen.findByRole("combobox");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
    expect(onSelect).not.toHaveBeenCalled();
  });

  test("mengetik memanggil onQueryChange; tanpa hasil menampilkan pesan", async () => {
    const user = userEvent.setup();
    renderUi(<Harness items={[]} onSelect={() => {}} />);
    const input = await screen.findByRole("combobox");
    await user.type(input, "zzz");
    expect(input).toHaveValue("zzz");
    expect(screen.getByRole("status")).toHaveTextContent("Tidak ada hasil");
    expect(input).not.toHaveAttribute("aria-activedescendant");
  });

  test("karakter yang cocok disorot", async () => {
    renderUi(<Harness onSelect={() => {}} />);
    await screen.findByRole("combobox");
    const marks = document.querySelectorAll("mark");
    expect([...marks].map((m) => m.textContent)).toEqual(["P", "e"]);
  });
});
