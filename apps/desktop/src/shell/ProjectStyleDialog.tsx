// Dialog "Kustomisasi tampilan project": logo (teks/emoji atau gambar), latar (warna, gradien dengan generator + templat, atau
// gambar unggahan), dan warna border. Pratinjau langsung. Disimpan lewat AppearanceStore (tervalidasi) per project.
import {
  DEFAULT_GRADIENT,
  DEFAULT_PROJECT_COLOR,
  FALLBACK_PICKER_COLOR,
  GRADIENT_TEMPLATES,
  type GradientSpec,
  gradientCss,
  isColor,
  MAX_STOPS,
  MAX_STYLE_IMAGE_CHARS,
  MIN_STOPS,
  NEW_STOP_COLOR,
  type Project,
  type ProjectStyle,
  parseGradient,
  randomGradient,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, Dialog, Field, IconButton, Input, Select, Slider, Tabs, TrashIcon, useToast } from "@zandor/ui";
import { useRef, useState } from "react";
import { imageToDataUrl } from "./image-resize";
import { useAppearance, useServices } from "./services";

type BgMode = "none" | "color" | "gradient" | "image";
const HEX6 = /^#[0-9a-f]{6}$/i;
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** CSS latar untuk pratinjau/sidebar dari gaya (image > gradient > background). */
export function projectBackground(s: ProjectStyle | undefined): string | undefined {
  if (!s) return undefined;
  if (s.image) return `center / cover no-repeat url(${s.image})`;
  return s.gradient ?? s.background;
}

function modeOf(s: ProjectStyle | undefined): BgMode {
  if (s?.image) return "image";
  if (s?.gradient) return "gradient";
  if (s?.background) return "color";
  return "none";
}

export function ProjectStyleDialog({ project, onClose }: { project: Project; onClose: () => void }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const saved = useAppearance().projects[project.id];
  const toast = useToast();
  const [mode, setMode] = useState<BgMode>(modeOf(saved));
  const [color, setColor] = useState(saved?.background ?? DEFAULT_PROJECT_COLOR);
  const [spec, setSpec] = useState<GradientSpec>(
    () => (saved?.gradient ? parseGradient(saved.gradient) : null) ?? DEFAULT_GRADIENT,
  );
  const [image, setImage] = useState<string | null>(saved?.image ?? null);
  const [logo, setLogo] = useState(saved?.logo ?? "");
  const [logoImage, setLogoImage] = useState<string | null>(saved?.logoImage ?? null);
  const [border, setBorder] = useState(saved?.border ?? "");
  const bgFile = useRef<HTMLInputElement>(null);
  const logoFile = useRef<HTMLInputElement>(null);

  const draft: ProjectStyle = {
    ...(border && isColor(border) ? { border } : {}),
    ...(logo.trim() ? { logo: logo.trim() } : {}),
    ...(logoImage ? { logoImage } : {}),
    ...(mode === "color" && isColor(color) ? { background: color } : {}),
    ...(mode === "gradient" ? { gradient: gradientCss(spec) } : {}),
    ...(mode === "image" && image ? { image } : {}),
  };

  const save = () => {
    const entries: Array<[keyof ProjectStyle, string | undefined]> = [
      ["border", draft.border],
      ["logo", draft.logo],
      ["logoImage", draft.logoImage],
      ["background", draft.background],
      ["gradient", draft.gradient],
      ["image", draft.image],
    ];
    for (const [k, v] of entries) {
      const err = store.setProjectStyle(project.id, k, v ?? null);
      if (err) {
        toast.show({ title: t("pstyle.error", { error: err }), tone: "danger" });
        return;
      }
    }
    onClose();
  };

  const upload = async (file: File | undefined, kind: "bg" | "logo") => {
    if (!file) return;
    try {
      if (kind === "bg") {
        setImage(
          await imageToDataUrl(file, {
            maxSide: 640,
            mime: "image/jpeg",
            quality: 0.8,
            maxChars: MAX_STYLE_IMAGE_CHARS,
          }),
        );
        setMode("image");
      } else {
        setLogoImage(await imageToDataUrl(file, { maxSide: 64, mime: "image/png", maxChars: 120_000 }));
      }
    } catch (e) {
      toast.show({ title: t("pstyle.uploadError", { error: errText(e) }), tone: "danger" });
    }
  };

  const setStop = (i: number, patch: Partial<GradientSpec["stops"][number]>) =>
    setSpec((s) => ({ ...s, stops: s.stops.map((x, j) => (j === i ? { ...x, ...patch } : x)) }));

  const colorField = (label: string, value: string, onChange: (v: string) => void) => (
    <div className="pstyle__color">
      <input
        type="color"
        aria-label={label}
        value={HEX6.test(value) ? value : FALLBACK_PICKER_COLOR}
        onChange={(e) => onChange(e.target.value)}
      />
      <Input
        aria-label={`${label} (hex)`}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={t("pstyle.hexHint")}
      />
    </div>
  );

  const bg = projectBackground(draft);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("pstyle.title", { name: project.name })}
      footer={
        <>
          <Button
            variant="ghost"
            onClick={() => {
              setMode("none");
              setLogo("");
              setLogoImage(null);
              setBorder("");
              setImage(null);
            }}
          >
            {t("pstyle.reset")}
          </Button>
          <Button onClick={onClose}>{t("ui.cancel")}</Button>
          <Button variant="solid" onClick={save}>
            {t("sb.save")}
          </Button>
        </>
      }
    >
      <div className="pstyle">
        <div
          className="pstyle__preview"
          data-bg={bg ? "" : undefined}
          style={{
            ...(bg ? { background: bg } : {}),
            ...(draft.border ? { boxShadow: `inset 0 0 0 var(--z-border-width) ${draft.border}` } : {}),
          }}
          data-testid="pstyle-preview"
        >
          <span className="tree__logo" aria-hidden="true">
            {logoImage ? <img src={logoImage} alt="" className="pstyle__logoimg" /> : logo.trim() || "📁"}
          </span>
          <span className="tree__label">{project.name}</span>
        </div>

        <section className="pstyle__group" aria-labelledby="pstyle-bg-head">
          <h3 id="pstyle-bg-head" className="z-label pstyle__head">
            {t("pstyle.background")}
          </h3>
          <Tabs
            aria-label={t("pstyle.background")}
            value={mode}
            onValueChange={(v) => setMode(v as BgMode)}
            items={[
              {
                value: "none",
                label: t("pstyle.bg.none"),
                content: <p className="z-small pstyle__hint">{t("pstyle.bg.noneHint")}</p>,
              },
              {
                value: "color",
                label: t("pstyle.bg.color"),
                content: colorField(t("pstyle.bg.color"), color, setColor),
              },
              {
                value: "gradient",
                label: t("pstyle.bg.gradient"),
                content: (
                  <div className="pstyle__gradient">
                    <fieldset className="pstyle__templates">
                      <legend className="z-visually-hidden">{t("pstyle.templates")}</legend>
                      {GRADIENT_TEMPLATES.map((tpl) => (
                        <button
                          key={tpl.id}
                          type="button"
                          className="pstyle__tpl"
                          title={tpl.name}
                          aria-label={t("pstyle.template", { name: tpl.name })}
                          style={{ background: gradientCss(tpl.spec) }}
                          onClick={() => setSpec(tpl.spec)}
                        />
                      ))}
                    </fieldset>
                    <Field label={t("pstyle.type")}>
                      {(c) => (
                        <Select
                          {...c}
                          value={spec.type}
                          onValueChange={(v) => setSpec((s) => ({ ...s, type: v as GradientSpec["type"] }))}
                          items={[
                            { value: "linear", label: t("pstyle.type.linear") },
                            { value: "radial", label: t("pstyle.type.radial") },
                          ]}
                        />
                      )}
                    </Field>
                    {spec.type === "linear" && (
                      <Slider
                        label={t("pstyle.angle", { deg: spec.angle })}
                        min={0}
                        max={360}
                        value={spec.angle}
                        onValueChange={(angle) => setSpec((s) => ({ ...s, angle }))}
                      />
                    )}
                    <ul className="pstyle__stops" aria-label={t("pstyle.stops")}>
                      {spec.stops.map((s, i) => (
                        // biome-ignore lint/suspicious/noArrayIndexKey: titik warna diidentifikasi urutannya
                        <li key={i} className="pstyle__stop">
                          {colorField(t("pstyle.stop", { n: i + 1 }), s.color, (color) => setStop(i, { color }))}
                          <Slider
                            label={t("pstyle.stopPos", { n: i + 1 })}
                            min={0}
                            max={100}
                            value={s.pos}
                            onValueChange={(pos) => setStop(i, { pos })}
                          />
                          <IconButton
                            size="sm"
                            label={t("pstyle.removeStop", { n: i + 1 })}
                            icon={<TrashIcon size="sm" />}
                            disabled={spec.stops.length <= MIN_STOPS}
                            onClick={() => setSpec((sp) => ({ ...sp, stops: sp.stops.filter((_, j) => j !== i) }))}
                          />
                        </li>
                      ))}
                    </ul>
                    <div className="pstyle__row">
                      <Button
                        size="sm"
                        disabled={spec.stops.length >= MAX_STOPS}
                        onClick={() =>
                          setSpec((s) => ({ ...s, stops: [...s.stops, { color: NEW_STOP_COLOR, pos: 100 }] }))
                        }
                      >
                        {t("pstyle.addStop")}
                      </Button>
                      <Button size="sm" onClick={() => setSpec(randomGradient())}>
                        {t("pstyle.random")}
                      </Button>
                    </div>
                  </div>
                ),
              },
              {
                value: "image",
                label: t("pstyle.bg.image"),
                content: (
                  <div className="pstyle__row">
                    <input
                      ref={bgFile}
                      type="file"
                      accept="image/png,image/jpeg,image/webp,image/gif"
                      hidden
                      aria-label={t("pstyle.bg.upload")}
                      onChange={(e) => {
                        void upload(e.target.files?.[0], "bg");
                        e.target.value = "";
                      }}
                    />
                    <Button size="sm" onClick={() => bgFile.current?.click()}>
                      {t("pstyle.bg.upload")}
                    </Button>
                    {image && (
                      <Button size="sm" variant="ghost" onClick={() => setImage(null)}>
                        {t("pstyle.bg.removeImage")}
                      </Button>
                    )}
                    <p className="z-small pstyle__hint">{t("pstyle.bg.imageHint")}</p>
                  </div>
                ),
              },
            ]}
          />
        </section>

        <Field label={t("pstyle.logo")} description={t("pstyle.logoHint")}>
          {(c) => (
            <div className="pstyle__inline">
              <Input {...c} value={logo} maxLength={8} placeholder="🚀" onChange={(e) => setLogo(e.target.value)} />
              <Button onClick={() => logoFile.current?.click()}>{t("pstyle.logo.upload")}</Button>
              {logoImage && (
                <Button variant="ghost" onClick={() => setLogoImage(null)}>
                  {t("pstyle.logo.remove")}
                </Button>
              )}
            </div>
          )}
        </Field>
        <input
          ref={logoFile}
          type="file"
          accept="image/png,image/jpeg,image/webp,image/gif"
          hidden
          aria-label={t("pstyle.logo.upload")}
          onChange={(e) => {
            void upload(e.target.files?.[0], "logo");
            e.target.value = "";
          }}
        />
        <Field label={t("pstyle.border")}>{() => colorField(t("pstyle.border"), border, setBorder)}</Field>
      </div>
    </Dialog>
  );
}
