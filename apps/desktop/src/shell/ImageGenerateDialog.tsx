// Dialog "Generate gambar" (ADR-077): prompt + ukuran → model gambar bawaan → PNG di `<project>/images/`, lalu dibuka
// di Panel 3. Di luar aplikasi desktop (tanpa kunci/Rust) dialog menjelaskan bahwa fitur tidak tersedia.
import { useI18n } from "@zandor/i18n/react";
import { Button, Dialog, Field, Select, Textarea, useToast } from "@zandor/ui";
import { type FormEvent, useEffect, useState } from "react";
import { generateWithDefault, knownRoots } from "./AgentControlDriver";
import { type ImageSize, imageFileName } from "./agent-control";
import { type ControlHost, loadControlHost } from "./control-host";
import { useModelPrefs, useServices } from "./services";

export function ImageGenerateDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const { t } = useI18n();
  const toast = useToast();
  const { projects, modelPrefs, docs, panels, nav } = useServices();
  const prefs = useModelPrefs();
  const [host, setHost] = useState<ControlHost | null | undefined>(undefined);
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState<ImageSize>("1024x1024");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open && host === undefined) void loadControlHost().then(setHost);
    if (open) setError(null);
  }, [open, host]);

  const root = projects.selected()?.session.directory ?? knownRoots(projects)[0] ?? null;
  const model = prefs.defaultImage;
  const blocker = host === null ? t("img.unavailable") : !model ? t("img.noModel") : !root ? t("img.noProject") : null;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!host || !root || !prompt.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const rel = await generateWithDefault(host, modelPrefs, {
        prompt: prompt.trim(),
        size,
        root,
        rel: imageFileName(prompt),
      });
      docs.open(root, rel);
      panels.actions?.reveal("docs");
      toast.show({ title: t("img.saved", { path: rel }), tone: "success", timeoutMs: 5000 });
      setPrompt("");
      onOpenChange(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !busy && onOpenChange(o)}
      title={t("img.title")}
      description={t("img.body")}
      footer={
        <>
          {!model && host !== null && (
            <Button
              onClick={() => {
                onOpenChange(false);
                nav.push("/settings/models");
              }}
            >
              {t("img.openSettings")}
            </Button>
          )}
          <Button onClick={() => onOpenChange(false)} disabled={busy}>
            {t("ui.cancel")}
          </Button>
          <Button variant="solid" type="submit" form="image-form" disabled={busy || !!blocker || !prompt.trim()}>
            {busy ? t("img.busy") : t("img.generate")}
          </Button>
        </>
      }
    >
      <form id="image-form" className="imggen__form" onSubmit={(e) => void submit(e)}>
        {model && (
          <p className="z-small imggen__model">{t("img.model", { model: `${model.providerID}/${model.modelID}` })}</p>
        )}
        <Field label={t("img.prompt")}>
          {(c) => (
            <Textarea
              {...c}
              autoFocus
              rows={4}
              value={prompt}
              disabled={busy}
              onChange={(e) => setPrompt(e.target.value)}
            />
          )}
        </Field>
        <Select
          aria-label={t("img.size")}
          value={size}
          disabled={busy}
          items={[
            { value: "1024x1024", label: t("img.sizeSquare") },
            { value: "1536x1024", label: t("img.sizeLandscape") },
            { value: "1024x1536", label: t("img.sizePortrait") },
          ]}
          onValueChange={(v) => setSize(v as ImageSize)}
        />
        {blocker && host !== undefined && (
          <p className="z-small imggen__note" role="status">
            {blocker}
          </p>
        )}
        {error && (
          <p className="z-small imggen__error" role="alert">
            {error}
          </p>
        )}
      </form>
    </Dialog>
  );
}
