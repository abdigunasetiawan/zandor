// Pemilih model: popover di atas pemicu dengan beberapa tab.
//  • Model (default): pencarian (langsung terfokus), Terakhir digunakan (3), Disematkan, dan semua provider — semua bagian
//    bisa dilipat; grup provider juga bisa dilipat.
//  • Uji (ping): uji semua/per provider/per model dengan status, waktu respons, dsb; bisa diurutkan dan dikelompokkan.
//  • Satu tab per provider: model milik provider itu, info koneksi, dan uji model-modelnya.
import { type ModelOption, modelKey } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Badge,
  Button,
  ChevronDownIcon,
  ChevronRightIcon,
  ModelLogo,
  PingIcon,
  Popover,
  SearchField,
  Tabs,
} from "@zandor/ui";
import { type ReactNode, useMemo, useRef, useState } from "react";
import { ModelPingTab } from "./ModelPingTab";
import { ModelRow } from "./ModelRow";
import { useChat, useModelPrefs, useProviders, useServices } from "./services";

const matches = (m: ModelOption, q: string) =>
  !q ||
  m.name.toLowerCase().includes(q) ||
  m.providerName.toLowerCase().includes(q) ||
  m.modelID.toLowerCase().includes(q);

/** Bagian yang bisa dilipat (judul + jumlah). */
function Section({
  title,
  count,
  open,
  onToggle,
  children,
}: {
  title: string;
  count?: number;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}) {
  return (
    <section aria-label={title} className="msection">
      <h3 className="z-label msection__head">
        <button type="button" className="msection__toggle" aria-expanded={open} onClick={onToggle}>
          {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
          <span>{title}</span>
          {count !== undefined && <span className="msection__count">({count})</span>}
        </button>
      </h3>
      {open && children}
    </section>
  );
}

export function ModelPicker() {
  const { t } = useI18n();
  const { chat, nav, providers: store } = useServices();
  const s = useChat();
  const pv = useProviders();
  const prefs = useModelPrefs();
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState("models");
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [folded, setFolded] = useState<Set<string>>(new Set());
  const searchRef = useRef<HTMLInputElement>(null);

  const current = s.models.find((m) => s.model && m.providerID === s.model.providerID && m.modelID === s.model.modelID);
  const q = query.trim().toLowerCase();
  // model yang disembunyikan di Pengaturan tidak tampil, kecuali yang sedang dipakai
  const visible = useMemo(
    () =>
      s.models.filter(
        (m) =>
          !prefs.hidden.includes(modelKey(m)) ||
          (s.model?.providerID === m.providerID && s.model?.modelID === m.modelID),
      ),
    [s.models, s.model, prefs.hidden],
  );
  const find = (k: string) => visible.find((m) => modelKey(m) === k);

  const groups = useMemo(() => {
    const byProvider = new Map<string, { name: string; models: ModelOption[] }>();
    for (const m of visible) {
      const g = byProvider.get(m.providerID) ?? { name: m.providerName, models: [] };
      g.models.push(m);
      byProvider.set(m.providerID, g);
    }
    return [...byProvider.entries()].sort((a, b) => a[1].name.localeCompare(b[1].name));
  }, [visible]);
  const recent = s.recent.map(find).filter((m): m is ModelOption => !!m && matches(m, q));
  const pinned = s.pinned.map(find).filter((m): m is ModelOption => !!m && matches(m, q));
  const shown = groups
    .map(([id, g]) => [id, { ...g, models: g.models.filter((m) => matches(m, q)) }] as const)
    .filter(([, g]) => g.models.length > 0);

  if (s.modelsStatus === "error")
    return <Badge tone="danger">{t("chat.modelsError", { error: s.modelsError ?? "" })}</Badge>;
  if (s.models.length === 0) return null;

  const choose = (m: ModelOption) => {
    chat.selectModel({ providerID: m.providerID, modelID: m.modelID });
    setOpen(false);
    setQuery("");
    setTab("models");
  };
  const onOpenChange = (o: boolean) => {
    setOpen(o);
    if (o) {
      if (s.model) setExpanded((prev) => new Set(prev).add(s.model?.providerID ?? ""));
    } else {
      setQuery("");
      setTab("models");
    }
  };
  const flip = (set: Set<string>, id: string) => {
    const next = new Set(set);
    if (!next.delete(id)) next.add(id);
    return next;
  };
  const sectionOpen = (id: string) => !folded.has(id);
  const toggleSection = (id: string) => setFolded((prev) => flip(prev, id));

  const modelsTab = (
    <div className="mpicker__page">
      <SearchField
        aria-label={t("mp.search")}
        placeholder={t("mp.search")}
        value={query}
        onValueChange={setQuery}
        inputRef={searchRef}
      />
      <div className="mpicker__scroll">
        {recent.length === 0 && pinned.length === 0 && shown.length === 0 && <p className="z-small">{t("mp.none")}</p>}
        {recent.length > 0 && (
          <Section
            title={t("mp.recent")}
            count={recent.length}
            open={sectionOpen("recent")}
            onToggle={() => toggleSection("recent")}
          >
            <ul className="mlist">
              {recent.map((m) => (
                <ModelRow key={`recent:${modelKey(m)}`} model={m} showProvider onChoose={choose} />
              ))}
            </ul>
          </Section>
        )}
        {pinned.length > 0 && (
          <Section
            title={t("mp.pinned")}
            count={pinned.length}
            open={sectionOpen("pinned")}
            onToggle={() => toggleSection("pinned")}
          >
            <ul className="mlist">
              {pinned.map((m) => (
                <ModelRow key={`pin:${modelKey(m)}`} model={m} showProvider onChoose={choose} />
              ))}
            </ul>
          </Section>
        )}
        {shown.length > 0 && (
          <Section
            title={t("mp.all")}
            count={shown.length}
            open={sectionOpen("all")}
            onToggle={() => toggleSection("all")}
          >
            {shown.map(([id, g]) => {
              const isOpen = q !== "" || expanded.has(id);
              return (
                <section key={id} aria-label={g.name} className="mgroup">
                  <header className="mgroup__head">
                    <button
                      type="button"
                      className="mgroup__toggle"
                      aria-expanded={isOpen}
                      onClick={() => setExpanded((prev) => flip(prev, id))}
                    >
                      {isOpen ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
                      <ModelLogo providerID={id} name={g.name} />
                      <span className="mgroup__name">{g.name}</span>
                      <span className="mgroup__count z-small">({g.models.length})</span>
                    </button>
                  </header>
                  {isOpen && (
                    <ul className="mlist">
                      {g.models.map((m) => (
                        <ModelRow key={`all:${modelKey(m)}`} model={m} showProvider={false} onChoose={choose} />
                      ))}
                    </ul>
                  )}
                </section>
              );
            })}
          </Section>
        )}
      </div>
    </div>
  );

  const providerTab = (id: string, name: string, models: ModelOption[]) => {
    const spec = pv.providers.find((p) => p.id === id);
    const owned = spec !== undefined && spec.kind !== "builtin";
    const kindLabel = t(`pv.kind.${spec?.kind ?? "builtin"}` as never);
    return (
      <div className="mpicker__page">
        <div className="mprov__head">
          <ModelLogo providerID={id} name={name} />
          <div className="mprov__title">
            <span className="mprov__name">{name}</span>
            <span className="z-small mprov__sub">
              {[kindLabel, spec?.baseUrl, t("mp.provider.models", { n: models.length })].filter(Boolean).join(" · ")}
            </span>
          </div>
          {owned && (
            <Button
              size="sm"
              onClick={() =>
                void (async () => {
                  for (const m of models) await store.ping(m.providerID, m.modelID);
                })()
              }
            >
              <PingIcon size="sm" /> {t("mp.provider.pingAll")}
            </Button>
          )}
        </div>
        <div className="mpicker__scroll">
          <ul className="mlist">
            {models.map((m) => (
              <ModelRow key={modelKey(m)} model={m} showProvider={false} onChoose={choose} />
            ))}
          </ul>
        </div>
      </div>
    );
  };

  const trigger = (
    <Button size="sm" className="chat__model" aria-label={t("mp.open", { name: current?.name ?? "" })}>
      <ModelLogo providerID={current?.providerID} modelID={current?.modelID} name={current?.providerName} />
      <span className="chat__model-name">{current?.name ?? t("chat.model")}</span>
    </Button>
  );

  return (
    <Popover
      open={open}
      onOpenChange={onOpenChange}
      trigger={trigger}
      title={t("mp.title")}
      hideTitle
      side="top"
      align="end"
      initialFocus={searchRef}
      className="mpicker-pop"
    >
      <div className="mpicker">
        <Tabs
          aria-label={t("mp.title")}
          value={tab}
          onValueChange={setTab}
          items={[
            { value: "models", label: t("mp.tab.models"), content: modelsTab },
            { value: "ping", label: t("mp.tab.ping"), icon: <PingIcon size="sm" />, content: <ModelPingTab /> },
            ...groups.map(([id, g]) => ({
              value: `p:${id}`,
              label: g.name,
              icon: <ModelLogo providerID={id} name={g.name} />,
              content: providerTab(id, g.name, g.models),
            })),
          ]}
        />
        <div className="mpicker__foot">
          <Button
            size="sm"
            onClick={() => {
              setOpen(false);
              nav.push("/settings/models");
            }}
          >
            {t("mp.manage")}
          </Button>
        </div>
      </div>
    </Popover>
  );
}
