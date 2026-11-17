// Demo setiap komponen untuk halaman design system. Data dummy hanya boleh ada di sini (roadmap #7).
// `demoCoverage` test memastikan kunci di sini == katalog komponen di @zandor/ui.
import type { AtomName, MoleculeName } from "@zandor/ui";
import {
  AppLogo,
  Avatar,
  Badge,
  Button,
  Checkbox,
  Combobox,
  ContextMenu,
  Dialog,
  Field,
  IconButton,
  Input,
  Kbd,
  Menu,
  type MenuEntry,
  ModelLogo,
  Palette,
  PlusIcon,
  Popover,
  SearchField,
  Select,
  Skeleton,
  Slider,
  Spinner,
  Switch,
  Tabs,
  Textarea,
  Tooltip,
  useConfirm,
  useToast,
  Working,
} from "@zandor/ui";
import { type ReactNode, useState } from "react";

const row = (children: ReactNode) => <div className="ds-row">{children}</div>;

const MODELS = [
  { value: "claude", label: "Claude Sonnet" },
  { value: "gpt", label: "GPT" },
  { value: "llama", label: "Llama lokal" },
  { value: "deepseek", label: "DeepSeek" },
];

const MENU_ENTRIES = (log: (s: string) => void): MenuEntry[] => [
  { type: "item", id: "new", label: "Sesi baru", shortcut: "Ctrl+N", onSelect: () => log("Sesi baru") },
  { type: "item", id: "open", label: "Buka project…", shortcut: "Ctrl+O", onSelect: () => log("Buka project") },
  { type: "separator", id: "s1" },
  { type: "item", id: "off", label: "Tidak tersedia", disabled: true, onSelect: () => {} },
  { type: "item", id: "del", label: "Hapus sesi", danger: true, onSelect: () => log("Hapus sesi") },
];

/** Daftar panjang/bergrup: Select otomatis memakai pencarian + grup yang bisa dilipat. */
const GROUPED_MODELS = ["OpenAI", "Anthropic", "Google", "OpenRouter"].flatMap((p) =>
  ["mini", "pro", "max"].map((m) => ({ value: `${p}/${m}`, label: `${p} ${m}`, group: p })),
);

function GroupedSelect() {
  const [v, setV] = useState<string | null>("Google/pro");
  return <Select aria-label="Model (bergrup)" value={v} onValueChange={setV} items={GROUPED_MODELS} />;
}

function ControlledSelect() {
  const [v, setV] = useState<string | null>("id");
  return (
    <Select
      aria-label="Bahasa"
      value={v}
      onValueChange={setV}
      items={[
        { value: "id", label: "Indonesia" },
        { value: "en", label: "English" },
        { value: "fr", label: "Français", disabled: true },
      ]}
    />
  );
}

function ChoiceDemo() {
  const [on, setOn] = useState(true);
  const [c, setC] = useState(false);
  return row(
    <>
      <Switch label="Ingat project terakhir" checked={on} onCheckedChange={setOn} />
      <Checkbox label="Setuju" checked={c} onCheckedChange={setC} />
      <Checkbox label="Sebagian" checked={false} indeterminate onCheckedChange={() => {}} />
      <Switch label="Nonaktif" checked={false} disabled onCheckedChange={() => {}} />
    </>,
  );
}

function SliderDemo() {
  const [v, setV] = useState(100);
  return (
    <div className="ds-stack ds-narrow">
      <Slider label="Skala UI" value={v} min={75} max={150} step={12.5} onValueChange={setV} />
      <span className="z-small">{v}%</span>
    </div>
  );
}

function ToastDemo() {
  const toast = useToast();
  return row(
    <>
      <Button
        onClick={() => toast.show({ title: "Tersimpan", description: "Sesi dicatat di engine.", tone: "success" })}
      >
        Sukses
      </Button>
      <Button
        onClick={() => toast.show({ title: "Gagal terhubung", description: "Engine tidak merespons.", tone: "danger" })}
      >
        Galat
      </Button>
      <Button onClick={() => toast.show({ title: "Info", tone: "neutral" })}>Netral</Button>
    </>,
  );
}

function DialogDemo() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Buka dialog</Button>
      <Dialog open={open} onOpenChange={setOpen} title="Pengaturan project" description="Atur nama dan folder project.">
        <Field label="Nama project">{(c) => <Input {...c} defaultValue="zandor" />}</Field>
      </Dialog>
    </>
  );
}

function ConfirmDemo() {
  const confirm = useConfirm();
  const [result, setResult] = useState<string>("—");
  return row(
    <>
      <Button
        variant="danger"
        onClick={async () => {
          const ok = await confirm({
            title: "Hapus sesi?",
            description: "Riwayat sesi ini akan hilang.",
            tone: "danger",
            confirmLabel: "Hapus",
          });
          setResult(ok ? "dikonfirmasi" : "dibatalkan");
        }}
      >
        Hapus sesi…
      </Button>
      <span className="z-small" role="status">
        Hasil: {result}
      </span>
    </>,
  );
}

function PaletteDemo() {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [last, setLast] = useState("—");
  const all = [
    { id: "a", label: "Buka Pengaturan", group: "Aplikasi", shortcut: "Ctrl+," },
    { id: "b", label: "Ganti tema gelap/terang", group: "Tampilan", shortcut: "Ctrl+Alt+T" },
    { id: "c", label: "Perbesar UI", group: "Tampilan", shortcut: "Ctrl+=" },
    { id: "d", label: "Tidak tersedia", group: "Aplikasi", disabled: true },
  ];
  const items = all.filter((i) => i.label.toLowerCase().includes(q.trim().toLowerCase()));
  return row(
    <>
      <Button
        onClick={() => {
          setQ("");
          setOpen(true);
        }}
      >
        Buka palette
      </Button>
      <span className="z-small" role="status">
        Dipilih: {last}
      </span>
      <Palette
        aria-label="Contoh palette"
        placeholder="Cari perintah…"
        open={open}
        onOpenChange={setOpen}
        query={q}
        onQueryChange={setQ}
        items={items}
        onSelect={(id) => setLast(all.find((i) => i.id === id)?.label ?? id)}
      />
    </>,
  );
}

function ComboboxDemo() {
  const [v, setV] = useState<string | null>(null);
  return (
    <div className="ds-narrow">
      <Combobox aria-label="Model" placeholder="Cari model…" items={MODELS} value={v} onValueChange={setV} />
    </div>
  );
}

function SearchDemo() {
  const [q, setQ] = useState("");
  return (
    <div className="ds-narrow">
      <SearchField aria-label="Cari sesi" value={q} onValueChange={setQ} />
    </div>
  );
}

function MenuDemo() {
  const [last, setLast] = useState("—");
  return row(
    <>
      <Menu trigger={<Button>Berkas</Button>} entries={MENU_ENTRIES(setLast)} />
      <span className="z-small" role="status">
        Dipilih: {last}
      </span>
    </>,
  );
}

function ContextMenuDemo() {
  const [last, setLast] = useState("—");
  return (
    <div className="ds-stack">
      <ContextMenu className="ds-contextarea" entries={MENU_ENTRIES(setLast)}>
        Klik kanan di area ini
      </ContextMenu>
      <span className="z-small" role="status">
        Dipilih: {last}
      </span>
    </div>
  );
}

export const ATOM_DEMOS: Record<AtomName, () => ReactNode> = {
  Button: () => (
    <div className="ds-stack">
      {row(
        <>
          <Button variant="solid">Solid</Button>
          <Button variant="outline">Outline</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="danger">Danger</Button>
          <Button disabled>Nonaktif</Button>
          <Button loading>Memuat</Button>
        </>,
      )}
      {row(
        <>
          <Button size="sm">Kecil</Button>
          <Button size="md">Sedang</Button>
          <Button size="lg">Besar</Button>
        </>,
      )}
    </div>
  ),
  IconButton: () =>
    row(
      <>
        <IconButton label="Tambah project" icon={<PlusIcon />} />
        <IconButton label="Tambah (outline)" variant="outline" icon={<PlusIcon />} />
        <IconButton label="Tambah (kecil)" size="sm" icon={<PlusIcon size="sm" />} />
      </>,
    ),
  Input: () => (
    <div className="ds-stack ds-narrow">
      <Input aria-label="Nama" placeholder="Nama project" />
      <Input aria-label="Tidak valid" defaultValue="salah" invalid />
      <Input aria-label="Nonaktif" placeholder="Nonaktif" disabled />
    </div>
  ),
  Textarea: () => (
    <div className="ds-narrow">
      <Textarea aria-label="Catatan" placeholder="Catatan sesi…" />
    </div>
  ),
  Select: () => (
    <div className="ds-narrow">
      <ControlledSelect />
      <GroupedSelect />
    </div>
  ),
  Switch: ChoiceDemo,
  Checkbox: () => <p className="z-small">Lihat contoh Checkbox di bagian Switch di atas.</p>,
  Slider: SliderDemo,
  Badge: () =>
    row(
      <>
        <Badge>Netral</Badge>
        <Badge tone="outline">Outline</Badge>
        <Badge tone="success">Aktif</Badge>
        <Badge tone="warning">Peringatan</Badge>
        <Badge tone="danger">Galat</Badge>
      </>,
    ),
  Avatar: () =>
    row(
      <>
        <Avatar name="Budi Santoso" size="sm" />
        <Avatar name="Budi Santoso" />
        <Avatar name="Ana Lopez" size="lg" />
      </>,
    ),
  ModelLogo: () =>
    row(
      <>
        <ModelLogo providerID="anthropic" modelID="claude-sonnet-4" />
        <ModelLogo providerID="antigravity" modelID="gemini-3" />
        <ModelLogo providerID="opencode" modelID="gpt-5" />
        <ModelLogo providerID="openrouter" modelID="deepseek-chat" />
        <ModelLogo providerID="zeta-lab" modelID="model-baru" name="Zeta Lab" />
      </>,
    ),
  AppLogo: () =>
    row(
      <>
        <AppLogo size="sm" />
        <AppLogo />
        <AppLogo size="lg" />
      </>,
    ),
  Kbd: () =>
    row(
      <>
        <Kbd keys="Ctrl+K" />
        <Kbd keys="Ctrl+Shift+P" />
        <Kbd keys="Esc" />
      </>,
    ),
  Tooltip: () =>
    row(
      <Tooltip content="Disimpan otomatis">
        <Button>Arahkan atau fokus ke sini</Button>
      </Tooltip>,
    ),
  Spinner: () =>
    row(
      <>
        <Spinner size="sm" />
        <Spinner />
        <Spinner size="lg" />
      </>,
    ),
  Working: () =>
    row(
      <>
        <Working variant="wave" />
        <Working variant="ripple" />
        <Working variant="orbit" />
        <Working variant="wave" size="sm" />
        <Working variant="ripple" size="dot" />
      </>,
    ),
  Skeleton: () => (
    <div className="ds-stack ds-narrow">
      <Skeleton />
      <Skeleton />
    </div>
  ),
};

export const MOLECULE_DEMOS: Record<MoleculeName, () => ReactNode> = {
  Menu: MenuDemo,
  ContextMenu: ContextMenuDemo,
  Popover: () => (
    <Popover trigger={<Button>Buka popover</Button>} title="Info sesi">
      <p className="z-p">Konten bebas: teks, form, atau tombol.</p>
    </Popover>
  ),
  Tabs: () => (
    <div className="ds-narrow-wide">
      <Tabs
        aria-label="Alat"
        items={[
          { value: "terminal", label: "Terminal", content: <p className="z-p">Isi terminal.</p> },
          { value: "git", label: "Git", content: <p className="z-p">Isi git.</p> },
          { value: "log", label: "Log", content: <p className="z-p">Isi log aktivitas.</p> },
        ]}
      />
    </div>
  ),
  Toast: ToastDemo,
  Dialog: DialogDemo,
  ConfirmDialog: ConfirmDemo,
  Combobox: ComboboxDemo,
  SearchField: SearchDemo,
  Palette: PaletteDemo,
  Field: () => (
    <div className="ds-stack ds-narrow">
      <Field label="Nama project" description="Huruf kecil, tanpa spasi.">
        {(c) => <Input {...c} placeholder="zandor" />}
      </Field>
      <Field label="Folder" error="Folder tidak ditemukan.">
        {(c) => <Input {...c} defaultValue={"C:\\tidak-ada"} />}
      </Field>
    </div>
  ),
};
