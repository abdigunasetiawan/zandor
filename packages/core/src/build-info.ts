// Versi aplikasi (ADR-016): semver untuk rilis; build dev ditampilkan sebagai `0.0.<jumlah commit>`.

export type BuildInfo = { version: string; commitCount: number | null; channel: "dev" | "release" };

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

export function buildInfo(input: {
  packageVersion: string;
  commitCount: string | number | null;
  isRelease: boolean;
}): BuildInfo {
  if (!SEMVER.test(input.packageVersion)) throw new Error(`versi paket bukan semver: ${input.packageVersion}`);
  const n = input.commitCount === null || input.commitCount === "" ? null : Number(input.commitCount);
  const commitCount = n !== null && Number.isInteger(n) && n >= 0 ? n : null;
  if (input.isRelease) return { version: input.packageVersion, commitCount, channel: "release" };
  return { version: `0.0.${commitCount ?? 0}`, commitCount, channel: "dev" };
}
