// Spike S5 — benchmark renderer Panel 3 dengan file ±10 MB.
// Setiap kasus: waktu parse/render, jeda frame maks & long task (seberapa lama UI membeku),
// heap JS (Chromium), dan cek isi (teks yang diharapkan benar-benar ter-render).
import * as pdfjs from "pdfjs-dist";
import { renderAsync as docxRender } from "docx-preview";
import mammoth from "mammoth";
import * as XLSX from "xlsx";
import { PptxViewer } from "@aiden0z/pptx-renderer";
import { init as pptxPreviewInit } from "pptx-preview";

pdfjs.GlobalWorkerOptions.workerSrc = "/pdf.worker.min.mjs";

type Result = Record<string, unknown>;
const results: Record<string, Result> = {};
const stage = document.getElementById("stage")!;
const status = document.getElementById("status")!;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const heap = () => ((performance as any).memory?.usedJSHeapSize ?? 0) / 1048576;

async function measure(name: string, file: string, fn: (buf: ArrayBuffer, el: HTMLElement) => Promise<Result>) {
  status.textContent = "menjalankan " + name;
  stage.innerHTML = "";
  const el = document.createElement("div");
  el.className = "doc";
  stage.appendChild(el);
  await sleep(300);
  const buf = await (await fetch("/fixtures/" + file)).arrayBuffer();
  let maxGap = 0, last = performance.now(), run = true;
  const longTasks: number[] = [];
  const po = new PerformanceObserver((l) => l.getEntries().forEach((e) => longTasks.push(Math.round(e.duration))));
  try { po.observe({ type: "longtask", buffered: false }); } catch {}
  (function tick() { const n = performance.now(); maxGap = Math.max(maxGap, n - last); last = n; if (run) requestAnimationFrame(tick); })();
  let wasHidden = document.hidden;
  const onVis = () => { if (document.hidden) wasHidden = true; };
  document.addEventListener("visibilitychange", onVis);
  const h0 = heap();
  const t0 = performance.now();
  try {
    const extra = await fn(buf, el);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    results[name] = {
      ok: true,
      fileMB: +(buf.byteLength / 1048576).toFixed(2),
      totalMs: Math.round(performance.now() - t0),
      maxFrameGapMs: Math.round(maxGap),
      tabHiddenDuringRun: wasHidden,
      longestTaskMs: longTasks.length ? Math.max(...longTasks) : 0,
      longTasksOver200ms: longTasks.filter((x) => x > 200).length,
      heapDeltaMB: +(heap() - h0).toFixed(1),
      domNodes: el.getElementsByTagName("*").length,
      ...extra,
    };
  } catch (e) {
    results[name] = { ok: false, error: String((e as Error)?.stack ?? e).slice(0, 500) };
  } finally {
    run = false;
    po.disconnect();
    document.removeEventListener("visibilitychange", onVis);
  }
  console.log(name, results[name]);
}

const text = (el: HTMLElement) => el.innerText || el.textContent || "";

async function main() {
  await measure("pdf_pdfjs", "sample.pdf", async (buf, el) => {
    const t0 = performance.now();
    const doc = await pdfjs.getDocument({ data: new Uint8Array(buf) }).promise;
    const openMs = Math.round(performance.now() - t0);
    const renderPage = async (n: number) => {
      const page = await doc.getPage(n);
      const vp = page.getViewport({ scale: 1.5 });
      const c = document.createElement("canvas");
      c.width = vp.width; c.height = vp.height; el.appendChild(c);
      const s = performance.now();
      await page.render({ canvas: c, viewport: vp }).promise;
      return Math.round(performance.now() - s);
    };
    const firstPageMs = await renderPage(1);
    const page200Ms = await renderPage(200);
    const tc = await (await doc.getPage(1)).getTextContent();
    const pageText = tc.items.map((i: any) => i.str).join(" ");
    if (!pageText.includes("Halaman 1")) throw new Error("teks halaman 1 tidak ditemukan");
    return { pages: doc.numPages, openMs, firstPageMs, page200Ms, textOk: true, note: "render per halaman (lazy), parsing di worker" };
  });

  await measure("docx_docx-preview", "sample.docx", async (buf, el) => {
    await docxRender(new Blob([buf]), el, undefined, { inWrapper: true, breakPages: true, ignoreLastRenderedPageBreak: true });
    const t = text(el);
    if (!t.includes("Bab 1") || !t.includes("Zandor S5")) throw new Error("teks tidak ditemukan");
    return { images: el.querySelectorAll("img").length, tables: el.querySelectorAll("table").length, sections: el.querySelectorAll("section").length };
  });

  await measure("docx_mammoth", "sample.docx", async (buf, el) => {
    const r = await mammoth.convertToHtml({ arrayBuffer: buf });
    el.innerHTML = r.value;
    if (!text(el).includes("Bab 1")) throw new Error("teks tidak ditemukan");
    return { images: el.querySelectorAll("img").length, htmlMB: +(r.value.length / 1048576).toFixed(1), warnings: r.messages.length, note: "semantik saja (tanpa tata letak halaman)" };
  });

  await measure("xlsx_sheetjs", "sample.xlsx", async (buf, el) => {
    const t0 = performance.now();
    const wb = XLSX.read(buf, { dense: true });
    const parseMs = Math.round(performance.now() - t0);
    const ws = wb.Sheets["Data"];
    const range = XLSX.utils.decode_range(ws["!ref"]!);
    // Render hanya 200 baris pertama (produk: tabel tervirtualisasi).
    const s = performance.now();
    const rows = XLSX.utils.sheet_to_json<unknown[]>(ws, { header: 1, range: { s: { r: 0, c: 0 }, e: { r: 199, c: range.e.c } } });
    el.innerHTML = "<table>" + rows.map((r) => "<tr>" + r.map((c) => `<td>${String(c ?? "")}</td>`).join("") + "</tr>").join("") + "</table>";
    if (!text(el).includes("Kategori")) throw new Error("header tidak ditemukan");
    return { sheets: wb.SheetNames.length, rows: range.e.r + 1, cols: range.e.c + 1, parseMs, render200RowsMs: Math.round(performance.now() - s), note: "parse di main thread; produk: Web Worker" };
  });

  await measure("xlsx_sheetjs_worker", "sample.xlsx", async (buf, el) => {
    const w = new Worker("/dist/xlsx-worker.js", { type: "module" });
    const res = await new Promise<any>((resolve, reject) => {
      w.onmessage = (e) => resolve(e.data);
      w.onerror = (e) => reject(new Error(e.message));
      w.postMessage(buf, [buf]);
    });
    w.terminate();
    el.innerHTML = "<table>" + res.rows.map((r: unknown[]) => "<tr>" + r.map((c) => `<td>${String(c ?? "")}</td>`).join("") + "</tr>").join("") + "</table>";
    if (!text(el).includes("Kategori")) throw new Error("header tidak ditemukan");
    return { rows: res.total, cols: res.cols, sheets: res.sheets, parseMsInWorker: res.parseMs, note: "main thread bebas selama parse" };
  });

  await measure("pptx_aiden0z", "sample.pptx", async (buf, el) => {
    el.style.height = "800px"; el.style.overflow = "auto";
    const v = await PptxViewer.open(buf, el, { renderMode: "list", lazySlides: true, lazyMedia: true });
    await sleep(500);
    const t = text(el);
    const ok = t.includes("Slide 1");
    const slides = el.querySelectorAll("[data-slide-index], .pptx-slide, section").length;
    v.destroy?.();
    if (!ok) throw new Error("teks slide 1 tidak ditemukan");
    return { slidesInDom: slides, textOk: ok };
  });

  await measure("pptx_pptx-preview", "sample.pptx", async (buf, el) => {
    const p = pptxPreviewInit(el, { width: 960, height: 540, mode: "list" });
    await p.preview(buf);
    await sleep(1500);
    const t = text(el);
    const textOk = t.includes("Slide 1");
    const diag = { nodes: el.getElementsByTagName("*").length, svg: el.querySelectorAll("svg").length, canvas: el.querySelectorAll("canvas").length, textSample: t.replace(/\s+/g, " ").slice(0, 120) };
    if (!textOk) throw new Error("teks slide 1 tidak ditemukan; DOM: " + JSON.stringify(diag));
    return { imgs: el.querySelectorAll("img").length, textOk, ...diag };
  });

  status.textContent = "selesai";
  (window as any).__results = results;
  await fetch("/report", { method: "POST", body: JSON.stringify({ userAgent: navigator.userAgent, results }, null, 2) });
}

main();
