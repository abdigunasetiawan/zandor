// Konteks sesi demo: kartu izin/pertanyaan dan aksi pesan memeriksanya untuk berperilaku lokal (tanpa engine).
import { createContext, useContext } from "react";

export type DemoApi = {
  permission(requestId: string, reply: "once" | "always" | "reject"): void;
  /** `null` = dilewati. */
  question(requestId: string, answers: string[][] | null): void;
};

export const DemoContext = createContext<DemoApi | null>(null);
export const useDemo = () => useContext(DemoContext);
