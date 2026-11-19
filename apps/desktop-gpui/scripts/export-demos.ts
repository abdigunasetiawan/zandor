// Ekspor sesi demo React (apps/desktop/src/shell/demo-sessions.ts) ke JSON untuk GPUI — satu sumber kebenaran.
// Jalankan dari akar repo: bun apps/desktop-gpui/scripts/export-demos.ts
import { writeFileSync } from "node:fs";
import { DEMO_SESSIONS } from "../../desktop/src/shell/demo-sessions";

const out = DEMO_SESSIONS.map((d) => {
  const s = d.build();
  return {
    id: d.id,
    titleKey: d.titleKey,
    subtitleKey: d.subtitleKey,
    sessionID: s.sessionID,
    messages: s.messages,
    todos: s.todos,
    permissions: d.initialPermissions,
    questions: d.initialQuestions,
  };
});
writeFileSync(new URL("../assets/demo-sessions.json", import.meta.url), JSON.stringify(out));
console.log(`ekspor ${out.length} sesi demo`);
