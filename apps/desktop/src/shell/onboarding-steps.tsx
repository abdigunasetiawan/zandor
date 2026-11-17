// Langkah onboarding tambahan: Sambutan, perilaku memori & skill, impor memori dari AI lain, terima kasih.
// Impor: pengguna menyalin prompt ke AI yang biasa dipakai (ChatGPT, Gemini, Claude, DeepSeek, Grok, …), menempel
// balasannya (boleh dari beberapa AI). Fakta digabung dengan memori global yang ada; kunci sama dengan nilai berbeda
// (mis. nama "Adi" di Gemini vs "Abdi" di ChatGPT) ditampilkan sebagai konflik dan WAJIB dipilih sebelum disimpan.
import {
  buildMemory,
  conflictsOf,
  FILE_GLOBAL,
  type ImportSource,
  type KeyedFact,
  type ListPick,
  listsOf,
  mergeMemory,
  type Resolution,
  unresolved,
  valueId,
} from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import {
  AppLogo,
  ArrowRightIcon,
  Badge,
  Button,
  CheckIcon,
  CloseIcon,
  CopyIcon,
  IconButton,
  Input,
  PlusIcon,
  Select,
  Textarea,
  useToast,
} from "@zandor/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { useMemory, useServices, useSettings } from "./services";

const WELCOME_POINTS = ["profile", "provider", "system", "memory", "import"] as const;

export function WelcomeStep({ onNext, onSkip }: { onNext: () => void; onSkip: () => void }) {
  const { t } = useI18n();
  const { settings } = useServices();
  const { language: lang, theme } = useSettings();
  return (
    <main className="onb" aria-label={t("onb.title")}>
      <div className="onb__frame onb__welcome">
        <AppLogo size="lg" className="onb__logo" />
        <h1 className="z-h1 onb__title">{t("onb.title")}</h1>
        <p className="z-p onb__muted">{t("onb.welcome.body")}</p>
        <p className="z-p onb__muted">
          {t("onb.welcome.setup", {
            list: WELCOME_POINTS.map((s) => t(`onb.${s}.title` as MessageKey).toLowerCase()).join(", "),
          })}
        </p>
        <div className="onb__row">
          <Button variant="solid" onClick={onNext}>
            {t("onb.welcome.start")} <ArrowRightIcon size="sm" />
          </Button>
          <Button variant="ghost" onClick={onSkip}>
            {t("onb.skip")}
          </Button>
        </div>
        <Segmented
          name="onb-lang"
          legend={t("settings.language")}
          options={[
            { value: "id" as const, label: t("language.id"), flag: "flags/id.svg" },
            { value: "en" as const, label: t("language.en"), flag: "flags/gb.svg" },
          ]}
          value={lang}
          onChange={(v) => settings.set("language", v)}
        />
        <Segmented
          name="onb-theme"
          legend={t("theme.label")}
          options={(["system", "light", "dark"] as const).map((v) => ({ value: v, label: t(`theme.${v}`) }))}
          value={theme}
          onChange={(v) => settings.set("theme", v)}
        />
      </div>
    </main>
  );
}

/** Pilihan bersegmen ringkas (radio asli di dalam label, jadi keyboard dan pembaca layar tetap jalan). */
function Segmented<T extends string>({
  name,
  legend,
  options,
  value,
  onChange,
}: {
  name: string;
  legend: string;
  /** `flag`: gambar dekoratif di `public/` (bendera bahasa). */
  options: { value: T; label: string; flag?: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <fieldset className="onb__seg">
      <legend className="z-label">{legend}</legend>
      <div className="onb__segrow">
        {options.map((o) => (
          <label key={o.value} className="onb__segopt" data-checked={value === o.value ? "" : undefined}>
            <input type="radio" name={name} checked={value === o.value} onChange={() => onChange(o.value)} />
            {o.flag && <img className="onb__flag" src={o.flag} alt="" />}
            {o.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

type Mode = "auto" | "manual" | "off";
const MODES: Mode[] = ["auto", "manual", "off"];

/** Satu kartu: penjelasan singkat + pilihan mode; kelebihan/kekurangan hanya untuk mode terpilih. */
function ModeChoice({ kind, value, onChange }: { kind: "mem" | "skill"; value: Mode; onChange: (m: Mode) => void }) {
  const { t } = useI18n();
  return (
    <div className="onb__mode">
      <Segmented
        name={`onb-${kind}`}
        legend={t(`onb.${kind}.legend` as MessageKey)}
        options={MODES.map((m) => ({ value: m, label: t(`onb.mode.${m}` as MessageKey) }))}
        value={value}
        onChange={onChange}
      />
      <p className="z-small onb__muted">{t(`onb.${kind}.what` as MessageKey)}</p>
      <p className="z-small onb__pro">+ {t(`onb.${kind}.${value}.pro` as MessageKey)}</p>
      <p className="z-small onb__con">− {t(`onb.${kind}.${value}.con` as MessageKey)}</p>
    </div>
  );
}

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function MemoryStep() {
  const { t } = useI18n();
  const toast = useToast();
  const { memory } = useServices();
  const state = useMemory();
  // "Tentukan sendiri" dan "Mati" sama-sama menyimpan bawaan mati; bedanya anjuran: nyalakan per project/sesi.
  const [mem, setMem] = useState<Mode | null>(null);
  const [skill, setSkill] = useState<Mode | null>(null);
  useEffect(() => {
    if (!state.ready) void memory.refresh();
  }, [memory, state.ready]);
  const fail = (e: unknown) => toast.show({ title: t("profile.error", { error: errText(e) }), tone: "danger" });
  return (
    <div className="onb__stack">
      <ModeChoice
        kind="mem"
        value={mem ?? (state.settings.defaultEnabled ? "auto" : "manual")}
        onChange={(m) => {
          setMem(m);
          void memory.setDefaultEnabled(m === "auto").catch(fail);
        }}
      />
      <ModeChoice
        kind="skill"
        value={skill ?? (state.settings.skillCreator ? "auto" : "manual")}
        onChange={(m) => {
          setSkill(m);
          void memory.setSkillCreator(m === "auto").catch(fail);
        }}
      />
      <p className="z-small onb__muted">{t("onb.memory.later")}</p>
    </div>
  );
}

const AI_SOURCES = ["chatgpt", "gemini", "claude", "deepseek", "grok", "copilot", "other"] as const;
type AiSource = (typeof AI_SOURCES)[number];
/** Nama produk (bukan teks UI yang diterjemahkan); "other" memakai teks i18n. */
const AI_LABEL: Record<AiSource, string> = {
  chatgpt: "ChatGPT",
  gemini: "Gemini",
  claude: "Claude",
  deepseek: "DeepSeek",
  grok: "Grok",
  copilot: "Copilot",
  other: "",
};

/** Penjaga saat meninggalkan langkah: true = boleh pindah. */
export type LeaveGuard = () => Promise<boolean>;

export function ImportStep({ onLeave }: { onLeave: (guard: LeaveGuard | null) => void }) {
  const { t } = useI18n();
  const toast = useToast();
  const { memory } = useServices();
  const state = useMemory();
  const [ai, setAi] = useState<AiSource>("chatgpt");
  const [text, setText] = useState("");
  const [sources, setSources] = useState<ImportSource[]>([]);
  const [existing, setExisting] = useState("");
  const [res, setRes] = useState<Record<string, Resolution>>({});
  /** Teks "Tulis sendiri" per kunci; dipertahankan saat pengguna berpindah pilihan lalu kembali. */
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [picks, setPicks] = useState<Record<string, ListPick>>({});
  const [saved, setSaved] = useState(false);
  const aiName = (s: AiSource) => AI_LABEL[s] || t("onb.import.other");
  const sourceName = (id: string) =>
    id === "zandor" ? t("onb.import.existing") : (sources.find((s) => s.id === id)?.label ?? id);

  useEffect(() => {
    memory.readFile(FILE_GLOBAL).then(setExisting, () => setExisting(""));
  }, [memory]);

  const merged = useMemo(
    () => mergeMemory([...(existing.trim() ? [{ id: "zandor", label: "Zandor", text: existing }] : []), ...sources]),
    [existing, sources],
  );
  const conflicts = conflictsOf(merged);
  const lists = listsOf(merged);
  const open = unresolved(merged, res);
  const preview = buildMemory(merged, res, picks);

  const copyPrompt = () =>
    navigator.clipboard.writeText(t("onb.import.prompt")).then(
      () => toast.show({ title: t("onb.import.copied"), timeoutMs: 2500 }),
      () => toast.show({ title: t("onb.sys.copyFailed"), tone: "danger" }),
    );
  const add = () => {
    const base = aiName(ai);
    const n = sources.filter((s) => s.label === base || s.label.startsWith(`${base} `)).length;
    setSources([...sources, { id: `${ai}-${sources.length}`, label: n ? `${base} ${n + 1}` : base, text }]);
    setText("");
    setSaved(false);
  };
  const remove = (id: string) => setSources(sources.filter((s) => s.id !== id));
  const save = async (): Promise<boolean> => {
    try {
      await memory.saveFile(FILE_GLOBAL, preview);
      setExisting(preview);
      setSources([]);
      setRes({});
      setPicks({});
      setSaved(true);
      toast.show({ title: t("onb.import.saved"), tone: "success" });
      return true;
    } catch (e) {
      toast.show({ title: t("profile.error", { error: errText(e) }), tone: "danger" });
      return false;
    }
  };

  // Pindah langkah dengan impor yang belum disimpan: simpan otomatis (dulu hasilnya hilang diam-diam);
  // bila masih ada konflik, tahan di langkah ini dan beri tahu.
  const latest = useRef({ pending: false, open: 0, save, preview });
  latest.current = { pending: sources.length > 0, open: open.length, save, preview };
  useEffect(() => {
    onLeave(async () => {
      const l = latest.current;
      if (!l.pending || !l.preview) return true;
      if (l.open > 0) {
        toast.show({ title: t("onb.import.unresolved", { n: l.open }), tone: "danger" });
        return false;
      }
      return l.save();
    });
    return () => onLeave(null);
  }, [onLeave, toast, t]);

  return (
    <div className="onb__stack">
      <section className="onb__stack" aria-labelledby="onb-imp-1">
        <h2 id="onb-imp-1" className="z-h4">
          {t("onb.import.step1")}
        </h2>
        <pre className="onb__prompt">{t("onb.import.prompt")}</pre>
        <div className="onb__row">
          <Button onClick={() => void copyPrompt()}>
            <CopyIcon size="sm" /> {t("onb.import.copy")}
          </Button>
        </div>
      </section>

      <section className="onb__stack" aria-labelledby="onb-imp-2">
        <h2 id="onb-imp-2" className="z-h4">
          {t("onb.import.step2")}
        </h2>
        <Select
          aria-label={t("onb.import.source")}
          value={ai}
          onValueChange={(v) => setAi(v as AiSource)}
          items={AI_SOURCES.map((s) => ({ value: s, label: aiName(s) }))}
        />
        <Textarea
          aria-label={t("onb.import.paste")}
          placeholder={t("onb.import.paste")}
          rows={6}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <div className="onb__row">
          <Button onClick={add} disabled={!text.trim()}>
            {t("onb.import.add")}
          </Button>
        </div>
        {sources.length > 0 && (
          <ul className="onb__row onb__sources" aria-label={t("onb.import.sources")}>
            {sources.map((s) => (
              <li key={s.id}>
                <Badge>{s.label}</Badge>
                <Button size="sm" variant="ghost" onClick={() => remove(s.id)}>
                  {t("onb.import.remove", { name: s.label })}
                </Button>
              </li>
            ))}
          </ul>
        )}
        {merged.secrets > 0 && (
          <p className="z-small onb__error" role="alert">
            {t("onb.import.secrets", { n: merged.secrets })}
          </p>
        )}
      </section>

      {conflicts.length > 0 && (
        <section className="onb__stack" aria-labelledby="onb-imp-3">
          <h2 id="onb-imp-3" className="z-h4">
            {t("onb.import.conflicts", { n: conflicts.length })}
          </h2>
          <p className="z-small onb__muted">{t("onb.import.conflictsHint")}</p>
          {conflicts.map((c) => (
            <fieldset key={c.key} className="onb__choice">
              <legend className="z-label">{c.label}</legend>
              {c.options.map((o, k) => (
                <label key={o.value} className="onb__option" data-checked={res[c.key] === k ? "" : undefined}>
                  <input
                    type="radio"
                    name={`onb-c-${c.key}`}
                    checked={res[c.key] === k}
                    onChange={() => setRes({ ...res, [c.key]: k })}
                  />
                  <span className="onb__optiontext">
                    <span className="z-label">{o.value}</span>
                    <span className="z-small onb__muted">
                      {t("onb.import.from", { list: o.sources.map(sourceName).join(", ") })}
                    </span>
                  </span>
                </label>
              ))}
              <label className="onb__option" data-checked={typeof res[c.key] === "object" ? "" : undefined}>
                <input
                  type="radio"
                  name={`onb-c-${c.key}`}
                  checked={typeof res[c.key] === "object"}
                  onChange={() => setRes({ ...res, [c.key]: { custom: drafts[c.key] ?? "" } })}
                />
                <span className="onb__optiontext">
                  <span className="z-label">{t("onb.import.custom")}</span>
                  <span className="z-small onb__muted">{t("onb.import.customHint")}</span>
                </span>
              </label>
              {typeof res[c.key] === "object" && (
                <Input
                  // biome-ignore lint/a11y/noAutofocus: muncul tepat setelah pengguna memilih "Tulis sendiri"
                  autoFocus
                  aria-label={t("onb.import.customLabel", { key: c.label })}
                  placeholder={t("onb.import.customPlaceholder")}
                  maxLength={300}
                  value={drafts[c.key] ?? ""}
                  onChange={(e) => {
                    setDrafts({ ...drafts, [c.key]: e.target.value });
                    setRes({ ...res, [c.key]: { custom: e.target.value } });
                  }}
                />
              )}
            </fieldset>
          ))}
        </section>
      )}

      {lists.length > 0 && (
        <section className="onb__stack" aria-labelledby="onb-imp-lists">
          <h2 id="onb-imp-lists" className="z-h4">
            {t("onb.import.lists")}
          </h2>
          <p className="z-small onb__muted">{t("onb.import.listsHint")}</p>
          {lists.map((l) => (
            <ListPicker
              key={l.key}
              fact={l}
              pick={picks[l.key] ?? EMPTY_PICK}
              sourceName={sourceName}
              onChange={(p) => setPicks({ ...picks, [l.key]: p })}
            />
          ))}
        </section>
      )}

      {sources.length > 0 && (
        <section className="onb__stack" aria-labelledby="onb-imp-4">
          <h2 id="onb-imp-4" className="z-h4">
            {t("onb.import.preview")}
          </h2>
          <pre className="onb__prompt">{preview || t("onb.import.empty")}</pre>
          {open.length > 0 && (
            <p className="z-small onb__muted" role="status">
              {t("onb.import.unresolved", { n: open.length })}
            </p>
          )}
          <div className="onb__row">
            <Button variant="solid" onClick={() => void save()} disabled={open.length > 0 || !preview}>
              {t("onb.import.save")}
            </Button>
          </div>
        </section>
      )}
      {saved && (
        <p className="z-p onb__status" role="status" data-tone="ok">
          <CheckIcon size="sm" /> {t("onb.import.saved")}
        </p>
      )}
      {saved && !state.settings.defaultEnabled && <p className="z-small onb__muted">{t("onb.import.memOff")}</p>}
      {!memory.connected && <p className="z-small onb__muted">{t("onb.import.webOnly")}</p>}
      <p className="z-small onb__muted">{t("onb.import.optional")}</p>
    </div>
  );
}

const EMPTY_PICK: ListPick = { excluded: [], extra: [] };

/**
 * Kunci bernilai banyak (proyek, alat, minat…): semua nilai dari AI tercentang secara bawaan (lebih baik kelebihan
 * daripada hilang); pengguna membuang yang salah dan menambah nilai sendiri sebanyak perlu.
 */
function ListPicker({
  fact,
  pick,
  sourceName,
  onChange,
}: {
  fact: KeyedFact;
  pick: ListPick;
  sourceName: (id: string) => string;
  onChange: (p: ListPick) => void;
}) {
  const { t } = useI18n();
  const setExtra = (i: number, v: string) => onChange({ ...pick, extra: pick.extra.map((x, k) => (k === i ? v : x)) });
  return (
    <fieldset className="onb__choice">
      <legend className="z-label">{fact.label}</legend>
      {fact.options.map((o) => {
        const id = valueId(o.value);
        const on = !pick.excluded.includes(id);
        return (
          <label key={id} className="onb__option" data-checked={on ? "" : undefined}>
            <input
              type="checkbox"
              checked={on}
              onChange={() =>
                onChange({
                  ...pick,
                  excluded: on ? [...pick.excluded, id] : pick.excluded.filter((x) => x !== id),
                })
              }
            />
            <span className="onb__optiontext">
              <span className="z-label">{o.value}</span>
              <span className="z-small onb__muted">
                {t("onb.import.from", { list: o.sources.map(sourceName).join(", ") })}
              </span>
            </span>
          </label>
        );
      })}
      {pick.extra.map((v, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: baris isian tanpa id; urutan hanya berubah lewat hapus baris ini
        <div key={i} className="onb__extra">
          <Input
            // biome-ignore lint/a11y/noAutofocus: baris baru langsung diisi setelah menekan "Tambah nilai"
            autoFocus={i === pick.extra.length - 1 && !v}
            aria-label={t("onb.import.customLabel", { key: fact.label })}
            placeholder={t("onb.import.customPlaceholder")}
            maxLength={300}
            value={v}
            onChange={(e) => setExtra(i, e.target.value)}
          />
          <IconButton
            label={t("onb.import.removeValue")}
            icon={<CloseIcon size="sm" />}
            onClick={() => onChange({ ...pick, extra: pick.extra.filter((_, k) => k !== i) })}
          />
        </div>
      ))}
      <div className="onb__row">
        <Button size="sm" variant="ghost" onClick={() => onChange({ ...pick, extra: [...pick.extra, ""] })}>
          <PlusIcon size="sm" /> {t("onb.import.addValue")}
        </Button>
      </div>
    </fieldset>
  );
}

export function ThanksStep() {
  const { t } = useI18n();
  return (
    <div className="onb__stack onb__thanks">
      <AppLogo size="lg" className="onb__logo" />
      <p className="z-p">{t("onb.thanks.text")}</p>
      <p className="z-small onb__muted">{t("onb.thanks.tip")}</p>
    </div>
  );
}
