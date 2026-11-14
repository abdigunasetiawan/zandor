// Tampilan teks diff unified dengan pewarnaan baris (tambah/hapus/konteks). Dipakai Panel Git dan Perubahan sesi.

export function DiffText({ text, testId = "git-diff" }: { text: string; testId?: string }) {
  return (
    <pre className="fold__pre diff" data-testid={testId}>
      {text.split("\n").map((l, i) => {
        const kind =
          l.startsWith("+") && !l.startsWith("+++") ? "add" : l.startsWith("-") && !l.startsWith("---") ? "del" : "ctx";
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: baris diff statis, urutan tetap
          <span key={i} className={`diff__line diff__line--${kind}`}>
            {l}
            {"\n"}
          </span>
        );
      })}
    </pre>
  );
}
