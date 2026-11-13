// Halaman Provider & model (Fase 6): koneksi buatan pengguna (endpoint kustom, lokal, kunci provider katalog),
// sign-in lewat engine, ping model, dan status yang jujur. Kunci hanya lewat host (keychain), tidak disimpan di state UI.
import {
  type AuthMethod,
  type CatalogProvider,
  type LocalFound,
  type ProbeKind,
  type ProbeResult,
  type ProviderView,
  pingKey,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  Badge,
  Button,
  Dialog,
  Field,
  IconButton,
  Input,
  PingIcon,
  PlusIcon,
  SearchField,
  Select,
  Spinner,
  Tabs,
  TrashIcon,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { type FormEvent, useMemo, useState } from "react";
import { useEngineState, useProviders, useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function useFail() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
}

export function pingLabel(t: ReturnType<typeof useI18n>["t"], r: ProbeResult): string {
  if (r.ok) return t("pv.ping.ok", { ms: r.latencyMs });
  const kind: ProbeKind = r.kind;
  return `${t(`pv.ping.${kind}` as never)}${r.error ? ` — ${r.error}` : ""}`;
}

export function ProvidersPage() {
  const { t } = useI18n();
  const { nav, providers: store } = useServices();
  const state = useProviders();
  const engine = useEngineState();
  const confirm = useConfirm();
  const fail = useFail();
  const [dialog, setDialog] = useState<{ open: boolean; editing: ProviderView | null }>({ open: false, editing: null });

  const remove = async (p: ProviderView) => {
    const ok = await confirm({
      title: t("pv.confirmDeleteTitle", { name: p.name }),
      description: t("pv.confirmDeleteBody"),
      confirmLabel: t("pv.delete"),
      tone: "danger",
    });
    if (ok) await store.remove(p.id).catch(fail);
  };

  return (
    <main className="providers">
      <header className="providers__head">
        <IconButton label={t("pv.back")} icon={<ArrowLeftIcon />} onClick={() => nav.push("/")} />
        <h1 className="z-h3 providers__title">{t("pv.title")}</h1>
        <Button variant="solid" onClick={() => setDialog({ open: true, editing: null })}>
          <PlusIcon size="sm" /> {t("pv.add")}
        </Button>
      </header>
      <p className="z-small providers__secure">{store.secure ? t("pv.secure.yes") : t("pv.secure.no")}</p>
      {state.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {state.error}
        </p>
      )}
      <h2 className="z-h4">{t("pv.list")}</h2>
      {state.loaded && state.providers.length === 0 && <p className="z-p providers__empty">{t("pv.empty")}</p>}
      <ul className="providers__list">
        {state.providers.map((p) => (
          <ProviderCard
            key={p.id}
            provider={p}
            engineReady={engine.status === "ready"}
            onEdit={() => setDialog({ open: true, editing: p })}
            onDelete={() => void remove(p)}
          />
        ))}
      </ul>
      {dialog.open && (
        <ConnectionDialog
          editing={dialog.editing}
          onClose={() => setDialog({ open: false, editing: null })}
          onSaved={() => setDialog({ open: false, editing: null })}
        />
      )}
    </main>
  );
}

function ProviderCard({
  provider: p,
  engineReady,
  onEdit,
  onDelete,
}: {
  provider: ProviderView;
  engineReady: boolean;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const state = useProviders();
  const connected = state.connected.includes(p.id);
  const canPing = p.kind !== "builtin";
  return (
    <li className="pcard" aria-label={p.name}>
      <div className="pcard__top">
        <div className="pcard__titles">
          <h3 className="z-label pcard__name">{p.name}</h3>
          {p.baseUrl && <p className="z-small pcard__url">{p.baseUrl}</p>}
        </div>
        <Badge tone="outline">{t(`pv.kind.${p.kind}` as never)}</Badge>
        <Badge tone={p.hasKey ? "success" : "neutral"}>{p.hasKey ? t("pv.key.saved") : t("pv.key.none")}</Badge>
        <Badge tone={!engineReady ? "warning" : connected ? "success" : "warning"}>
          {!engineReady
            ? t("pv.status.engineDown")
            : connected
              ? t("pv.status.connected")
              : t("pv.status.notConnected")}
        </Badge>
        <Button size="sm" onClick={onEdit}>
          {t("pv.edit")}
        </Button>
        <IconButton label={t("pv.delete")} size="sm" icon={<TrashIcon />} onClick={onDelete} />
      </div>
      {canPing && p.models.length > 0 && (
        <ul className="pcard__models" aria-label={t("pv.models")}>
          {p.models.map((m) => {
            const ping = state.pings[pingKey(p.id, m.id)];
            return (
              <li key={m.id} className="pmodel">
                <span className="pmodel__name">{m.name ?? m.id}</span>
                <span className="pmodel__status z-small" role="status">
                  {ping?.status === "pending" && (
                    <>
                      <Spinner size="sm" /> {t("pv.pinging")}
                    </>
                  )}
                  {ping?.status === "done" && (
                    <span data-tone={ping.result.ok ? "ok" : "bad"}>{pingLabel(t, ping.result)}</span>
                  )}
                </span>
                <IconButton
                  label={t("pv.ping", { name: m.name ?? m.id })}
                  size="sm"
                  icon={<PingIcon />}
                  disabled={ping?.status === "pending"}
                  onClick={() => void store.ping(p.id, m.id)}
                />
              </li>
            );
          })}
        </ul>
      )}
    </li>
  );
}

// ---------------- dialog tambah/ubah ----------------

function ConnectionDialog({
  editing,
  onClose,
  onSaved,
}: {
  editing: ProviderView | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useI18n();
  const [tab, setTab] = useState(editing?.kind === "builtin" ? "catalog" : "custom");
  const done = onSaved;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={editing ? t("pv.dialog.editTitle") : t("pv.dialog.title")}
      footer={<Button onClick={onClose}>{t("ui.close")}</Button>}
    >
      {editing && editing.kind !== "builtin" ? (
        <EndpointForm editing={editing} kind={editing.kind} onSaved={done} />
      ) : (
        <Tabs
          aria-label={t("pv.dialog.title")}
          value={tab}
          onValueChange={setTab}
          items={[
            {
              value: "custom",
              label: t("pv.tab.custom"),
              content: <EndpointForm editing={null} kind="custom" onSaved={done} />,
            },
            { value: "local", label: t("pv.tab.local"), content: <LocalTab onSaved={done} /> },
            { value: "catalog", label: t("pv.tab.catalog"), content: <CatalogTab editing={editing} onSaved={done} /> },
            { value: "signin", label: t("pv.tab.signin"), content: <SignInTab onSaved={done} /> },
          ]}
        />
      )}
    </Dialog>
  );
}

function EndpointForm({
  editing,
  kind,
  initial,
  onSaved,
}: {
  editing: ProviderView | null;
  kind: "custom" | "local";
  initial?: { name: string; baseUrl: string; models: string[] };
  onSaved: () => void;
}) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const toast = useToast();
  const fail = useFail();
  const [name, setName] = useState(editing?.name ?? initial?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(editing?.baseUrl ?? initial?.baseUrl ?? "");
  const [apiKey, setApiKey] = useState("");
  const [known, setKnown] = useState<string[]>(() => [
    ...new Set([...(editing?.models.map((m) => m.id) ?? []), ...(initial?.models ?? [])]),
  ]);
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(editing?.models.map((m) => m.id) ?? initial?.models ?? []),
  );
  const [filter, setFilter] = useState("");
  const [manual, setManual] = useState("");
  const [probing, setProbing] = useState(false);
  const [probeMsg, setProbeMsg] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const fetchModels = async () => {
    setProbing(true);
    setProbeMsg(null);
    const r = await store.probe(baseUrl, apiKey || undefined, editing?.id);
    setProbing(false);
    if (!r.ok) {
      setProbeMsg(t("pv.probeFailed", { error: r.error ?? "?" }));
      return;
    }
    setKnown((prev) => [...new Set([...prev, ...r.models])].sort());
    setProbeMsg(t("pv.modelsFound", { count: r.models.length }));
  };
  const addManual = () => {
    const id = manual.trim();
    if (!id) return;
    setKnown((prev) => (prev.includes(id) ? prev : [...prev, id].sort()));
    setSelected((prev) => new Set(prev).add(id));
    setManual("");
  };
  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const shown = known.filter((m) => m.toLowerCase().includes(filter.trim().toLowerCase()));

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const names = new Map(editing?.models.map((m) => [m.id, m.name]) ?? []);
      await store.saveEndpoint({
        ...(editing ? { id: editing.id } : {}),
        name,
        kind,
        baseUrl,
        ...(apiKey ? { apiKey } : {}),
        models: [...selected].map((id) => ({ id, ...(names.get(id) ? { name: names.get(id) as string } : {}) })),
      });
      toast.show({ title: t("pv.saved"), tone: "success" });
      onSaved();
    } catch (err) {
      fail(err);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form className="pform" onSubmit={(e) => void submit(e)}>
      <Field label={t("pv.name")}>
        {(c) => <Input {...c} value={name} onChange={(e) => setName(e.target.value)} />}
      </Field>
      <Field label={t("pv.baseUrl")}>
        {(c) => (
          <Input
            {...c}
            value={baseUrl}
            placeholder="http://localhost:8045/v1"
            onChange={(e) => setBaseUrl(e.target.value)}
          />
        )}
      </Field>
      <Field
        label={t("pv.apiKey")}
        description={editing?.hasKey ? t("pv.apiKeyKeep") : kind === "local" ? t("pv.apiKeyOptional") : undefined}
      >
        {(c) => (
          <Input {...c} type="password" autoComplete="off" value={apiKey} onChange={(e) => setApiKey(e.target.value)} />
        )}
      </Field>
      <div className="pform__row">
        <Button onClick={() => void fetchModels()} disabled={probing || !baseUrl.trim()}>
          {probing ? t("pv.fetching") : t("pv.fetchModels")}
        </Button>
        {probeMsg && (
          <span className="z-small" role="status">
            {probeMsg}
          </span>
        )}
      </div>
      {known.length > 8 && (
        <SearchField
          aria-label={t("pv.modelFilter")}
          placeholder={t("pv.modelFilter")}
          value={filter}
          onValueChange={setFilter}
        />
      )}
      {known.length > 0 && (
        <ul className="pform__models" aria-label={t("pv.models")}>
          {shown.map((m) => (
            <li key={m}>
              <label className="pform__check">
                <input type="checkbox" checked={selected.has(m)} onChange={() => toggle(m)} />
                <span>{m}</span>
              </label>
            </li>
          ))}
        </ul>
      )}
      <div className="pform__row">
        <Input
          aria-label={t("pv.manualModelId")}
          placeholder={t("pv.manualModelId")}
          value={manual}
          onChange={(e) => setManual(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addManual();
            }
          }}
        />
        <Button onClick={addManual} disabled={!manual.trim()}>
          {t("pv.addModel")}
        </Button>
      </div>
      <p className="z-small">{t("pv.selected", { count: selected.size })}</p>
      <div className="pform__row pform__row--end">
        <Button variant="solid" type="submit" disabled={saving}>
          {t("pv.save")}
        </Button>
      </div>
    </form>
  );
}

function LocalTab({ onSaved }: { onSaved: () => void }) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const fail = useFail();
  const toast = useToast();
  const [found, setFound] = useState<LocalFound[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [manual, setManual] = useState<LocalFound | null>(null);

  const detect = async () => {
    setBusy(true);
    setFound(await store.detectLocal());
    setBusy(false);
  };
  const add = async (f: LocalFound) => {
    try {
      await store.saveEndpoint({
        name: f.name,
        kind: "local",
        baseUrl: f.baseUrl,
        models: f.models.map((id) => ({ id })),
      });
      toast.show({ title: t("pv.saved"), tone: "success" });
      onSaved();
    } catch (e) {
      fail(e);
    }
  };
  if (manual) {
    return (
      <EndpointForm
        editing={null}
        kind="local"
        initial={{ name: manual.name, baseUrl: manual.baseUrl, models: [] }}
        onSaved={onSaved}
      />
    );
  }
  return (
    <div className="pform">
      <div className="pform__row">
        <Button variant="solid" onClick={() => void detect()} disabled={busy}>
          {busy ? t("pv.local.detecting") : t("pv.local.detect")}
        </Button>
      </div>
      {found && found.length === 0 && <p className="z-small">{t("pv.local.none")}</p>}
      <ul className="plist">
        {(found ?? []).map((f) => (
          <li key={f.id} className="plist__row">
            <span>{t("pv.local.found", { name: f.name, count: f.models.length })}</span>
            <Button size="sm" variant="solid" onClick={() => void add(f)}>
              {t("pv.local.add", { name: f.name })}
            </Button>
          </li>
        ))}
      </ul>
      <div className="pform__row">
        {[
          { id: "ollama", name: "Ollama", baseUrl: "http://localhost:11434/v1", models: [] },
          { id: "lmstudio", name: "LM Studio", baseUrl: "http://localhost:1234/v1", models: [] },
        ].map((preset) => (
          <Button key={preset.id} size="sm" onClick={() => setManual(preset)}>
            {preset.name}
          </Button>
        ))}
      </div>
    </div>
  );
}

/** Provider katalog yang cukup satu kunci API sederhana (satu env var, tanpa prompt tambahan pada metode API). */
export function isSimpleKeyProvider(p: CatalogProvider, methods: AuthMethod[] | undefined): boolean {
  if (p.env.length !== 1) return false;
  const apiPrompts = (methods ?? []).filter((m) => m.type === "api").some((m) => m.prompts.length > 0);
  return !apiPrompts;
}

function CatalogTab({ editing, onSaved }: { editing: ProviderView | null; onSaved: () => void }) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const state = useProviders();
  const engine = useEngineState();
  const fail = useFail();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<string | null>(editing?.id ?? null);
  const [key, setKey] = useState("");

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    const managed = new Set(state.providers.map((p) => p.id));
    return state.catalog
      .filter((p) => (editing ? p.id === editing.id : !managed.has(p.id)))
      .filter((p) => !q || p.name.toLowerCase().includes(q) || p.id.includes(q))
      .slice(0, 80);
  }, [state.catalog, state.providers, query, editing]);

  if (engine.status !== "ready") return <p className="z-small">{t("pv.catalog.engineDown")}</p>;
  if (state.catalogStatus === "loading" && state.catalog.length === 0)
    return <p className="z-small">{t("pv.catalog.loading")}</p>;
  if (state.catalogStatus === "error")
    return (
      <p className="chat__note chat__note--error z-small">
        {t("pv.catalog.error", { error: state.catalogError ?? "" })}
      </p>
    );

  const connect = async (p: CatalogProvider) => {
    try {
      await store.saveBuiltin({ id: p.id, name: p.name, apiKey: key });
      toast.show({ title: t("pv.saved"), tone: "success" });
      onSaved();
    } catch (e) {
      fail(e);
    }
  };

  return (
    <div className="pform">
      {!editing && (
        <SearchField
          aria-label={t("pv.catalog.search")}
          placeholder={t("pv.catalog.search")}
          value={query}
          onValueChange={setQuery}
        />
      )}
      {list.length === 0 && <p className="z-small">{t("pv.catalog.none")}</p>}
      <ul className="plist">
        {list.map((p) => {
          const simple = isSimpleKeyProvider(p, state.authMethods[p.id]);
          const open = picked === p.id;
          return (
            <li key={p.id} className="plist__row plist__row--col">
              <div className="plist__line">
                <span className="plist__name">{p.name}</span>
                <span className="z-small">{t("pv.catalog.models", { count: p.modelCount })}</span>
                {p.connected && <Badge tone="success">{t("pv.status.connected")}</Badge>}
                {simple ? (
                  <Button size="sm" onClick={() => setPicked(open ? null : p.id)} aria-expanded={open}>
                    {t("pv.catalog.connect")} {p.name}
                  </Button>
                ) : (
                  <span className="z-small">{t("pv.catalog.needsMore")}</span>
                )}
              </div>
              {open && simple && (
                <form
                  className="pform__row"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void connect(p);
                  }}
                >
                  <Input
                    type="password"
                    autoComplete="off"
                    aria-label={`${t("pv.apiKey")} ${p.name}`}
                    placeholder={p.env[0]}
                    value={key}
                    onChange={(e) => setKey(e.target.value)}
                  />
                  <Button variant="solid" type="submit" disabled={!key.trim()}>
                    {t("pv.save")}
                  </Button>
                </form>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

const OAUTH_WAIT_MS = 5 * 60_000;

function SignInTab({ onSaved }: { onSaved: () => void }) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const state = useProviders();
  const engine = useEngineState();
  const toast = useToast();
  const [flow, setFlow] = useState<{
    providerId: string;
    name: string;
    method: number;
    stage: "inputs" | "waiting" | "code";
    inputs: Record<string, string>;
    instructions?: string;
  } | null>(null);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);

  const entries = useMemo(() => {
    const names = new Map(state.catalog.map((p) => [p.id, p.name]));
    return Object.entries(state.authMethods)
      .map(([id, methods]) => ({
        id,
        name: names.get(id) ?? id,
        methods: methods.map((m, index) => ({ ...m, index })).filter((m) => m.type === "oauth"),
      }))
      .filter((e) => e.methods.length > 0);
  }, [state.authMethods, state.catalog]);

  if (engine.status !== "ready") return <p className="z-small">{t("pv.catalog.engineDown")}</p>;

  const finish = async (providerId: string, name: string, method: number, withCode?: string) => {
    try {
      await Promise.race([
        store.finishOAuth(providerId, method, withCode),
        new Promise((_, rej) => setTimeout(() => rej(new Error("waktu habis menunggu login")), OAUTH_WAIT_MS)),
      ]);
      toast.show({ title: t("pv.signin.done", { name }), tone: "success" });
      setFlow(null);
      onSaved();
    } catch (e) {
      setError(t("pv.signin.failed", { error: errText(e) }));
      setFlow(null);
    }
  };

  const start = async (providerId: string, name: string, method: number, inputs: Record<string, string>) => {
    setError(null);
    try {
      const s = await store.startOAuth(providerId, method, Object.keys(inputs).length ? inputs : undefined);
      if (s.method === "code") {
        setFlow({ providerId, name, method, stage: "code", inputs, instructions: s.instructions });
      } else {
        setFlow({ providerId, name, method, stage: "waiting", inputs, instructions: s.instructions });
        void finish(providerId, name, method);
      }
    } catch (e) {
      setError(t("pv.signin.failed", { error: errText(e) }));
      setFlow(null);
    }
  };

  return (
    <div className="pform">
      <p className="z-small">{t("pv.signin.note")}</p>
      {error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {error}
        </p>
      )}
      {flow?.stage === "waiting" && (
        <p className="chat__note z-small" role="status">
          <Spinner size="sm" /> {t("pv.signin.waiting")} {flow.instructions}
        </p>
      )}
      {flow?.stage === "code" && (
        <form
          className="pform__row"
          onSubmit={(e) => {
            e.preventDefault();
            void finish(flow.providerId, flow.name, flow.method, code.trim());
          }}
        >
          <p className="z-small">{flow.instructions}</p>
          <Input aria-label={t("pv.signin.code")} value={code} onChange={(e) => setCode(e.target.value)} />
          <Button variant="solid" type="submit" disabled={!code.trim()}>
            {t("pv.signin.submit")}
          </Button>
        </form>
      )}
      {entries.length === 0 && state.catalogStatus === "ready" && <p className="z-small">{t("pv.signin.none")}</p>}
      <ul className="plist">
        {entries.map((e) => (
          <li key={e.id} className="plist__row plist__row--col">
            <span className="plist__name">{e.name}</span>
            {e.methods.map((m) => (
              <SignInMethod
                key={m.index}
                method={m}
                disabled={flow !== null}
                providerName={e.name}
                onStart={(inputs) => void start(e.id, e.name, m.index, inputs)}
              />
            ))}
          </li>
        ))}
      </ul>
    </div>
  );
}

function SignInMethod({
  method,
  providerName,
  disabled,
  onStart,
}: {
  method: AuthMethod & { index: number };
  providerName: string;
  disabled: boolean;
  onStart: (inputs: Record<string, string>) => void;
}) {
  const { t } = useI18n();
  const [inputs, setInputs] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      method.prompts.filter((p) => p.type === "select").map((p) => [p.key, p.options?.[0]?.value ?? ""]),
    ),
  );
  const visible = method.prompts.filter((p) => {
    const w = (p as { when?: { key: string; op: string; value: string } }).when;
    return !w || (w.op === "eq" ? inputs[w.key] === w.value : inputs[w.key] !== w.value);
  });
  return (
    <div className="pform__row">
      {visible.map((p) =>
        p.type === "select" ? (
          <Select
            key={p.key}
            aria-label={p.message}
            size="sm"
            items={(p.options ?? []).map((o) => ({ value: o.value, label: o.label }))}
            value={inputs[p.key] ?? null}
            onValueChange={(v) => setInputs((prev) => ({ ...prev, [p.key]: v }))}
          />
        ) : (
          <Input
            key={p.key}
            aria-label={p.message}
            placeholder={p.placeholder ?? p.message}
            value={inputs[p.key] ?? ""}
            onChange={(e) => setInputs((prev) => ({ ...prev, [p.key]: e.target.value }))}
          />
        ),
      )}
      <Button size="sm" variant="solid" disabled={disabled} onClick={() => onStart(inputs)}>
        {t("pv.signin.start")} · {method.label}
        <span className="visually-hidden"> {providerName}</span>
      </Button>
    </div>
  );
}
