import { buildInfo } from "@zandor/core";

/** Info versi build (ADR-016): dev = 0.0.<jumlah commit>; rilis = semver paket. */
export const build = buildInfo({
  packageVersion: __APP_PACKAGE_VERSION__,
  commitCount: __APP_COMMIT_COUNT__,
  isRelease: __APP_RELEASE__,
});
