// Penanda sesi untuk komponen pesan: apakah sesi berjalan di dalam project (kartu berkas dokumen hanya untuk sesi TANPA project).
import { createContext, useContext } from "react";

export type SessionFlags = { hasProject: boolean };

export const SessionFlagsContext = createContext<SessionFlags>({ hasProject: false });
export const useSessionFlags = () => useContext(SessionFlagsContext);
