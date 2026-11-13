// Server statis lokal untuk benchmark S5; POST /report menulis hasil ke ../target/s5-report.json.
import { join } from "node:path";
const pub = join(import.meta.dir, "..", "public");
const nm = join(import.meta.dir, "..", "node_modules");
const port = Number(process.env.PORT ?? 5175);
Bun.serve({
  port,
  hostname: "127.0.0.1",
  async fetch(req) {
    const url = new URL(req.url);
    if (req.method === "POST" && url.pathname === "/report") {
      await Bun.write(join(import.meta.dir, "..", "..", "target", "s5-report.json"), await req.text());
      return new Response("ok");
    }
    if (url.pathname === "/pdf.worker.min.mjs") return new Response(Bun.file(join(nm, "pdfjs-dist/build/pdf.worker.min.mjs")), { headers: { "content-type": "text/javascript" } });
    const f = Bun.file(join(pub, url.pathname === "/" ? "index.html" : url.pathname));
    return (await f.exists()) ? new Response(f) : new Response("not found", { status: 404 });
  },
});
console.log(`S5 bench: http://127.0.0.1:${port}/`);
