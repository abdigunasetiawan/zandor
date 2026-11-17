// Pengaturan → Provider & model: model bawaan (chat, gambar), daftar provider (katalog engine, lokal, kustom
// kompatibel OpenAI), pengelola model per provider (tampil di pemilih composer, kemampuan, logo). Kunci API hanya lewat host.
import {
  compactTokens,
  type ModelOption,
  modelKey,
  modelRefKey,
  type ProviderModelSpec,
  type ProviderView,
  pingKey,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  Badge,
  Button,
  Dialog,
  detectLogoKey,
  EditIcon,
  EyeIcon,
  EyeOffIcon,
  Field,
  IconButton,
  ImageIcon,
  Input,
  ModelLogo,
  PingIcon,
  PlusIcon,
  SearchField,
  Select,
  Spinner,
  Switch,
  TrashIcon,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { ConnectionPanel, pingLabel } from "./ConnectionPanel";
import { LogoDialog } from "./LogoDialog";
import { useChat, useEngineState, useModelPrefs, useProviders, useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const AUTO = "";

function Row({
  title,
  body,
  children,
  testId,
}: {
  title: string;
  body?: string;
  children: ReactNode;
  testId?: string;
}) {
  return (
    <div className="settings__row" data-setting={testId}>
      <div className="settings__text">
        <span className="z-label">{title}</span>
        {body && <span className="z-small">{body}</span>}
      </div>
      <div className="settings__control">{children}</div>
    </div>
  );
}

// ---------------- model bawaan ----------------

/**
 * Model kecil engine (opencode `small_model`: judul sesi dan ringkasan). Disimpan oleh Rust di `model-roles.json` dan
 * dimasukkan ke config engine saat engine dimulai, jadi perubahan berlaku setelah engine dimulai ulang.
 */
function SmallModelRow() {
  const { t } = useI18n();
  const { host } = useServices();
  const chat = useChat();
  const [value, setValue] = useState<string | null | undefined>(undefined);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    host.smallModel().then(
      (v) => alive && setValue(v),
      (e) => alive && setError(e instanceof Error ? e.message : String(e)),
    );
    return () => {
      alive = false;
    };
  }, [host]);
  const items = useMemo(() => {
    const list = [
      { value: AUTO, label: t("ms.smallAuto") },
      ...chat.models.map((m) => ({ value: modelKey(m), label: `${m.providerName} · ${m.name}` })),
    ];
    // Model tersimpan yang tidak lagi tersedia tetap ditampilkan apa adanya (bukan diam-diam dianggap Otomatis).
    if (value && !list.some((i) => i.value === value)) list.push({ value, label: t("ms.smallMissing", { id: value }) });
    return list;
  }, [chat.models, t, value]);
  const change = (v: string) => {
    const next = v === AUTO ? null : v;
    setError(null);
    host.setSmallModel(next).then(
      () => {
        setValue(next);
        setPending(true);
      },
      (e) => setError(e instanceof Error ? e.message : String(e)),
    );
  };
  return (
    <Row title={t("ms.small")} body={host.native ? t("ms.smallBody") : t("ms.smallWeb")} testId="smallModel">
      <div className="ms__small">
        <Select
          aria-label={t("ms.small")}
          value={value ?? AUTO}
          items={items}
          disabled={!host.native || value === undefined}
          onValueChange={change}
        />
        {pending && (
          <Button
            size="sm"
            onClick={() => {
              setPending(false);
              void host.restart();
            }}
          >
            {t("ms.smallRestart")}
          </Button>
        )}
        {error && (
          <span className="z-small ms__error" role="alert">
            {error}
          </span>
        )}
      </div>
    </Row>
  );
}

function DefaultsSection() {
  const { t } = useI18n();
  const { modelPrefs } = useServices();
  const prefs = useModelPrefs();
  const chat = useChat();
  const items = useMemo(
    () => [
      { value: AUTO, label: t("ms.auto") },
      ...chat.models.map((m) => ({ value: modelKey(m), label: `${m.providerName} · ${m.name}` })),
    ],
    [chat.models, t],
  );
  const refOf = (key: string) => {
    const m = chat.models.find((o) => modelKey(o) === key);
    return m ? { providerID: m.providerID, modelID: m.modelID } : null;
  };
  const known = (r: { providerID: string; modelID: string } | null) =>
    r && chat.models.some((m) => modelKey(m) === modelRefKey(r)) ? modelRefKey(r) : AUTO;
  return (
    <section className="settings__group" aria-labelledby="ms-h-defaults">
      <h2 id="ms-h-defaults" className="z-h4">
        {t("ms.defaults")}
      </h2>
      <Row title={t("ms.defaultChat")} body={t("ms.defaultChatBody")} testId="defaultChatModel">
        <Select
          aria-label={t("ms.defaultChat")}
          value={known(prefs.defaultChat)}
          items={items}
          onValueChange={(v) => modelPrefs.setDefaultChat(v === AUTO ? null : refOf(v))}
        />
      </Row>
      <SmallModelRow />
      <Row title={t("ms.defaultImage")} body={t("ms.defaultImageBody")} testId="defaultImageModel">
        <Select
          aria-label={t("ms.defaultImage")}
          value={known(prefs.defaultImage)}
          items={items}
          onValueChange={(v) => modelPrefs.setDefaultImage(v === AUTO ? null : refOf(v))}
        />
      </Row>
    </section>
  );
}

// ---------------- daftar provider ----------------

type ProviderRow = { id: string; name: string; spec: ProviderView | null; models: ModelOption[] };

function useProviderRows(): ProviderRow[] {
  const chat = useChat();
  const pv = useProviders();
  return useMemo(() => {
    const rows = new Map<string, ProviderRow>();
    for (const p of pv.providers) rows.set(p.id, { id: p.id, name: p.name, spec: p, models: [] });
    for (const c of pv.catalog) {
      if (c.connected && !rows.has(c.id)) rows.set(c.id, { id: c.id, name: c.name, spec: null, models: [] });
    }
    for (const m of chat.models) {
      const r = rows.get(m.providerID) ?? { id: m.providerID, name: m.providerName, spec: null, models: [] };
      r.models.push(m);
      rows.set(m.providerID, r);
    }
    return [...rows.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [pv.providers, pv.catalog, chat.models]);
}

function ProvidersSection({
  onManage,
  onConnect,
}: {
  onManage: (id: string) => void;
  onConnect: (editing: ProviderView | null) => void;
}) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const prefs = useModelPrefs();
  const pv = useProviders();
  const engine = useEngineState();
  const confirm = useConfirm();
  const toast = useToast();
  const rows = useProviderRows();

  const remove = async (p: ProviderView) => {
    const ok = await confirm({
      title: t("pv.confirmDeleteTitle", { name: p.name }),
      description: t("pv.confirmDeleteBody"),
      confirmLabel: t("pv.delete"),
      tone: "danger",
    });
    if (ok)
      await store
        .remove(p.id)
        .catch((e) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" }));
  };

  return (
    <section className="settings__group" aria-labelledby="ms-h-providers">
      <div className="ms__head">
        <h2 id="ms-h-providers" className="z-h4">
          {t("ms.providers")}
        </h2>
        <div className="ms__headactions">
          <Button variant="solid" onClick={() => onConnect(null)}>
            <PlusIcon size="sm" /> {t("pv.add")}
          </Button>
        </div>
      </div>
      <p className="z-small">{t("ms.providersBody")}</p>
      <p className="z-small ms__secure">{store.secure ? t("pv.secure.yes") : t("pv.secure.no")}</p>
      {pv.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {pv.error}
        </p>
      )}
      {rows.length === 0 && pv.loaded && <p className="z-p">{t("ms.noProviders")}</p>}
      <ul className="ms__list">
        {rows.map((r) => {
          // engine belum tentu memuat model koneksi yang baru disimpan: pakai daftar tersimpan sebagai cadangan
          const keys =
            r.models.length > 0 ? r.models.map(modelKey) : (r.spec?.models ?? []).map((m) => `${r.id}/${m.id}`);
          const hidden = keys.filter((k) => prefs.hidden.includes(k)).length;
          const connected = pv.connected.includes(r.id) || r.models.length > 0;
          return (
            <li key={r.id} className="ms__prov" aria-label={r.name}>
              <ModelLogo providerID={r.id} name={r.name} className="ms__logo" />
              <div className="ms__provtext">
                <span className="z-label">{r.name}</span>
                <span className="z-small ms__sub">
                  {[r.spec?.baseUrl, t("ms.modelCount", { shown: keys.length - hidden, total: keys.length })]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </div>
              <span className="ms__badges">
                <Badge tone="outline">{t(`pv.kind.${r.spec?.kind ?? "builtin"}` as never)}</Badge>
                {r.spec && (
                  <Badge tone={r.spec.hasKey ? "success" : "neutral"}>
                    {r.spec.hasKey ? t("pv.key.saved") : t("pv.key.none")}
                  </Badge>
                )}
                <Badge tone={engine.status !== "ready" || !connected ? "warning" : "success"}>
                  {engine.status !== "ready"
                    ? t("pv.status.engineDown")
                    : connected
                      ? t("pv.status.connected")
                      : t("pv.status.notConnected")}
                </Badge>
              </span>
              <span className="ms__actions">
                <Button
                  size="sm"
                  onClick={() => onManage(r.id)}
                  disabled={r.models.length === 0 && !r.spec?.models.length}
                >
                  {t("ms.manageModels")}
                </Button>
                {r.spec && (
                  <>
                    <IconButton label={t("pv.edit")} size="sm" icon={<EditIcon />} onClick={() => onConnect(r.spec)} />
                    <IconButton
                      label={t("pv.delete")}
                      size="sm"
                      icon={<TrashIcon />}
                      onClick={() => void (r.spec && remove(r.spec))}
                    />
                  </>
                )}
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

// ---------------- pengelola model ----------------

type GroupBy = "none" | "family" | "capability";

function capabilityGroup(m: ModelOption): string {
  if (m.reasoning) return "reasoning";
  if (m.attachment) return "vision";
  if (m.toolCall) return "tools";
  return "other";
}

function CapBadges({ m }: { m: ModelOption }) {
  const { t } = useI18n();
  return (
    <span className="ms__caps">
      {m.contextLimit ? (
        <Badge tone="outline">{t("ms.cap.context", { n: compactTokens(m.contextLimit) })}</Badge>
      ) : null}
      {m.outputLimit ? <Badge tone="outline">{t("ms.cap.output", { n: compactTokens(m.outputLimit) })}</Badge> : null}
      {m.reasoning ? <Badge tone="success">{t("ms.cap.reasoning")}</Badge> : null}
      {m.toolCall ? <Badge tone="neutral">{t("ms.cap.tools")}</Badge> : null}
      {m.attachment ? <Badge tone="neutral">{t("ms.cap.attachment")}</Badge> : null}
      {m.variants.length > 0 ? <Badge tone="neutral">{t("ms.cap.variants", { n: m.variants.length })}</Badge> : null}
    </span>
  );
}

function ModelEditDialog({
  provider,
  model,
  onClose,
}: {
  provider: ProviderView;
  model: ProviderModelSpec;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const toast = useToast();
  const [name, setName] = useState(model.name ?? "");
  const [context, setContext] = useState(model.context ? String(model.context) : "");
  const [output, setOutput] = useState(model.output ? String(model.output) : "");
  const [reasoning, setReasoning] = useState(model.reasoning ?? false);
  const [toolCall, setToolCall] = useState(model.toolCall ?? true);
  const [attachment, setAttachment] = useState(model.attachment ?? false);
  const [saving, setSaving] = useState(false);
  const num = (s: string) => {
    const n = Number(s);
    return Number.isFinite(n) && n > 0 ? Math.round(n) : undefined;
  };
  const save = async () => {
    setSaving(true);
    try {
      const next: ProviderModelSpec = {
        id: model.id,
        ...(name.trim() ? { name: name.trim() } : {}),
        ...(num(context) ? { context: num(context) as number } : {}),
        ...(num(output) ? { output: num(output) as number } : {}),
        reasoning,
        toolCall,
        attachment,
      };
      await store.saveModels(
        provider.id,
        provider.models.map((m) => (m.id === model.id ? next : m)),
      );
      toast.show({ title: t("pv.saved"), tone: "success", timeoutMs: 2000 });
      onClose();
    } catch (e) {
      toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
    } finally {
      setSaving(false);
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("ms.editModel", { id: model.id })}
      description={t("ms.editModelBody")}
      footer={
        <>
          <Button onClick={onClose}>{t("ui.cancel")}</Button>
          <Button variant="solid" loading={saving} onClick={() => void save()}>
            {t("pv.save")}
          </Button>
        </>
      }
    >
      <div className="pform">
        <Field label={t("pv.name")}>
          {(c) => <Input {...c} value={name} placeholder={model.id} onChange={(e) => setName(e.target.value)} />}
        </Field>
        <div className="pform__row">
          <Field label={t("ms.field.context")}>
            {(c) => (
              <Input
                {...c}
                type="number"
                min={1}
                value={context}
                placeholder="128000"
                onChange={(e) => setContext(e.target.value)}
              />
            )}
          </Field>
          <Field label={t("ms.field.output")}>
            {(c) => (
              <Input
                {...c}
                type="number"
                min={1}
                value={output}
                placeholder="16384"
                onChange={(e) => setOutput(e.target.value)}
              />
            )}
          </Field>
        </div>
        <Switch label={t("ms.cap.reasoning")} checked={reasoning} onCheckedChange={setReasoning} />
        <Switch label={t("ms.cap.tools")} checked={toolCall} onCheckedChange={setToolCall} />
        <Switch label={t("ms.cap.attachment")} checked={attachment} onCheckedChange={setAttachment} />
      </div>
    </Dialog>
  );
}

function ModelManager({ providerId, onBack }: { providerId: string; onBack: () => void }) {
  const { t } = useI18n();
  const { modelPrefs, providers: store } = useServices();
  const prefs = useModelPrefs();
  const pv = useProviders();
  const rows = useProviderRows();
  const row = rows.find((r) => r.id === providerId);
  const [query, setQuery] = useState("");
  const [groupBy, setGroupBy] = useState<GroupBy>("none");
  const [logoFor, setLogoFor] = useState<{ modelID?: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [fetching, setFetching] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  const spec = row?.spec ?? null;
  const editable = spec !== null && spec.kind !== "builtin";
  // model dari engine (lengkap dengan kemampuan); bila engine belum siap, jatuh ke daftar yang disimpan
  const models: ModelOption[] = useMemo(() => {
    if (!row) return [];
    if (row.models.length > 0) return row.models;
    return (row.spec?.models ?? []).map((m) => ({
      providerID: row.id,
      providerName: row.name,
      modelID: m.id,
      name: m.name ?? m.id,
      variants: [],
      ...(m.context ? { contextLimit: m.context } : {}),
      ...(m.output ? { outputLimit: m.output } : {}),
      ...(m.reasoning !== undefined ? { reasoning: m.reasoning } : {}),
      ...(m.toolCall !== undefined ? { toolCall: m.toolCall } : {}),
      ...(m.attachment !== undefined ? { attachment: m.attachment } : {}),
    }));
  }, [row]);

  const q = query.trim().toLowerCase();
  const shown = models.filter((m) => !q || `${m.name} ${m.modelID}`.toLowerCase().includes(q));
  const refs = shown.map((m) => ({ providerID: m.providerID, modelID: m.modelID }));
  const hiddenCount = models.filter((m) => prefs.hidden.includes(modelKey(m))).length;

  const groups = useMemo(() => {
    const out = new Map<string, ModelOption[]>();
    for (const m of shown) {
      const k =
        groupBy === "family"
          ? (detectLogoKey(m.modelID) ?? detectLogoKey(m.name) ?? "other")
          : groupBy === "capability"
            ? capabilityGroup(m)
            : "";
      out.set(k, [...(out.get(k) ?? []), m]);
    }
    return [...out.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [shown, groupBy]);

  if (!row) {
    return (
      <section className="settings__group">
        <Button onClick={onBack}>
          <ArrowLeftIcon size="sm" /> {t("ms.back")}
        </Button>
        <p className="z-p">{t("ms.providerGone")}</p>
      </section>
    );
  }

  const fetchModels = async () => {
    if (!spec?.baseUrl) return;
    setFetching(true);
    setNote(null);
    const r = await store.probe(spec.baseUrl, undefined, spec.id);
    setFetching(false);
    if (!r.ok) return setNote(t("pv.probeFailed", { error: r.error ?? "?" }));
    const have = new Set(spec.models.map((m) => m.id));
    const added = r.models.filter((id) => !have.has(id));
    if (added.length > 0) {
      try {
        await store.saveModels(spec.id, [...spec.models, ...added.map((id) => ({ id }))]);
      } catch (e) {
        return setNote(errText(e));
      }
    }
    setNote(t("ms.fetched", { found: r.models.length, added: added.length }));
  };

  const groupLabel = (k: string) =>
    groupBy === "capability" ? t(`ms.group.${k}` as never) : k === "other" ? t("ms.group.other") : k;
  const editSpec = editing ? spec?.models.find((m) => m.id === editing) : undefined;
  const logoModel = logoFor?.modelID ? models.find((m) => m.modelID === logoFor.modelID) : undefined;

  return (
    <section className="settings__group ms__manager" aria-label={t("ms.manageTitle", { name: row.name })}>
      <div className="ms__head">
        <Button onClick={onBack}>
          <ArrowLeftIcon size="sm" /> {t("ms.back")}
        </Button>
        <div className="ms__headactions">
          {editable && (
            <Button onClick={() => void fetchModels()} loading={fetching}>
              {t("pv.fetchModels")}
            </Button>
          )}
          <Button onClick={() => setLogoFor({})}>
            <ImageIcon size="sm" /> {t("ms.providerLogo")}
          </Button>
        </div>
      </div>
      <div className="ms__title">
        <ModelLogo providerID={row.id} name={row.name} className="ms__logo" />
        <h2 className="z-h4">{row.name}</h2>
        <span className="z-small">
          {t("ms.visibleOf", { shown: models.length - hiddenCount, total: models.length })}
        </span>
      </div>
      {note && (
        <p className="z-small" role="status">
          {note}
        </p>
      )}
      <div className="ms__toolbar">
        <SearchField aria-label={t("mp.search")} placeholder={t("mp.search")} value={query} onValueChange={setQuery} />
        <Select
          aria-label={t("ms.groupBy")}
          value={groupBy}
          onValueChange={(v) => setGroupBy(v as GroupBy)}
          items={(["none", "family", "capability"] as const).map((g) => ({
            value: g,
            label: t(`ms.groupBy.${g}` as never),
          }))}
        />
        <Button disabled={refs.length === 0} onClick={() => modelPrefs.setVisible(refs, true)}>
          <EyeIcon size="sm" /> {t("ms.selectAll")}
        </Button>
        <Button disabled={refs.length === 0} onClick={() => modelPrefs.setVisible(refs, false)}>
          <EyeOffIcon size="sm" /> {t("ms.deselectAll")}
        </Button>
      </div>
      {shown.length === 0 && <p className="z-p">{t("mp.none")}</p>}
      {groups.map(([k, list]) => (
        <div key={k || "all"} className="ms__group">
          {groupBy !== "none" && (
            <h3 className="z-label ms__grouphead">
              {groupLabel(k)} <span className="z-small">({list.length})</span>
            </h3>
          )}
          <ul className="ms__models">
            {list.map((m) => {
              const visible = !prefs.hidden.includes(modelKey(m));
              return (
                <li key={m.modelID} className="ms__model" data-hidden={visible ? undefined : ""}>
                  <Switch
                    label=""
                    aria-label={t("ms.showInPicker", { name: m.name })}
                    checked={visible}
                    onCheckedChange={(v) =>
                      modelPrefs.setVisible([{ providerID: m.providerID, modelID: m.modelID }], v)
                    }
                  />
                  <button
                    type="button"
                    className="ms__logobtn"
                    title={t("ms.modelLogo")}
                    aria-label={t("ms.modelLogoFor", { name: m.name })}
                    onClick={() => setLogoFor({ modelID: m.modelID })}
                  >
                    <ModelLogo providerID={m.providerID} modelID={m.modelID} name={m.name} />
                  </button>
                  <span className="ms__modeltext">
                    <span className="z-label">{m.name}</span>
                    <span className="z-small ms__sub">{m.modelID}</span>
                  </span>
                  <CapBadges m={m} />
                  {editable && (
                    <>
                      <span className="z-small ms__ping" role="status">
                        {pv.pings[pingKey(m.providerID, m.modelID)]?.status === "pending" && <Spinner size="sm" />}
                        {(() => {
                          const ping = pv.pings[pingKey(m.providerID, m.modelID)];
                          return ping?.status === "done" ? (
                            <span data-tone={ping.result.ok ? "ok" : "bad"}>{pingLabel(t, ping.result)}</span>
                          ) : null;
                        })()}
                      </span>
                      <IconButton
                        label={t("pv.ping", { name: m.name })}
                        size="sm"
                        icon={<PingIcon />}
                        disabled={pv.pings[pingKey(m.providerID, m.modelID)]?.status === "pending"}
                        onClick={() => void store.ping(m.providerID, m.modelID)}
                      />
                    </>
                  )}
                  {editable && (
                    <IconButton
                      label={t("ms.editModel", { id: m.name })}
                      size="sm"
                      icon={<EditIcon />}
                      onClick={() => setEditing(m.modelID)}
                    />
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      ))}
      {logoFor && (
        <LogoDialog
          title={
            logoFor.modelID ? t("ms.modelLogoFor", { name: logoModel?.name ?? logoFor.modelID }) : t("ms.providerLogo")
          }
          providerID={row.id}
          {...(logoFor.modelID ? { modelID: logoFor.modelID } : {})}
          name={logoModel?.name ?? row.name}
          custom={
            logoFor.modelID
              ? (prefs.modelLogos[`${row.id}/${logoFor.modelID}`] ?? null)
              : (prefs.providerLogos[row.id] ?? null)
          }
          onSave={(url) =>
            logoFor.modelID
              ? modelPrefs.setModelLogo({ providerID: row.id, modelID: logoFor.modelID }, url)
              : modelPrefs.setProviderLogo(row.id, url)
          }
          onClose={() => setLogoFor(null)}
        />
      )}
      {spec && editSpec && <ModelEditDialog provider={spec} model={editSpec} onClose={() => setEditing(null)} />}
    </section>
  );
}

/** Isi kategori "Provider & model" di Pengaturan. */
export function ModelSettings() {
  const [managing, setManaging] = useState<string | null>(null);
  const [connection, setConnection] = useState<{ editing: ProviderView | null } | null>(null);
  if (connection)
    return (
      <ConnectionPanel
        editing={connection.editing}
        onClose={() => setConnection(null)}
        onSaved={() => setConnection(null)}
      />
    );
  if (managing) return <ModelManager providerId={managing} onBack={() => setManaging(null)} />;
  return (
    <>
      <DefaultsSection />
      <ProvidersSection onManage={setManaging} onConnect={(editing) => setConnection({ editing })} />
    </>
  );
}
