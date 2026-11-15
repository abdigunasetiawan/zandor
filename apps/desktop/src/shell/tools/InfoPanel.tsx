// Panel 4 — Info sesi: model, penggunaan token/konteks, biaya (bila dilaporkan engine), tool call, waktu.
import { memoryEnabled, projectKey, sessionUsage } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { InfoIcon, Select } from "@zandor/ui";
import { useEffect, useState } from "react";
import { useChat, useMemory, useServices } from "../services";
import { touchedFiles } from "../session-timeline";
import { ToolNotice } from "./ToolNotice";
import { useSelectedSession } from "./use-selected-session";

const nf = new Intl.NumberFormat();

function Row({ label, children, testId }: { label: string; children: React.ReactNode; testId?: string }) {
  return (
    <>
      <dt className="z-small">{label}</dt>
      <dd data-testid={testId}>{children}</dd>
    </>
  );
}

function duration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} dtk`;
  const m = Math.floor(s / 60);
  return `${m} mnt ${s % 60} dtk`;
}

/** Opt-in memori per sesi (PRD: opt-in saat membuat project/sesi, bisa diubah kemudian). */
function SessionMemory({ sessionId, projectPath }: { sessionId: string; projectPath: string | null }) {
  const { t } = useI18n();
  const { memory } = useServices();
  const state = useMemory();
  const [key, setKey] = useState<string | null>(null);
  useEffect(() => {
    void memory.loadSettings();
    let live = true;
    if (projectPath) void projectKey(projectPath).then((k) => live && setKey(k));
    else setKey(null);
    return () => {
      live = false;
    };
  }, [memory, projectPath]);
  const own = state.settings.sessions[sessionId];
  const value = typeof own === "boolean" ? (own ? "on" : "off") : "follow";
  const effective = memoryEnabled(state.settings, key, sessionId);
  return (
    <div className="infop__memory" data-testid="info-memory">
      <h4 className="z-label">{t("mem.session.title")}</h4>
      {key !== null ? (
        <p className="z-small">
          {t("mem.session.projectShared", { state: t(effective ? "mem.session.stateOn" : "mem.session.stateOff") })}
        </p>
      ) : (
        <>
          <Select
            aria-label={t("mem.session.title")}
            value={value}
            onValueChange={(v) => void memory.setSessionEnabled(sessionId, v === "follow" ? null : v === "on")}
            items={[
              { value: "follow", label: t("mem.session.follow") },
              { value: "on", label: t("mem.session.on") },
              { value: "off", label: t("mem.session.off") },
            ]}
          />
          <p className="z-small">
            {t("mem.session.effective", { state: t(effective ? "mem.session.stateOn" : "mem.session.stateOff") })}
          </p>
        </>
      )}
    </div>
  );
}

export function InfoPanel() {
  const { t } = useI18n();
  const chat = useChat();
  const sel = useSelectedSession();
  if (!sel) return <ToolNotice icon={<InfoIcon size="lg" />}>{t("info.empty")}</ToolNotice>;
  const session = sel.session;

  const model = chat.models.find((m) => m.providerID === chat.model?.providerID && m.modelID === chat.model?.modelID);
  const last = [...session.messages].reverse().find((m) => m.info.role === "assistant" && m.info.modelID);
  const usedModel =
    chat.models.find((m) => m.providerID === last?.info.providerID && m.modelID === last?.info.modelID) ?? model;
  const u = sessionUsage(session.messages, usedModel?.contextLimit ?? null);

  return (
    <div className="infop">
      <h3 className="z-label">{sel.demo ? t("sess.demoTitle") : (sel.title ?? t("sb.untitled"))}</h3>
      {sel.demo && <p className="z-small infop__note">{t("file.demoNote")}</p>}
      <dl className="infop__grid">
        <Row label={t("info.status")} testId="info-status">
          {t(
            session.sending || session.status.type === "busy"
              ? "info.status.busy"
              : session.status.type === "retry"
                ? "info.status.retry"
                : "info.status.idle",
          )}
          {session.permissions.length + session.questions.length > 0 &&
            ` · ${t("info.waiting", { n: session.permissions.length + session.questions.length })}`}
        </Row>
        {!sel.demo && <Row label={t("info.project")}>{sel.projectName ?? t("chat.noProject")}</Row>}
        {!sel.demo && (
          <Row label={t("info.dir")}>
            <code className="infop__path">{sel.directory}</code>
          </Row>
        )}
        <Row label={t("info.model")} testId="info-model">
          {u.model.modelID ? `${u.model.modelID} (${u.model.providerID})` : (usedModel?.name ?? "—")}
        </Row>
        <Row label={t("info.messages")}>
          {t("info.messagesValue", { user: u.messages.user, assistant: u.messages.assistant })}
        </Row>
        <Row label={t("info.context")} testId="info-context">
          {u.contextTokens === null
            ? "—"
            : u.contextLimit
              ? `${nf.format(u.contextTokens)} / ${nf.format(u.contextLimit)} (${(u.contextPercent ?? 0).toFixed(1)}%)`
              : `${nf.format(u.contextTokens)} (${t("info.contextUnknown")})`}
          {u.contextPercent !== null && (
            <progress className="infop__bar" max={100} value={u.contextPercent} aria-label={t("info.context")} />
          )}
        </Row>
        <Row label={t("info.tokens")} testId="info-tokens">
          {t("info.tokensValue", {
            input: nf.format(u.tokens.input),
            output: nf.format(u.tokens.output),
            reasoning: nf.format(u.tokens.reasoning),
          })}
        </Row>
        <Row label={t("info.cost")} testId="info-cost">
          {u.cost === null ? t("info.costNone") : `$${u.cost.toFixed(4)}`}
        </Row>
        <Row label={t("info.tools")} testId="info-tools">
          {u.toolCalls.total === 0
            ? "—"
            : `${u.toolCalls.total} (${t("info.toolsValue", { ok: u.toolCalls.completed, failed: u.toolCalls.failed, running: u.toolCalls.running })})`}
        </Row>
        {Object.keys(u.toolCalls.byTool).length > 0 && (
          <Row label={t("info.byTool")}>
            {Object.entries(u.toolCalls.byTool)
              .sort((a, b) => b[1] - a[1])
              .map(([k, n]) => `${k} ×${n}`)
              .join(", ")}
          </Row>
        )}
        <Row label={t("info.work")}>{duration(u.workMs)}</Row>
        <Row label={t("info.files")} testId="info-files">
          {touchedFiles(session.messages).length}
        </Row>
        {session.todos.length > 0 && (
          <Row label={t("info.plan")}>
            {t("todo.title", {
              done: session.todos.filter((x) => x.status === "completed").length,
              total: session.todos.length,
            })}
          </Row>
        )}
        <Row label={t("info.permissionMode")}>{t(`chat.mode.${chat.permissionMode}` as never)}</Row>
        {u.firstAt !== null && <Row label={t("info.first")}>{new Date(u.firstAt).toLocaleString()}</Row>}
        <Row label={t("info.session")}>
          <code>{sel.id}</code>
        </Row>
      </dl>
      {!sel.demo && <SessionMemory sessionId={sel.id} projectPath={sel.projectPath} />}
    </div>
  );
}
