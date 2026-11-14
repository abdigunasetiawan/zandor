// Tampilan sesi demo bawaan: memakai komponen transkrip yang sama dengan percakapan nyata, tetapi isinya statis dan kartu
// izin/pertanyaan dijawab lokal (hanya ditampilkan lewat toast, tidak dikirim ke engine). Diberi label jelas "Demo".
import { useI18n } from "@zandor/i18n/react";
import { Badge, Button, PinIcon, useToast } from "@zandor/ui";
import { useMemo, useState } from "react";
import { PromptDock, Transcript } from "./ChatPanel";
import { type DemoApi, DemoContext } from "./demo-context";
import { demoById } from "./demo-sessions";
import { SessionFlagsContext } from "./session-context";

export function DemoChat({ id }: { id: string }) {
  const { t } = useI18n();
  const toast = useToast();
  const def = demoById(id);
  const base = useMemo(() => def?.build(), [def]);
  const [permissions, setPermissions] = useState(def?.initialPermissions ?? []);
  const [questions, setQuestions] = useState(def?.initialQuestions ?? []);
  if (!def || !base) return null;

  const api: DemoApi = {
    permission(requestId, reply) {
      setPermissions((p) => p.filter((x) => x.id !== requestId));
      toast.show({
        title: t("demo.answered.permission", { reply: t(`demo.reply.${reply}` as never) }),
        tone: "success",
      });
    },
    question(requestId, answers) {
      setQuestions((q) => q.filter((x) => x.id !== requestId));
      toast.show({
        title: answers
          ? t("demo.answered.question", { answers: answers.map((a) => a.join(", ")).join(" | ") })
          : t("demo.answered.skipped"),
        tone: "success",
      });
    },
  };
  const reset = () => {
    setPermissions(def.initialPermissions);
    setQuestions(def.initialQuestions);
  };
  const pending = def.initialPermissions.length + def.initialQuestions.length > 0;

  return (
    <DemoContext.Provider value={api}>
      <SessionFlagsContext.Provider value={{ hasProject: false }}>
        <div className="demo-banner" role="note">
          <Badge tone="outline">
            <PinIcon size="sm" /> {t("demo.badge")}
          </Badge>
          <span className="z-small demo-banner__text">{t("demo.banner")}</span>
          {pending && (
            <Button size="sm" onClick={reset}>
              {t("demo.reset")}
            </Button>
          )}
        </div>
        <Transcript session={base} follow={false} />
        <PromptDock sessionId={base.sessionID} permissions={permissions} questions={questions} todos={base.todos} />
      </SessionFlagsContext.Provider>
    </DemoContext.Provider>
  );
}
