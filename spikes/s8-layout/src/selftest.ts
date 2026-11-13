// Self-test S8. Dua fase (fase 2 berjalan setelah webview UI di-reload untuk menguji persistensi).
import { MIN, SCALES, type AppApi } from "./main";
import { invoke, listen } from "./tauri";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const frame = () => new Promise((r) => requestAnimationFrame(() => r(null)));
const log = (m: string) => invoke("selftest_log", { msg: `${new Date().toISOString().slice(11, 23)} ${m}` });
const app = (): AppApi => (window as any).__s8;
type Rep = Record<string, any>;

type Bounds = { x: number; y: number; w: number; h: number; scale: number; visible: boolean };
async function boundsDiff() {
  const r = app().browserHost()!.getBoundingClientRect();
  const b = await invoke<Bounds>("browser_get_bounds");
  return { diff: Math.max(Math.abs(r.left - b.x), Math.abs(r.top - b.y), Math.abs(r.width - b.w), Math.abs(r.height - b.h)), host: [r.left, r.top, r.width, r.height].map(Math.round), b };
}
async function boundsOk(label: string) {
  await sleep(150);
  const d = await boundsDiff();
  if (!d.b.visible) throw new Error(`${label}: browser tersembunyi`);
  if (d.diff > 2) throw new Error(`${label}: bounds selisih ${d.diff.toFixed(1)}px host=${d.host} webview=${[d.b.x, d.b.y, d.b.w, d.b.h].map(Math.round)}`);
  return +d.diff.toFixed(2);
}
function overflow() {
  const de = document.documentElement;
  const top = document.querySelector(".topbar") as HTMLElement;
  return { page: de.scrollWidth > innerWidth + 1 || de.scrollHeight > innerHeight + 1, topbar: top.scrollWidth > top.clientWidth + 1 };
}
const remPx = () => parseFloat(getComputedStyle(document.documentElement).fontSize);
function widths() {
  const out: Record<string, number> = {};
  for (const k of ["p1", "p2", "p3", "p4"] as const) out[k] = Math.round(app().panels[k].current!.getSize().inPixels);
  return out;
}
const collapsed = () => (["p1", "p3", "p4"] as const).filter((k) => app().panels[k].current!.isCollapsed());

let probeResolve: ((v: string) => void) | null = null;
listen<string>("browser-probe", (v) => probeResolve?.(v));
const probe = (expr: string) =>
  new Promise<string>((res, rej) => { probeResolve = res; invoke("browser_probe", { expr }); setTimeout(() => rej(new Error("timeout probe")), 8000); });
let lastLoad = "";
listen<[string, boolean]>("browser-load", ([u, fin]) => { if (fin) lastLoad = u; });

async function check(r: Rep, name: string, fn: () => Promise<Rep | void>) {
  log("mulai " + name);
  try { r[name] = { ok: true, ...((await fn()) ?? {}) }; } catch (e) { r[name] = { ok: false, error: String(e).slice(0, 400) }; }
  log(`selesai ${name}: ${JSON.stringify(r[name]).slice(0, 300)}`);
}

/** Pantau jeda antar-frame & long task selama `fn`. */
async function measure<T>(fn: () => Promise<T>) {
  let maxGap = 0, last = performance.now(), run = true, frames = 0;
  const longTasks: number[] = [];
  const po = new PerformanceObserver((l) => l.getEntries().forEach((e) => longTasks.push(Math.round(e.duration))));
  try { po.observe({ type: "longtask", buffered: false }); } catch {}
  (function tick() { const n = performance.now(); maxGap = Math.max(maxGap, n - last); last = n; frames++; if (run) requestAnimationFrame(tick); })();
  const t0 = performance.now();
  const value = await fn();
  run = false; po.disconnect();
  return { value, ms: Math.round(performance.now() - t0), frames, maxFrameGapMs: Math.round(maxGap), longTasks };
}

async function phase1() {
  const r: Rep = { startedAt: new Date().toISOString() };
  await invoke("window_set_size", { w: 1366, h: 768 });
  app().setScale(1);
  await sleep(500);
  const t0 = performance.now();
  while (!lastLoad.includes("google.") && performance.now() - t0 < 30000) await sleep(100);

  await check(r, "initial_1366x768", async () => {
    // Layout bawaan eksplisit: layout/skala tersimpan dari run sebelumnya bisa sudah dipulihkan saat mount.
    for (const k of ["p1", "p3", "p4"] as const) app().panels[k].current!.expand();
    app().group.current!.setLayout({ p1: 18, p2: 34, p3: 22, p4: 26 });
    await sleep(300);
    const w = widths();
    if (collapsed().length)
      throw new Error(`panel terlipat saat awal: ${collapsed()} · scale=${app().scale()} rem=${remPx()} group=${Math.round(document.querySelector(".group")!.getBoundingClientRect().width)} win=${innerWidth} layout=${JSON.stringify(app().group.current!.getLayout())} auto=${app().autoCollapsed}`);
    if (w.p2 < MIN.p2 * remPx() - 1) throw new Error("chat di bawah minimum " + JSON.stringify(w));
    const o = overflow(); if (o.page || o.topbar) throw new Error("overflow " + JSON.stringify(o));
    return { widths: w, boundsDiff: await boundsOk("awal"), googleLoaded: lastLoad };
  });

  await check(r, "drag_simulated_60_steps", async () => {
    const g = app().group.current!;
    const start = g.getLayout();
    let lagFrames = 0;
    const m = await measure(async () => {
      for (let i = 0; i <= 60; i++) {
        const t = i / 60;
        g.setLayout({ p1: start.p1, p2: start.p2 - 6 * t, p3: start.p3 + 6 * t, p4: start.p4 });
        await frame();
        if ((await boundsDiff()).diff > 2) lagFrames++;
      }
    });
    const settled = await boundsOk("setelah drag");
    if (m.maxFrameGapMs > 50) throw new Error("jank: jeda frame " + m.maxFrameGapMs + "ms");
    return { ...m, value: undefined, stepsWithBoundsLagAfter1Frame: lagFrames, settledDiff: settled };
  });

  await check(r, "drag_pointer_separator", async () => {
    const sep = document.querySelector('[data-separator][id="s3"], #s3') as HTMLElement;
    if (!sep) throw new Error("separator s3 tidak ditemukan");
    const before = widths().p4;
    const rect = sep.getBoundingClientRect();
    const x0 = rect.left + rect.width / 2, y = rect.top + rect.height / 2;
    const ev = (type: string, x: number) => new PointerEvent(type, { bubbles: true, clientX: x, clientY: y, pointerId: 1, pointerType: "mouse", isPrimary: true, button: 0, buttons: type === "pointerup" ? 0 : 1 });
    const m = await measure(async () => {
      sep.dispatchEvent(ev("pointerdown", x0));
      for (let i = 1; i <= 20; i++) { document.dispatchEvent(ev("pointermove", x0 - i * 5)); await frame(); }
      document.dispatchEvent(ev("pointerup", x0 - 100));
    });
    await sleep(100);
    const after = widths().p4;
    if (Math.abs(after - before - 100) > 15) throw new Error(`lebar P4 ${before}→${after}, diharapkan +100`);
    return { before, after, maxFrameGapMs: m.maxFrameGapMs, boundsDiff: await boundsOk("drag pointer") };
  });

  await check(r, "collapse_expand", async () => {
    const res: Rep = {};
    for (const k of ["p1", "p3", "p4"] as const) {
      app().panels[k].current!.collapse(); await sleep(250);
      if (k === "p4") {
        await app().syncBounds();
        const b = await invoke<Bounds>("browser_get_bounds");
        if (b.visible) throw new Error("browser masih tampil saat P4 terlipat");
      } else res[`${k}_collapsed_diff`] = await boundsOk(`${k} terlipat`);
      app().panels[k].current!.expand(); await sleep(250);
      res[`${k}_expanded_diff`] = await boundsOk(`${k} dibuka`);
    }
    return res;
  });

  await check(r, "overlay_hides_browser", async () => {
    app().setOverlay(true); await sleep(250);
    const hidden = !(await invoke<Bounds>("browser_get_bounds")).visible;
    app().setOverlay(false); await sleep(250);
    if (!hidden) throw new Error("browser tidak disembunyikan saat overlay");
    return { boundsAfter: await boundsOk("setelah overlay") };
  });

  const baseDpr = Number(await probe("window.devicePixelRatio"));
  for (const s of SCALES) {
    await check(r, `zoom_${Math.round(s * 100)}`, async () => {
      for (const k of ["p1", "p3", "p4"] as const) app().panels[k].current!.expand();
      app().setScale(s); await sleep(400);
      const o = overflow();
      const top = (document.querySelector(".topbar") as HTMLElement).getBoundingClientRect().height;
      const w = widths(), c = collapsed();
      const violations = (["p1", "p2", "p3", "p4"] as const).filter((k) => !c.includes(k as any) && w[k] < MIN[k] * remPx() - 1);
      const diff = c.includes("p4") ? null : await boundsOk(`zoom ${s}`);
      const inner = c.includes("p4") ? null : Number(await probe("window.innerWidth"));
      const dpr = c.includes("p4") ? null : Number(await probe("window.devicePixelRatio"));
      const hostW = app().browserHost()!.getBoundingClientRect().width;
      if (o.page || o.topbar) throw new Error("overflow " + JSON.stringify(o));
      if (Math.abs(top - 2.5 * 16 * s) > 1) throw new Error(`tinggi topbar ${top} ≠ ${2.5 * 16 * s}`);
      if (violations.length) throw new Error("min width dilanggar: " + violations + " " + JSON.stringify(w));
      if (inner !== null && Math.abs(inner - hostW) > 2) throw new Error(`browser innerWidth ${inner} ≠ host ${hostW} (konten browser ikut ter-zoom?)`);
      if (dpr !== null && dpr !== baseDpr) throw new Error(`devicePixelRatio browser berubah ${baseDpr}→${dpr}`);
      return { widths: w, collapsed: c, topbar: top, boundsDiff: diff, browserInnerWidth: inner, browserDpr: dpr };
    });
  }

  await check(r, "auto_collapse_small_window_150", async () => {
    app().autoCollapsed.length = 0;
    for (const k of ["p1", "p3", "p4"] as const) app().panels[k].current!.expand();
    app().setScale(1.5);
    await invoke("window_set_size", { w: 1000, h: 650 }); await sleep(700);
    const w = widths(), c = collapsed();
    if (w.p2 < MIN.p2 * remPx() - 1) throw new Error("chat di bawah minimum " + JSON.stringify(w));
    const o = overflow(); if (o.page || o.topbar) throw new Error("overflow " + JSON.stringify(o));
    return { widths: w, collapsed: c, order: [...app().autoCollapsed] };
  });

  await check(r, "laptop_1366_at_125_all_open", async () => {
    await invoke("window_set_size", { w: 1366, h: 768 }); app().setScale(1.25); await sleep(500);
    for (const k of ["p1", "p3", "p4"] as const) app().panels[k].current!.expand();
    await sleep(500);
    return { widths: widths(), collapsed: collapsed(), fitsAllOpen: collapsed().length === 0 };
  });

  // Persiapan fase 2: tata letak & skala tertentu, lalu reload webview UI.
  app().setScale(1.25);
  for (const k of ["p1", "p3", "p4"] as const) app().panels[k].current!.expand();
  await sleep(300);
  const want = app().group.current!.setLayout({ p1: 17, p2: 38, p3: 20, p4: 25 });
  await sleep(500);
  localStorage.setItem("s8-expected", JSON.stringify(want));
  localStorage.setItem("s8-report", JSON.stringify(r));
  localStorage.setItem("s8-phase", "2");
  log("fase 1 selesai, reload UI");
  await invoke("ui_reload");
}

async function phase2() {
  const r: Rep = JSON.parse(localStorage.getItem("s8-report") || "{}");
  const want = JSON.parse(localStorage.getItem("s8-expected") || "{}");
  localStorage.removeItem("s8-phase");
  await sleep(800);
  await check(r, "layout_and_zoom_persist_after_reload", async () => {
    const got = app().group.current!.getLayout();
    const maxDiff = Math.max(...Object.keys(want).map((k) => Math.abs(want[k] - got[k])));
    if (maxDiff > 0.6) throw new Error(`layout ${JSON.stringify(got)} ≠ ${JSON.stringify(want)}`);
    if (app().scale() !== 1.25) throw new Error("skala tidak tersimpan: " + app().scale());
    return { want, got, maxDiff: +maxDiff.toFixed(2), boundsDiff: await boundsOk("setelah reload") };
  });
  r.finishedAt = new Date().toISOString();
  for (const k of Object.keys(localStorage)) if (k.startsWith("s8-") || k.startsWith("zandor-s8") || k.startsWith("react-resizable-panels")) localStorage.removeItem(k);
  await invoke("selftest_done", { report: r });
}

export async function runSelftest() {
  const phase = localStorage.getItem("s8-phase");
  if (phase === "2") return phase2();
  if (phase !== "1") {
    // Fase 0: mulai dari penyimpanan bersih.
    for (const k of Object.keys(localStorage)) if (k.startsWith("zandor-s8") || k.startsWith("s8-") || k.startsWith("react-resizable-panels")) localStorage.removeItem(k);
    localStorage.setItem("s8-phase", "1");
    return invoke("ui_reload");
  }
  localStorage.removeItem("s8-phase");
  try { await phase1(); } catch (e) { log("FATAL " + e); await invoke("selftest_done", { report: { fatal: String(e) } }); }
}
