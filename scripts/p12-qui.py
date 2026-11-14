import json

ROOT = "C:/Users/Abdi Guna Setiawan/Documents/Code/Zandor/"


def edit(p, pairs):
    s = open(ROOT + p, encoding="utf8").read()
    for a, b in pairs:
        assert a in s, (p, a[:80])
        s = s.replace(a, b, 1)
    open(ROOT + p, "w", encoding="utf8").write(s)


edit("apps/desktop/src/shell/ChatPanel.tsx", [
    ('import { ComposerInfo } from "./ComposerInfo";', 'import { ComposerInfo } from "./ComposerInfo";\nimport { QuestionCard, TodoList } from "./QuestionCard";'),
    ("[session.messages, total, session.permissions.length, busy]", "[session.messages, total, session.permissions.length, session.questions.length, busy]"),
    ("      {session.permissions.map((p) => (", "      {session.questions.map((q) => (\n        <QuestionCard key={q.id} request={q} sessionId={session.sessionID} />\n      ))}\n      {session.permissions.map((p) => ("),
])
s = open(ROOT + "apps/desktop/src/shell/ChatPanel.tsx", encoding="utf8").read()
i = s.index("<Composer session={")
print(s[max(0, i - 200): i + 80])

ID = {
    "q.title": "Pertanyaan dari agent", "q.multiHint": "Boleh pilih lebih dari satu.", "q.custom": "Jawaban sendiri untuk pertanyaan {n}", "q.customPlaceholder": "Atau ketik jawaban sendiri…",
    "q.send": "Kirim jawaban", "q.skip": "Lewati",
    "todo.title": "Rencana agent ({done}/{total} selesai)", "todo.status.pending": "belum", "todo.status.in_progress": "sedang dikerjakan", "todo.status.completed": "selesai", "todo.status.cancelled": "dibatalkan",
}
EN = {
    "q.title": "Question from the agent", "q.multiHint": "You can pick more than one.", "q.custom": "Your own answer for question {n}", "q.customPlaceholder": "Or type your own answer…",
    "q.send": "Send answers", "q.skip": "Skip",
    "todo.title": "Agent plan ({done}/{total} done)", "todo.status.pending": "pending", "todo.status.in_progress": "in progress", "todo.status.completed": "done", "todo.status.cancelled": "cancelled",
}


def patch(path, d, tail):
    s = open(ROOT + path, encoding="utf8").read()
    if '"q.title"' in s:
        return
    block = "".join(f"  {json.dumps(k)}: {json.dumps(v, ensure_ascii=False)},\n" for k, v in d.items())
    assert tail in s
    open(ROOT + path, "w", encoding="utf8").write(s.replace(tail, block + tail, 1))


patch("packages/i18n/src/locales/id.ts", ID, "} as const;")
patch("packages/i18n/src/locales/en.ts", EN, "};\n")

css = """
/* ---------- Form pertanyaan agent & daftar tugas (Fase 12) ---------- */
.qcard {
  display: flex;
  flex-direction: column;
  gap: var(--z-space-3);
  padding: var(--z-space-3);
  border: var(--z-border-width) solid var(--z-color-border-strong);
  border-radius: var(--z-radius-md);
  background: var(--z-color-raised);
}
.qcard__q {
  display: flex;
  flex-direction: column;
  gap: var(--z-space-2);
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}
.qcard__q legend {
  padding: 0;
}
.qcard__hint,
.qcard__desc {
  color: var(--z-color-fg-muted);
}
.qcard__opts {
  display: flex;
  flex-direction: column;
  gap: var(--z-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}
.qcard__opt {
  display: flex;
  align-items: flex-start;
  gap: var(--z-space-2);
  padding: var(--z-space-1) var(--z-space-2);
  border-radius: var(--z-radius-md);
  cursor: pointer;
}
.qcard__opt:hover {
  background: var(--z-color-hover);
}
.todos {
  flex: none;
  margin: 0 var(--z-space-3);
  padding: var(--z-space-1) var(--z-space-2);
  border: var(--z-border-width) solid var(--z-color-border);
  border-radius: var(--z-radius-md);
}
.todos summary {
  cursor: pointer;
}
.todos__list {
  display: flex;
  flex-direction: column;
  gap: var(--z-space-1);
  max-height: calc(var(--z-space-7) * 3);
  margin: var(--z-space-1) 0 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}
.todos__item {
  display: flex;
  gap: var(--z-space-2);
}
.todos__item[data-status="completed"] .todos__text,
.todos__item[data-status="cancelled"] .todos__text {
  color: var(--z-color-fg-muted);
  text-decoration: line-through;
}
.todos__item[data-status="in_progress"] .todos__mark {
  color: var(--z-color-warning);
}
"""
p = ROOT + "apps/desktop/src/styles.css"
cur = open(p, encoding="utf8").read()
open(p, "w", encoding="utf8").write(cur + css)
print("ok")
