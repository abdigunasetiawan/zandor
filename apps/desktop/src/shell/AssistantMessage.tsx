// Pesan asisten: saat BELUM disajikan hanya tampil "Bekerja selama …" + langkah terakhir (bisa dibuka untuk melihat semua
// langkah) + baris model (logo berkedip); setelah disajikan: ringkasan proses yang bisa dibuka, jawaban, kartu dokumen yang
// dibuat (sesi project), dan baris info + aksi. Proses = berpikir + tool call; jawaban = teks setelah langkah terakhir.
import type { ChatMessage, MessageInfo } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { ChevronDownIcon, ChevronRightIcon, ModelLogo } from "@zandor/ui";
import { useEffect, useRef, useState } from "react";
import { Part } from "./ChatPanel";
import { FileCard } from "./FileCard";
import { SAFE_IMAGE_URI } from "./image-uri";
import { MessageToolbar } from "./MessageToolbar";
import { messageDomId } from "./message-focus";
import { useOpenState } from "./open-state";
import { countSteps, createdDocuments, elapsedSeconds, splitElapsed, splitTurn, stepOf } from "./process-label";
import { useChat } from "./services";
import { useSessionFlags } from "./session-context";
import { tallyProcess } from "./tool-view";

/** Waktu sekarang yang diperbarui tiap detik hanya saat `active` (menghitung durasi "Bekerja selama"). */
function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [active]);
  return now;
}

function useFormatSeconds() {
  const { t } = useI18n();
  return (seconds: number) => {
    const { minutes, seconds: s } = splitElapsed(seconds);
    return minutes > 0 ? t("time.minsec", { m: minutes, s }) : t("time.sec", { s });
  };
}

/** Ringkasan hitungan proses ala agent harness: "12 dibaca · 3 diedit +45 −12 · 8 perintah" (hanya yang ada). */
function Tally({ process }: { process: ChatMessage["parts"] }) {
  const { t } = useI18n();
  const k = tallyProcess(process);
  const bits: string[] = [];
  if (k.read) bits.push(t("work.tally.read", { n: k.read }));
  if (k.edit) bits.push(t("work.tally.edit", { n: k.edit }));
  if (k.write) bits.push(t("work.tally.write", { n: k.write }));
  if (k.shell) bits.push(t("work.tally.shell", { n: k.shell }));
  if (k.search) bits.push(t("work.tally.search", { n: k.search }));
  if (k.web) bits.push(t("work.tally.web", { n: k.web }));
  if (k.task) bits.push(t("work.tally.task", { n: k.task }));
  if (bits.length === 0 && k.adds === 0 && k.dels === 0) return null;
  return (
    <span className="work__tally z-small">
      {bits.join(" · ")}
      {(k.adds > 0 || k.dels > 0) && (
        <span className="tstep__chg">
          {k.adds > 0 && <span className="tstep__add">+{k.adds}</span>}
          {k.dels > 0 && <span className="tstep__del">−{k.dels}</span>}
        </span>
      )}
    </span>
  );
}

function ProcessBlock({
  process,
  working,
  info,
  turnId,
}: {
  process: ChatMessage["parts"];
  working: boolean;
  info: MessageInfo;
  /** ID stabil giliran (pesan engine pertama): kunci state lipatan agar tidak menutup sendiri saat proses baru datang. */
  turnId: string;
}) {
  const { t } = useI18n();
  const fmt = useFormatSeconds();
  const [open, setOpen] = useOpenState(`proc:${turnId}`);
  const now = useNow(working);
  const seconds = elapsedSeconds((working ? now : (info.time.completed ?? now)) - info.time.created);
  const steps = countSteps(process);
  const last = [...process]
    .reverse()
    .map(stepOf)
    .find((s) => s !== null);
  const chevron = open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />;
  const list = open && (
    <div className="work__list">
      {process.map((p) => (
        <Part key={p.id} part={p} />
      ))}
    </div>
  );

  if (!working) {
    return (
      <div className="work work--done">
        <button type="button" className="work__sum" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          {chevron}
          <span>{t("work.done", { n: steps, time: fmt(seconds) })}</span>
          <Tally process={process} />
        </button>
        {list}
      </div>
    );
  }
  const label = last ? t(last.key, { tool: last.tool ?? "" }) : t("work.starting");
  return (
    <div className="work" data-working="">
      <p className="work__title" role="status">
        <span className="work__shimmer">{t("work.for", { time: fmt(seconds) })}</span>
      </p>
      {process.length > 0 && (
        <button type="button" className="work__step" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          {chevron}
          <span className="work__label">{label}</span>
          {last?.detail && <span className="work__detail z-small">{last.detail}</span>}
        </button>
      )}
      {list}
    </div>
  );
}

export function AssistantMessage({
  message,
  sessionId,
  isLastAssistant,
  streaming,
}: {
  message: ChatMessage;
  sessionId: string;
  isLastAssistant: boolean;
  streaming: boolean;
}) {
  const { t, locale } = useI18n();
  const fmt = useFormatSeconds();
  const { hasProject } = useSessionFlags();
  const { models, model } = useChat();
  const { info } = message;
  const turnId = (message as ChatMessage & { sourceIds?: string[] }).sourceIds?.[0] ?? info.id;
  const answerRef = useRef<HTMLDivElement>(null);
  const parts = message.parts.filter((p) => p.type !== "other" && p.type !== "step-finish");
  const { process, answer } = splitTurn(parts);
  // dokumen yang dibuat agent: kartu HANYA di sesi tanpa project (Chat); di sesi project berkasnya sudah ada di pohon berkas
  const docs = hasProject ? [] : createdDocuments(process);
  const done = info.time.completed !== undefined && !streaming;

  const providerID = info.providerID ?? model?.providerID;
  const modelID = info.modelID ?? model?.modelID;
  const providerName = models.find((m) => m.providerID === providerID)?.providerName ?? providerID ?? "";
  const when = info.time.completed
    ? new Date(info.time.completed).toLocaleString(locale === "id" ? "id-ID" : "en-US", {
        day: "numeric",
        month: "short",
        hour: "2-digit",
        minute: "2-digit",
      })
    : "";
  const duration = info.time.completed ? fmt(elapsedSeconds(info.time.completed - info.time.created)) : "";

  return (
    <article className="msg msg--assistant" aria-label={t("chat.assistant")} id={messageDomId(info.id)}>
      {(process.length > 0 || streaming) && (
        <ProcessBlock process={process} working={streaming} info={info} turnId={turnId} />
      )}
      <div className="msg__answer" ref={answerRef}>
        {answer.map((p) => {
          if (p.type !== "file") return <Part key={p.id} part={p} streaming={streaming} />;
          // gambar selalu tampil sebagai pratinjau; berkas lain hanya di sesi tanpa project
          const image = p.mime.startsWith("image/") && SAFE_IMAGE_URI.test(p.url);
          return image || !hasProject ? <FileCard key={p.id} filename={p.filename} mime={p.mime} url={p.url} /> : null;
        })}
        {docs.map((path) => (
          <FileCard key={path} path={path} />
        ))}
      </div>
      {info.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {info.error.name === "MessageAbortedError" ? t("chat.aborted") : info.error.message}
        </p>
      )}
      {modelID && (
        <footer className="msg__foot">
          <p className="msg__meta z-small" data-working={streaming ? "" : undefined}>
            <ModelLogo
              providerID={providerID}
              modelID={modelID}
              name={providerName}
              className={streaming ? "z-model-logo--working" : undefined}
            />
            {providerName && <span className="msg__provider">{providerName}</span>}
            <span aria-hidden="true">•</span>
            <code className="msg__model">{modelID}</code>
            {done && duration && (
              <>
                <span aria-hidden="true">•</span>
                <span>{duration}</span>
              </>
            )}
            {done && when && (
              <>
                <span aria-hidden="true">•</span>
                <span>{when}</span>
              </>
            )}
          </p>
          {done && (
            <MessageToolbar
              message={message}
              sessionId={sessionId}
              isLastAssistant={isLastAssistant}
              answerRef={answerRef}
            />
          )}
        </footer>
      )}
    </article>
  );
}
