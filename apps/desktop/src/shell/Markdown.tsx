// Markdown respons model di chat (Fase 12): renderer kaya (kode berwarna, Mermaid, KaTeX, tabel) dimuat malas agar bundel
// awal ringan; selama dimuat teks ditampilkan apa adanya. Detail keamanan ada di rich/RichMarkdown.tsx.
import { lazy, Suspense } from "react";

const Rich = lazy(() => import("./rich/RichMarkdown"));

export function Markdown({ text, streaming = false }: { text: string; streaming?: boolean }) {
  return (
    <Suspense
      fallback={
        <div className="md">
          <p className="msg__text">{text}</p>
        </div>
      }
    >
      <Rich text={text} streaming={streaming} />
    </Suspense>
  );
}
