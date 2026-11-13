// Akses minimal ke API global Tauri (withGlobalTauri).
const T = (window as any).__TAURI__;
export const invoke = <R = unknown>(cmd: string, args?: Record<string, unknown>): Promise<R> => T.core.invoke(cmd, args);
export const listen = <P = unknown>(ev: string, cb: (p: P) => void): Promise<() => void> =>
  T.event.listen(ev, (e: { payload: P }) => cb(e.payload));
