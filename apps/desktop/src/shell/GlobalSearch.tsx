// Pencarian global lintas sesi (Ctrl+Shift+F): judul + isi pesan di semua project dan Chat. Hasil bertambah bertahap
// selama pesan tiap sesi dimuat dari engine; klik hasil membuka sesinya dan menggulir ke pesan yang cocok.
import {
  CHAT_KEY,
  type SearchableSession,
  type SearchProgress,
  type SessionResult,
  SessionSearch,
  sessionTitle,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Dialog, SearchField, Spinner } from "@zandor/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { requestMessageFocus } from "./message-focus";
import { useProjects, useServices } from "./services";

function Snippet({ text, match }: { text: string; match: [number, number] }) {
  return (
    <span className="gsearch__snippet z-small">
      {text.slice(0, match[0])}
      <mark>{text.slice(match[0], match[1])}</mark>
      {text.slice(match[1])}
    </span>
  );
}

export function GlobalSearch({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t, locale } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  const search = useMemo(() => new SessionSearch(() => projects.engineApi()), [projects]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SessionResult[]>([]);
  const [progress, setProgress] = useState<SearchProgress | null>(null);
  const [running, setRunning] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Saat dibuka, muat daftar sesi project yang belum pernah dimuat (sidebar memuatnya hanya bila dibuka).
  useEffect(() => {
    if (!open) return;
    for (const p of state.projects) if (!state.sessions[p.id]) void projects.refresh(p.id);
    if (state.chatDir && !state.sessions[CHAT_KEY]) void projects.refresh(CHAT_KEY);
  }, [open, state.projects, state.sessions, state.chatDir, projects]);

  const sessions = useMemo<SearchableSession[]>(
    () =>
      Object.entries(state.sessions).flatMap(([key, g]) =>
        g.status === "ready" ? g.items.map((session) => ({ key, session })) : [],
      ),
    [state.sessions],
  );

  useEffect(() => {
    if (!open) return;
    const q = query.trim();
    if (!q) {
      search.cancel();
      setResults([]);
      setProgress(null);
      setRunning(false);
      return;
    }
    const timer = window.setTimeout(() => {
      setRunning(true);
      void search
        .search(sessions, q, (r, p) => {
          setResults(r);
          setProgress(p);
        })
        .then((done) => {
          if (!done) return; // dibatalkan oleh pencarian yang lebih baru
          setResults(done.results);
          setProgress(done.progress);
          setRunning(false);
        });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [open, query, sessions, search]);

  useEffect(() => {
    if (!open) search.cancel();
  }, [open, search]);

  const projectName = (key: string) =>
    key === CHAT_KEY ? t("chat.noProject") : (state.projects.find((p) => p.id === key)?.name ?? key);
  const date = (ms: number) => new Date(ms).toLocaleDateString(locale === "id" ? "id-ID" : "en-US");

  const go = (r: SessionResult, messageId?: string) => {
    projects.select(r.key, r.session.id);
    if (messageId) requestMessageFocus(messageId);
    onOpenChange(false);
  };

  let status = "";
  if (query.trim()) {
    if (running && progress) status = t("gsearch.progress", { done: progress.done, total: progress.total });
    else if (!running && results.length === 0 && progress) status = t("ui.noResults");
    else if (!running) status = t("gsearch.count", { n: results.length });
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o);
        if (!o) setQuery("");
      }}
      title={t("gsearch.title")}
      className="gsearch"
    >
      {/* biome-ignore lint/a11y/noStaticElementInteractions: Enter di kolom cari membuka hasil pertama */}
      <div
        className="gsearch__field"
        onKeyDown={(e) => {
          if (e.key === "Enter" && results[0] && e.target === inputRef.current) {
            e.preventDefault();
            go(results[0], results[0].hits[0]?.messageId);
          }
        }}
      >
        <SearchField
          value={query}
          onValueChange={setQuery}
          inputRef={inputRef}
          autoFocus
          placeholder={t("gsearch.placeholder")}
          aria-label={t("gsearch.placeholder")}
        />
      </div>
      <p className="z-small gsearch__status" role="status" aria-live="polite">
        {running && <Spinner size="sm" />} {status}
        {progress && progress.failed > 0 && ` · ${t("gsearch.failed", { n: progress.failed })}`}
      </p>
      {!query.trim() && <p className="z-small gsearch__hint">{t("gsearch.hint", { n: sessions.length })}</p>}
      <ul className="gsearch__list" aria-label={t("gsearch.results")}>
        {results.map((r) => (
          <li key={r.session.id} className="gsearch__item">
            <button type="button" className="gsearch__session" onClick={() => go(r, r.hits[0]?.messageId)}>
              <span className="gsearch__title">{sessionTitle(r.session) ?? t("sb.untitled")}</span>
              <span className="gsearch__meta z-small">
                {projectName(r.key)} · {date(r.session.time.updated)}
                {r.total > 0 && ` · ${t("gsearch.matches", { n: r.total })}`}
              </span>
            </button>
            {r.hits.length > 0 && (
              <ul className="gsearch__hits">
                {r.hits.map((h) => (
                  <li key={h.messageId}>
                    <button type="button" className="gsearch__hit" onClick={() => go(r, h.messageId)}>
                      <span className="gsearch__role z-small">
                        {h.role === "user" ? t("gsearch.you") : t("gsearch.assistant")}
                      </span>
                      <Snippet text={h.snippet} match={h.match} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
