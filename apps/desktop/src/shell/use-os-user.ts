// Nama pengguna OS dari host (null bila tidak diketahui, mis. di browser). Dipakai footer sidebar dan sapaan.
import { useEffect, useState } from "react";
import { useServices } from "./services";

export function useOsUser(): string | null {
  const { host } = useServices();
  const [user, setUser] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    host.osUser().then(
      (n) => live && setUser(n),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [host]);
  return user;
}

/** Bagian hari untuk sapaan (jam lokal). */
export function dayPart(hour: number): "morning" | "afternoon" | "evening" | "night" {
  if (hour >= 4 && hour < 11) return "morning";
  if (hour >= 11 && hour < 15) return "afternoon";
  if (hour >= 15 && hour < 19) return "evening";
  return "night";
}
