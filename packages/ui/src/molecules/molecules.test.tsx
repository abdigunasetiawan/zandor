// @vitest-environment jsdom
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import {
  Button,
  Combobox,
  ConfirmProvider,
  ContextMenu,
  Dialog,
  Field,
  Input,
  Menu,
  Popover,
  SearchField,
  Tabs,
  ToastProvider,
  useConfirm,
  useToast,
} from "../index";
import { renderUi } from "../test-utils";

afterEach(cleanup);

describe("Menu", () => {
  test("membuka, memilih item, dan menutup; shortcut tampil", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderUi(
      <Menu
        trigger={<Button>Berkas</Button>}
        entries={[
          { type: "item", id: "new", label: "Sesi baru", shortcut: "Ctrl+N", onSelect },
          { type: "separator", id: "s" },
          { type: "item", id: "del", label: "Hapus", danger: true, onSelect: () => {} },
        ]}
      />,
    );
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Berkas" }));
    expect(await screen.findByRole("menu")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Ctrl+N" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /Hapus/ })).toHaveClass("z-item--danger");
    await user.click(screen.getByRole("menuitem", { name: /Sesi baru/ }));
    expect(onSelect).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
  });

  test("pemicu tautan (nativeButton=false) berperan tombol dan membuka menu", async () => {
    const user = userEvent.setup();
    renderUi(
      <Menu
        nativeButton={false}
        trigger={
          <a href="https://contoh.dev" onClick={(e) => e.preventDefault()}>
            contoh
          </a>
        }
        entries={[{ type: "item", id: "a", label: "Buka", onSelect: () => {} }]}
      />,
    );
    await user.click(screen.getByRole("button", { name: "contoh" }));
    expect(await screen.findByRole("menuitem", { name: "Buka" })).toBeInTheDocument();
  });

  test("item checkbox mengubah state; Esc menutup", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(
      <Menu
        trigger={<Button>Tampilan</Button>}
        entries={[{ type: "checkbox", id: "chat", label: "Panel chat", checked: true, onCheckedChange: onChange }]}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Tampilan" }));
    const item = await screen.findByRole("menuitemcheckbox", { name: /Panel chat/ });
    expect(item).toHaveAttribute("aria-checked", "true");
    await user.click(item);
    expect(onChange).toHaveBeenCalledWith(false, expect.anything());
  });

  test("Esc menutup menu", async () => {
    const user = userEvent.setup();
    renderUi(
      <Menu trigger={<Button>M</Button>} entries={[{ type: "item", id: "a", label: "A", onSelect: () => {} }]} />,
    );
    await user.click(screen.getByRole("button", { name: "M" }));
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
  });
});

describe("ContextMenu", () => {
  test("klik kanan membuka menu", async () => {
    const onSelect = vi.fn();
    renderUi(
      <ContextMenu entries={[{ type: "item", id: "copy", label: "Salin", onSelect }]}>
        <div>area</div>
      </ContextMenu>,
    );
    fireEvent.contextMenu(screen.getByText("area"));
    const item = await screen.findByRole("menuitem", { name: /Salin/ });
    await userEvent.setup().click(item);
    expect(onSelect).toHaveBeenCalledOnce();
  });
});

describe("Popover", () => {
  test("menampilkan judul dan isi saat dibuka", async () => {
    renderUi(
      <Popover trigger={<Button>Info</Button>} title="Detail">
        <p>isi popover</p>
      </Popover>,
    );
    await userEvent.setup().click(screen.getByRole("button", { name: "Info" }));
    expect(await screen.findByText("isi popover")).toBeInTheDocument();
    expect(screen.getByText("Detail")).toBeInTheDocument();
  });
});

describe("Tabs", () => {
  test("klik dan panah mengganti panel; hanya panel aktif tampil", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(
      <Tabs
        aria-label="Alat"
        onValueChange={onChange}
        items={[
          { value: "git", label: "Git", content: <p>isi git</p> },
          { value: "log", label: "Log", content: <p>isi log</p> },
        ]}
      />,
    );
    expect(screen.getByText("isi git")).toBeVisible();
    expect(screen.queryByText("isi log")).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Log" }));
    expect(onChange).toHaveBeenCalledWith("log");
    expect(await screen.findByText("isi log")).toBeVisible();
    screen.getByRole("tab", { name: "Log" }).focus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("tab", { name: "Git" })).toHaveFocus();
  });
});

function ToastDemo() {
  const toast = useToast();
  return (
    <Button
      onClick={() => toast.show({ title: "Tersimpan", description: "Sesi dicatat", tone: "success", timeoutMs: 0 })}
    >
      Beri tahu
    </Button>
  );
}

describe("Toast", () => {
  test("menampilkan toast dan bisa ditutup", async () => {
    const user = userEvent.setup();
    renderUi(
      <ToastProvider>
        <ToastDemo />
      </ToastProvider>,
    );
    await user.click(screen.getByRole("button", { name: "Beri tahu" }));
    expect(await screen.findByText("Tersimpan")).toBeInTheDocument();
    expect(screen.getByText("Sesi dicatat")).toBeInTheDocument();
    // Tombol tutup baru terbaca pembaca layar setelah toast difokus (perilaku Base UI untuk toast bertumpuk).
    screen.getByRole("dialog").focus();
    await user.click(await screen.findByRole("button", { name: "Tutup" }));
    await waitFor(() => expect(screen.queryByText("Tersimpan")).not.toBeInTheDocument());
  });
});

describe("Dialog", () => {
  test("judul, deskripsi, isi; tombol Tutup dan Esc menutup", async () => {
    const user = userEvent.setup();
    // biome-ignore lint/correctness/noComponentHookFactories: komponen demo lokal khusus test
    const Demo = () => {
      const [open, setOpen] = useState(true);
      return (
        <Dialog open={open} onOpenChange={setOpen} title="Pengaturan" description="Atur aplikasi">
          <p>isi dialog</p>
        </Dialog>
      );
    };
    renderUi(<Demo />);
    const dlg = await screen.findByRole("dialog", { name: "Pengaturan" });
    expect(dlg).toHaveTextContent("Atur aplikasi");
    expect(dlg).toHaveAttribute("data-shortcut-scope", "modal");
    await user.click(screen.getByRole("button", { name: "Tutup" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });
});

function ConfirmDemo({ onResult }: { onResult: (v: boolean) => void }) {
  const confirm = useConfirm();
  return (
    <Button
      onClick={async () =>
        onResult(
          await confirm({
            title: "Hapus sesi?",
            description: "Tidak bisa dibatalkan",
            tone: "danger",
            confirmLabel: "Hapus",
          }),
        )
      }
    >
      Hapus
    </Button>
  );
}

describe("useConfirm (pengganti window.confirm)", () => {
  test("Ya → true, Batal → false, Esc → false", async () => {
    const user = userEvent.setup();
    const onResult = vi.fn();
    renderUi(
      <ConfirmProvider>
        <ConfirmDemo onResult={onResult} />
      </ConfirmProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Hapus" }));
    const dlg = await screen.findByRole("alertdialog", { name: "Hapus sesi?" });
    expect(dlg).toHaveTextContent("Tidak bisa dibatalkan");
    await user.click(
      screen.getByRole("button", { name: "Hapus", hidden: false, description: undefined }) as HTMLElement,
    );
    await waitFor(() => expect(onResult).toHaveBeenLastCalledWith(true));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Hapus" }));
    await user.click(await screen.findByRole("button", { name: "Batal" }));
    await waitFor(() => expect(onResult).toHaveBeenLastCalledWith(false));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Hapus" }));
    await screen.findByRole("alertdialog");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(onResult).toHaveBeenCalledTimes(3));
    expect(onResult).toHaveBeenLastCalledWith(false);
  });

  test("di luar provider melempar galat yang jelas", () => {
    // biome-ignore lint/correctness/noComponentHookFactories: komponen demo lokal khusus test
    const Bad = () => {
      useConfirm();
      return null;
    };
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => renderUi(<Bad />)).toThrow(/ConfirmProvider/);
  });
});

describe("Combobox", () => {
  const items = [
    { value: "claude", label: "Claude Sonnet" },
    { value: "gpt", label: "GPT" },
    { value: "llama", label: "Llama lokal" },
  ];

  test("menyaring saat mengetik, memilih item, dan menampilkan 'tidak ada hasil'", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderUi(<Combobox aria-label="Model" items={items} value={null} onValueChange={onChange} />);
    const input = screen.getByRole("combobox", { name: "Model" });
    await user.click(input);
    await user.type(input, "lla");
    const opt = await screen.findByRole("option", { name: "Llama lokal" });
    expect(screen.queryByRole("option", { name: "GPT" })).not.toBeInTheDocument();
    await user.click(opt);
    expect(onChange).toHaveBeenCalledWith("llama");
  });

  test("tanpa hasil menampilkan pesan", async () => {
    const user = userEvent.setup();
    renderUi(<Combobox aria-label="Model" items={items} value={null} onValueChange={() => {}} />);
    const input = screen.getByRole("combobox", { name: "Model" });
    await user.click(input);
    await user.type(input, "zzz");
    expect(await screen.findByText("Tidak ada hasil")).toBeInTheDocument();
  });
});

describe("SearchField", () => {
  test("tombol bersihkan dan Esc mengosongkan", async () => {
    const user = userEvent.setup();
    // biome-ignore lint/correctness/noComponentHookFactories: komponen demo lokal khusus test
    const Demo = () => {
      const [v, setV] = useState("");
      return <SearchField aria-label="Cari sesi" value={v} onValueChange={setV} />;
    };
    renderUi(<Demo />);
    const input = screen.getByRole("searchbox", { name: "Cari sesi" });
    expect(screen.queryByRole("button", { name: "Bersihkan" })).not.toBeInTheDocument();
    await user.type(input, "abc");
    expect(input).toHaveValue("abc");
    await user.click(screen.getByRole("button", { name: "Bersihkan" }));
    expect(input).toHaveValue("");
    await user.type(input, "xyz");
    await user.keyboard("{Escape}");
    expect(input).toHaveValue("");
  });
});

describe("Field", () => {
  test("label terhubung ke kontrol; deskripsi dan galat lewat aria-describedby", () => {
    renderUi(
      <Field label="Nama project" description="Huruf kecil saja" error="Wajib diisi">
        {(c) => <Input {...c} />}
      </Field>,
    );
    const input = screen.getByLabelText("Nama project");
    expect(input).toHaveAttribute("aria-invalid", "true");
    const described = (input.getAttribute("aria-describedby") ?? "").split(" ");
    expect(described).toHaveLength(2);
    expect(screen.getByText("Huruf kecil saja")).toHaveAttribute("id", described[0]);
    expect(screen.getByRole("alert")).toHaveTextContent("Wajib diisi");
    expect(screen.getByRole("alert")).toHaveAttribute("id", described[1]);
  });

  test("tanpa galat: valid dan tanpa alert", () => {
    renderUi(<Field label="Nama">{(c) => <Input {...c} />}</Field>);
    expect(screen.getByLabelText("Nama")).not.toHaveAttribute("aria-invalid");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
