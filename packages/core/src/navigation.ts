// Riwayat navigasi aplikasi (tombol back/next di top bar). Murni, bisa diamati.

export class NavigationHistory<T> {
  private stack: T[];
  private index = 0;
  private listeners = new Set<() => void>();
  private snap: { current: T; canBack: boolean; canForward: boolean };

  constructor(
    initial: T,
    private readonly equals: (a: T, b: T) => boolean = Object.is,
    private readonly limit = 100,
  ) {
    this.stack = [initial];
    this.snap = this.compute();
  }

  private compute() {
    return {
      current: this.stack[this.index] as T,
      canBack: this.index > 0,
      canForward: this.index < this.stack.length - 1,
    };
  }

  private changed() {
    this.snap = this.compute();
    for (const l of this.listeners) l();
  }

  /** Snapshot stabil: identitas hanya berubah bila ada navigasi. */
  state() {
    return this.snap;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Pergi ke tujuan baru. Entri ke depan dibuang. Tujuan yang sama dengan sekarang diabaikan. */
  push(entry: T): void {
    if (this.equals(entry, this.stack[this.index] as T)) return;
    this.stack = [...this.stack.slice(0, this.index + 1), entry];
    if (this.stack.length > this.limit) this.stack.shift();
    this.index = this.stack.length - 1;
    this.changed();
  }

  /** Ganti entri sekarang tanpa menambah riwayat (mis. sinkron dari URL). */
  replace(entry: T): void {
    if (this.equals(entry, this.stack[this.index] as T)) return;
    this.stack[this.index] = entry;
    this.changed();
  }

  back(): boolean {
    if (this.index === 0) return false;
    this.index--;
    this.changed();
    return true;
  }

  forward(): boolean {
    if (this.index >= this.stack.length - 1) return false;
    this.index++;
    this.changed();
    return true;
  }
}
