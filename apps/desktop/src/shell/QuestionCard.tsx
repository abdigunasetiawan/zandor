// Form pertanyaan dari agent (tool `question` opencode) dan daftar tugas agent (`todowrite`). Jawaban dikirim ke engine apa
// adanya (label pilihan atau teks bebas); "Lewati" menolak permintaan sehingga agent melanjutkan tanpa jawaban.
import type { QuestionRequest, Todo } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, CheckIcon, Textarea, useToast } from "@zandor/ui";
import { type FormEvent, useId, useState } from "react";
import { useDemo } from "./demo-context";
import { useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function QuestionCard({ request, sessionId }: { request: QuestionRequest; sessionId: string }) {
  const { t } = useI18n();
  const { chat } = useServices();
  const demo = useDemo();
  const toast = useToast();
  const uid = useId();
  const [picks, setPicks] = useState<Record<number, string[]>>({});
  const [custom, setCustom] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState(false);
  const [step, setStep] = useState(0);

  const answers = request.questions.map((q, i) => {
    const typed = q.custom || q.options.length === 0 ? (custom[i] ?? "").trim() : "";
    const chosen = picks[i] ?? [];
    return q.multiple ? [...chosen, ...(typed ? [typed] : [])] : typed ? [typed] : chosen;
  });
  const complete = answers.every((a) => a.length > 0);
  const total = request.questions.length;
  const last = step >= total - 1;

  const choose = (i: number, label: string, multiple: boolean, on: boolean) => {
    setPicks((p) => {
      const cur = p[i] ?? [];
      return { ...p, [i]: multiple ? (on ? [...cur, label] : cur.filter((x) => x !== label)) : [label] };
    });
    if (!multiple) setCustom((c) => ({ ...c, [i]: "" })); // pilihan menggantikan teks bebas pada pertanyaan tunggal
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!complete || busy) return;
    if (demo) return demo.question(request.id, answers);
    setBusy(true);
    try {
      await chat.answerQuestion(sessionId, request.id, answers);
    } catch (err) {
      toast.show({ title: t("sb.error", { error: errText(err) }), tone: "danger" });
    } finally {
      setBusy(false);
    }
  };
  const skip = async () => {
    if (demo) return demo.question(request.id, null);
    setBusy(true);
    try {
      await chat.rejectQuestion(sessionId, request.id);
    } catch (err) {
      toast.show({ title: t("sb.error", { error: errText(err) }), tone: "danger" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="qcard" aria-label={t("q.title")} onSubmit={(e) => void submit(e)} data-testid="question-card">
      <h3 className="qcard__title">
        {t("q.title")}
        {total > 1 && <span className="z-small qcard__progress">{t("q.progress", { n: step + 1, total })}</span>}
      </h3>
      {request.questions.map((q, i) =>
        i !== step ? null : (
          // biome-ignore lint/suspicious/noArrayIndexKey: pertanyaan statis per permintaan
          <fieldset key={i} className="qcard__q">
            <legend className="qcard__legend">
              {q.header && <span className="z-small qcard__topic">{q.header}</span>}
              <span className="z-p qcard__text">{q.question}</span>
            </legend>
            {q.options.length > 0 && (
              <p className="z-small qcard__hint">{q.multiple ? t("q.multiHint") : t("q.singleHint")}</p>
            )}
            <ul className="qcard__opts">
              {q.options.map((o) => {
                const inputId = `${uid}-${i}-${o.label}`;
                const checked = (picks[i] ?? []).includes(o.label);
                return (
                  <li key={o.label}>
                    <label className="qcard__opt" data-checked={checked || undefined} htmlFor={inputId}>
                      <input
                        id={inputId}
                        className="qcard__input"
                        type={q.multiple ? "checkbox" : "radio"}
                        name={`${uid}-${i}`}
                        checked={checked}
                        onChange={(e) => choose(i, o.label, q.multiple, e.target.checked)}
                      />
                      <span className="qcard__mark" data-kind={q.multiple ? "check" : "radio"} aria-hidden="true">
                        {q.multiple && checked && <CheckIcon size="sm" />}
                      </span>
                      <span className="qcard__body">
                        <span className="z-label">{o.label}</span>
                        {o.description && <span className="z-small qcard__desc">{o.description}</span>}
                      </span>
                    </label>
                  </li>
                );
              })}
            </ul>
            {(q.custom || q.options.length === 0) && (
              <Textarea
                className="qcard__free"
                rows={q.options.length === 0 ? 3 : 1}
                aria-label={t("q.custom", { n: i + 1 })}
                placeholder={q.options.length === 0 ? t("q.freePlaceholder") : t("q.customPlaceholder")}
                value={custom[i] ?? ""}
                onChange={(e) => {
                  const v = e.target.value;
                  setCustom((c) => ({ ...c, [i]: v }));
                  if (!q.multiple && v.trim()) setPicks((p) => ({ ...p, [i]: [] }));
                }}
              />
            )}
          </fieldset>
        ),
      )}
      <div className="perm__actions">
        {step > 0 && (
          <Button size="sm" type="button" disabled={busy} onClick={() => setStep((s) => s - 1)}>
            {t("q.back")}
          </Button>
        )}
        {last ? (
          <Button size="sm" variant="solid" type="submit" disabled={!complete || busy}>
            {t("q.send")}
          </Button>
        ) : (
          <Button
            size="sm"
            variant="solid"
            type="button"
            disabled={(answers[step]?.length ?? 0) === 0 || busy}
            onClick={() => setStep((s) => s + 1)}
          >
            {t("q.next")}
          </Button>
        )}
        <Button size="sm" variant="ghost" type="button" disabled={busy} onClick={() => void skip()}>
          {t("q.skip")}
        </Button>
      </div>
    </form>
  );
}

const MARK: Record<Todo["status"], string> = { pending: "○", in_progress: "◐", completed: "●", cancelled: "✕" };

export function TodoList({ todos }: { todos: Todo[] }) {
  const { t } = useI18n();
  if (todos.length === 0) return null;
  const done = todos.filter((x) => x.status === "completed").length;
  return (
    <details className="todos" open data-testid="todo-list">
      <summary className="z-label">{t("todo.title", { done, total: todos.length })}</summary>
      <ol className="todos__list" aria-label={t("todo.title", { done, total: todos.length })}>
        {todos.map((x, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: urutan daftar tugas adalah identitasnya
          <li key={i} className="todos__item" data-status={x.status}>
            <span role="img" aria-label={t(`todo.status.${x.status}` as never)} className="todos__mark">
              {MARK[x.status]}
            </span>
            <span className="todos__text">{x.content}</span>
          </li>
        ))}
      </ol>
    </details>
  );
}
