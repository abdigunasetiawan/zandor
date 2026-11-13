// Web Worker: parse XLSX di luar main thread, kirim balik dimensi + 200 baris pertama.
import * as XLSX from "xlsx";

self.onmessage = (e: MessageEvent<ArrayBuffer>) => {
  const t0 = performance.now();
  const wb = XLSX.read(e.data, { dense: true });
  const ws = wb.Sheets["Data"];
  const range = XLSX.utils.decode_range(ws["!ref"]!);
  const rows = XLSX.utils.sheet_to_json<unknown[]>(ws, { header: 1, range: { s: { r: 0, c: 0 }, e: { r: 199, c: range.e.c } } });
  (self as unknown as Worker).postMessage({ parseMs: Math.round(performance.now() - t0), rows, total: range.e.r + 1, cols: range.e.c + 1, sheets: wb.SheetNames.length });
};
