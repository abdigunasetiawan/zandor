// Satu langkah tool di proses agent, bergaya agent harness: ikon jenis · nama tool · target (berkas/perintah/kueri) ·
// ringkasan (baris yang dibaca, +/− perubahan, kode keluar, jumlah hasil) · status. Isi yang dibuka menampilkan diff
// bernomor baris untuk edit/tulis, baris yang dibaca, perintah + keluarannya, hasil pencarian per berkas, dan tugas sub-agen.
// Bentuk data: lihat tool-view.ts. Lipatan tahan remount lewat open-state (tidak menutup sendiri saat proses baru datang).
import type { ToolPart } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Badge,
  BotIcon,
  BrainIcon,
  ChatIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  DiffIcon,
  ExternalLinkIcon,
  FilePlusIcon,
  FileTextIcon,
  GlobeIcon,
  IconButton,
  ListIcon,
  SearchIcon,
  SparklesIcon,
  TerminalIcon,
  useToast,
  Working,
  WrenchIcon,
} from "@zandor/ui";
import type { ReactNode } from "react";
import { useDemo } from "./demo-context";
import { useOpenState } from "./open-state";
import { useDocActions } from "./services";
import {
  contentRows,
  type DiffRow,
  describeTool,
  grepGroups,
  type NumberedLine,
  parseDiff,
  readLines,
  splitPath,
  type ToolKind,
  type ToolView,
  taskResult,
} from "./tool-view";

const WEB_SEARCH = new Set(["websearch", "codesearch"]);
const str = (v: unknown): string => (typeof v === "string" ? v : "");

/** Batas baris yang dirender per blok (sisanya diringkas) supaya keluaran raksasa tidak membekukan UI. */
const MAX_ROWS = 300;

const KIND_ICON: Record<ToolKind, ReactNode> = {
  read: <FileTextIcon size="sm" />,
  edit: <DiffIcon size="sm" />,
  write: <FilePlusIcon size="sm" />,
  shell: <TerminalIcon size="sm" />,
  search: <SearchIcon size="sm" />,
  web: <GlobeIcon size="sm" />,
  task: <BotIcon size="sm" />,
  skill: <SparklesIcon size="sm" />,
  plan: <ListIcon size="sm" />,
  ask: <ChatIcon size="sm" />,
  memory: <BrainIcon size="sm" />,
  generic: <WrenchIcon size="sm" />,
};

/** Blok kode bernomor baris: diff (tambah/hapus/konteks/hunk) dan baris yang dibaca/dicari. */
export function DiffBlock({ rows, testId }: { rows: DiffRow[]; testId?: string }) {
  const { t } = useI18n();
  const shown = rows.slice(0, MAX_ROWS);
  return (
    <pre className="fold__pre diff diffb" data-testid={testId}>
      {shown.map((r, i) => {
        const no = r.kind === "del" ? r.oldNo : (r.newNo ?? r.oldNo);
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: baris diff statis, urutan tetap
          <span key={i} className={`diff__line diff__line--${r.kind} diffb__row`}>
            <span className="diffb__no" aria-hidden="true">
              {no ?? ""}
            </span>
            <span className="diffb__text">{r.text}</span>
          </span>
        );
      })}
      {rows.length > MAX_ROWS && (
        <span className="diff__line diff__line--hunk diffb__row">
          <span className="diffb__no" aria-hidden="true" />
          <span className="diffb__text">{t("tool.more", { n: rows.length - MAX_ROWS })}</span>
        </span>
      )}
    </pre>
  );
}

const asRows = (lines: NumberedLine[]): DiffRow[] =>
  lines.map((l) => ({ kind: "ctx" as const, text: l.text, ...(l.no !== undefined ? { newNo: l.no } : {}) }));

/** Teks keluaran apa adanya (monospasi), dipotong per baris. */
function OutputPre({ text }: { text: string }) {
  const { t } = useI18n();
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const clipped = lines.length > MAX_ROWS;
  return (
    <pre className="fold__pre tstep__pre">
      {(clipped ? lines.slice(0, MAX_ROWS) : lines).join("\n")}
      {clipped ? `\n${t("tool.more", { n: lines.length - MAX_ROWS })}` : ""}
    </pre>
  );
}

function FilePath({ path }: { path: ReturnType<typeof splitPath> }) {
  return (
    <span className="tstep__path" title={path.full}>
      {path.dir && <span className="tstep__dir">{path.dir}</span>}
      <span className="tstep__name">{path.name}</span>
    </span>
  );
}

function Changes({ adds, dels }: { adds?: number | undefined; dels?: number | undefined }) {
  if (adds === undefined && dels === undefined) return null;
  return (
    <span className="tstep__chg">
      {adds !== undefined && adds > 0 && <span className="tstep__add">+{adds}</span>}
      {dels !== undefined && dels > 0 && <span className="tstep__del">−{dels}</span>}
    </span>
  );
}

/** Jam selesai + durasi kerja satu langkah tool (kosong bila belum selesai atau waktu tak dilaporkan engine). */
function useFinished(state: ToolPart["state"]): { text: string; title: string } | null {
  const { t, locale } = useI18n();
  const end = state.time?.end;
  if (!end || !state.time?.start) return null;
  const ms = Math.max(0, end - state.time.start);
  const s = Math.round(ms / 1000);
  const dur =
    ms < 1000
      ? t("time.ms", { ms })
      : s >= 60
        ? t("time.minsec", { m: Math.floor(s / 60), s: s % 60 })
        : t("time.sec", { s });
  const at = new Date(end).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  return { text: t("tool.finished", { at, dur }), title: t("tool.finishedTitle", { at, dur }) };
}

function Status({ status }: { status: ToolPart["state"]["status"] }) {
  const { t } = useI18n();
  const label = t(`chat.tool.${status}` as never);
  if (status === "error") return <Badge tone="danger">{label}</Badge>;
  if (status === "completed")
    return (
      <span className="tstep__ok">
        <CheckIcon size="sm" />
        <span className="visually-hidden">{label}</span>
      </span>
    );
  return <Working variant="ripple" size="sm" label={label} />;
}

function Body({ part, view }: { part: ToolPart; view: ToolView }) {
  const { t } = useI18n();
  const { state } = part;
  const out = state.status === "error" ? "" : (state.output ?? "");
  const input = state.input;
  const error = state.error ?? "";

  let main: ReactNode = null;
  if (view.kind === "edit") {
    main = view.diff ? <DiffBlock rows={parseDiff(view.diff)} /> : <OutputPre text={JSON.stringify(input, null, 2)} />;
  } else if (view.kind === "write") {
    const content = str(input.content);
    main = content ? <DiffBlock rows={contentRows(content)} /> : null;
  } else if (view.kind === "read") {
    const { lines, note } = readLines(out);
    main =
      lines.length > 0 ? (
        <>
          <DiffBlock rows={asRows(lines)} />
          {note && <p className="tstep__note z-small">{note}</p>}
        </>
      ) : out ? (
        <OutputPre text={out} />
      ) : null;
  } else if (view.kind === "shell") {
    const output = str(state.metadata?.output) || out;
    main = (
      <>
        <p className="tstep__label z-small">{t("tool.command")}</p>
        <pre className="fold__pre tstep__cmd">
          <span className="tstep__prompt" aria-hidden="true">
            ${" "}
          </span>
          {view.command ?? view.target}
        </pre>
        {output.trim() ? (
          <OutputPre text={output.replace(/\n$/, "")} />
        ) : (
          <p className="tstep__note z-small">{t("tool.noOutput")}</p>
        )}
      </>
    );
  } else if (view.kind === "search") {
    const groups = grepGroups(out);
    if (part.tool === "grep" && groups.length > 0) {
      main = (
        <div className="tstep__groups">
          {groups.map((g) => (
            <div key={g.file} className="tstep__group">
              <FilePath path={splitPath(g.file)} />
              <DiffBlock rows={asRows(g.lines)} />
            </div>
          ))}
        </div>
      );
    } else if (out) {
      main = (
        <ul className="tstep__files">
          {out
            .split("\n")
            .filter((l) => l.trim())
            .slice(0, MAX_ROWS)
            .map((l) => (
              <li key={l}>
                <FilePath path={splitPath(l)} />
              </li>
            ))}
        </ul>
      );
    }
  } else if (view.kind === "task") {
    const result = taskResult(out);
    main = (
      <>
        {view.agent && <p className="tstep__label z-small">{t("tool.agent", { name: view.agent })}</p>}
        {str(input.prompt) && (
          <>
            <p className="tstep__label z-small">{t("tool.task")}</p>
            <OutputPre text={str(input.prompt)} />
          </>
        )}
        {result && (
          <>
            <p className="tstep__label z-small">{t("tool.result")}</p>
            <OutputPre text={result} />
          </>
        )}
        {view.childSession && <p className="tstep__note z-small">{t("tool.child", { id: view.childSession })}</p>}
      </>
    );
  } else if (view.kind === "plan" && view.todos) {
    main = (
      <ul className="tstep__todos">
        {view.todos.map((x, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: urutan daftar tugas adalah identitasnya
          <li key={i} data-status={x.status}>
            {x.content}
          </li>
        ))}
      </ul>
    );
  } else if (view.kind === "web") {
    main = (
      <>
        <p className="tstep__note z-small">{view.target}</p>
        {out && <OutputPre text={out} />}
      </>
    );
  } else {
    main = (
      <>
        <p className="tstep__label z-small">{t("chat.input")}</p>
        <OutputPre text={JSON.stringify(input, null, 2)} />
        {out && (
          <>
            <p className="tstep__label z-small">{t("chat.output")}</p>
            <OutputPre text={out} />
          </>
        )}
      </>
    );
  }
  return (
    <div className="fold__body tstep__body">
      {view.kind === "write" && state.metadata?.exists === false && (
        <p className="tstep__note z-small">{t("tool.newFile")}</p>
      )}
      {main}
      {error && (
        <p className="tstep__error z-small" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

export function ToolCard({ part }: { part: ToolPart }) {
  const { t } = useI18n();
  const [open, setOpen] = useOpenState(`tool:${part.id}`);
  const { state } = part;
  const view = describeTool(part);
  const finished = useFinished(state);
  const web = WEB_SEARCH.has(part.tool);
  const { open: openDoc, openDiff } = useDocActions();
  const toast = useToast();
  const demo = useDemo();
  // di sesi demo berkasnya fiktif: tombol Buka tidak ditampilkan
  const file =
    !demo && (view.kind === "read" || view.kind === "edit" || view.kind === "write") ? str(view.path?.full) : "";
  const openFile = () => {
    if (!openDoc(file)) toast.show({ title: t("doc.outside"), tone: "warning" });
  };
  // diff edit yang sudah selesai bisa ditinjau per bagian di Panel 3
  const reviewDiff = file && view.kind === "edit" && view.diff && state.status === "completed" ? view.diff : "";
  const openReview = () => {
    if (!openDiff(file, reviewDiff)) toast.show({ title: t("doc.outside"), tone: "warning" });
  };
  const range = view.range
    ? view.range.total !== undefined
      ? t("tool.linesOf", { from: view.range.from, to: view.range.to, total: view.range.total })
      : t("tool.lines", { from: view.range.from, to: view.range.to })
    : "";
  const failedExit = view.exit !== undefined && view.exit !== 0;

  return (
    <div className="fold fold--tool tstep" data-status={state.status} data-kind={view.kind}>
      <div className="tstep__row">
        <button
          type="button"
          className="fold__head tstep__head"
          aria-expanded={open}
          onClick={() => setOpen((o) => !o)}
        >
          {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
          <span className="tstep__icon" aria-hidden="true">
            {KIND_ICON[view.kind]}
          </span>
          <code className="fold__tool tstep__tool">{web ? t("chat.webSearch") : part.tool}</code>
          <span className="fold__summary tstep__target z-small" title={view.path?.full ?? view.target}>
            {view.path ? <FilePath path={view.path} /> : view.target}
          </span>
          <span className="tstep__meta z-small">
            {range && <span className="tstep__chip">{range}</span>}
            <Changes adds={view.adds} dels={view.dels} />
            {view.count !== undefined && view.kind === "search" && (
              <span className="tstep__chip">{t("tool.matches", { n: view.count })}</span>
            )}
            {view.count !== undefined && view.kind === "plan" && (
              <span className="tstep__chip">{t("tool.items", { n: view.count })}</span>
            )}
            {view.agent && <span className="tstep__chip">{view.agent}</span>}
            {failedExit && (
              <span className="tstep__chip tstep__chip--bad">{t("tool.exit", { code: view.exit ?? 0 })}</span>
            )}
            {finished && (
              <span className="tstep__time" title={finished.title}>
                {finished.text}
              </span>
            )}
          </span>
          <Status status={state.status} />
        </button>
        {reviewDiff && (
          <IconButton
            label={t("diff.open")}
            size="sm"
            className="tstep__open"
            icon={<DiffIcon />}
            onClick={openReview}
          />
        )}
        {file && (
          <IconButton
            label={t("doc.open", { name: file.split(/[\\/]/).at(-1) ?? file })}
            size="sm"
            className="tstep__open"
            icon={<ExternalLinkIcon />}
            onClick={openFile}
          />
        )}
      </div>
      {open && <Body part={part} view={view} />}
    </div>
  );
}
