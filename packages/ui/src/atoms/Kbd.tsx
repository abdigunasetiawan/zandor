import { cx } from "../cx";

/** Tampilan pintasan, mis. `Ctrl+Shift+K` → [Ctrl] + [Shift] + [K]. Pembaca layar membaca teks utuh. */
export function Kbd({ keys, className }: { keys: string; className?: string }) {
  const parts = keys.split("+").filter(Boolean);
  return (
    <span className={cx("z-kbd-group", className)} role="img" aria-label={keys}>
      {parts.map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: urutan tombol tetap dan bisa berulang
        <span key={i} aria-hidden="true">
          <kbd className="z-kbd">{p}</kbd>
          {i < parts.length - 1 && <span className="z-kbd-sep">+</span>}
        </span>
      ))}
    </span>
  );
}
