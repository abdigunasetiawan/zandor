// Klasifikasi risiko permintaan izin untuk mode "auto-safe": aksi biasa disetujui otomatis, aksi sensitif atau
// berbahaya tetap ditanyakan ke pengguna. Sengaja konservatif: jenis izin yang tidak dikenal selalu ditanyakan.
// Ini lapisan kenyamanan di klien, BUKAN sandbox — pola bisa dilewati oleh perintah yang dikaburkan; pengguna yang
// butuh jaminan memakai mode "ask".
import type { PermissionRequest } from "./types";

/** Alasan sebuah permintaan dianggap berisiko (kunci i18n `chat.risk.<alasan>`). */
export type PermissionRisk =
  | "outside" // akses folder di luar project
  | "loop" // agent mengulang aksi yang sama (rem pengaman engine)
  | "secret" // berkas rahasia: .env, kunci, kredensial, .git internal
  | "delete" // menghapus berkas/folder secara paksa atau rekursif
  | "git-history" // menulis ulang/membuang riwayat atau perubahan git
  | "publish" // keluar ke pihak luar: git push, publish paket, rilis
  | "remote-script" // menjalankan skrip dari internet
  | "system" // hak admin, sistem, layanan, registry, matikan mesin
  | "global-install" // memasang paket secara global
  | "unknown"; // jenis izin yang tidak dikenal

const SECRET_PATH: RegExp[] = [
  /(^|[\\/])\.env($|\.(?!example$|sample$|template$)[^\\/]*$)/i,
  /\.(pem|key|p12|pfx|jks|keystore|kdbx)$/i,
  /(^|[\\/])id_(rsa|dsa|ecdsa|ed25519)(\.pub)?$/i,
  /(^|[\\/])\.ssh([\\/]|$)/i,
  /(^|[\\/])\.git[\\/]/i,
  /(^|[\\/])(\.npmrc|\.pypirc|\.netrc|\.git-credentials|credentials(\.json)?|secrets?\.(json|ya?ml|toml))$/i,
];

const BASH_RULES: Array<[PermissionRisk, RegExp]> = [
  [
    "secret",
    /(^|[\s"'=/\\])\.env(\.(?!example\b|sample\b|template\b)\w+)?(\s|$|["'])|[/\\]\.ssh\b|\bid_(rsa|ed25519|ecdsa)\b/i,
  ],
  // rm -r / -f / wildcard, rmdir, del /s, Remove-Item -Recurse/-Force, find -delete, shred
  [
    "delete",
    /\brm\s+(-\w*[rRf]|--recursive|--force)|\brm\s+[^|;&]*\*|\brmdir\b|\brd\s+\/s|\bdel\s+(\/[sfq]\s*)+|\bRemove-Item\b[^|;&]*-(Recurse|Force)|\bfind\b[^|;&]*-delete\b|\bshred\b/i,
  ],
  [
    "git-history",
    /\bgit\s+(reset\s+--hard|clean\s+-\w*f|checkout\s+(--\s|\.)|restore\b|branch\s+-D|rebase\b|filter-branch|filter-repo|stash\s+(drop|clear)|reflog\s+expire|update-ref\s+-d)/i,
  ],
  [
    "publish",
    /\bgit\s+push\b|\b(npm|pnpm|yarn|bun)\s+publish\b|\bcargo\s+publish\b|\btwine\s+upload\b|\bdocker\s+push\b|\bgh\s+(release\s+(create|delete)|repo\s+(delete|create)|pr\s+merge)\b|\bvercel\b[^|;&]*--prod|\bnetlify\s+deploy\b[^|;&]*--prod/i,
  ],
  [
    "remote-script",
    /\b(curl|wget)\b[^|]*\|\s*(sudo\s+)?(ba|z)?sh\b|\b(iwr|irm|Invoke-WebRequest|Invoke-RestMethod)\b[^|]*\|\s*(iex|Invoke-Expression)\b|\b(iex|Invoke-Expression)\b/i,
  ],
  [
    "system",
    /(^|[;&|(]\s*)(sudo|su|doas|runas|shutdown|reboot|halt|poweroff)(\s|$)|\bmkfs\b|\bformat\s+[a-z]:|\bdd\s+[^|;&]*of=|\bchmod\s+(-R\s+)?(777|-R)|\bchown\s+-R\b|\b(reg|regedit)\s+(add|delete|import)\b|\bsetx\b|\bSet-ExecutionPolicy\b|\b(sc|systemctl|service)\s+(stop|delete|disable|config)\b|\b(kill\s+-9|killall|pkill|taskkill\b[^|;&]*\/f)\b|(^|\s)>\s*\/dev\/sd|\bdiskpart\b/i,
  ],
  [
    "global-install",
    /\b(npm|pnpm)\s+(i|install|add)\b[^|;&]*\s(-g|--global)\b|\byarn\s+global\s+add\b|\bbun\s+(add|install)\b[^|;&]*\s(-g|--global)\b|\bcargo\s+install\b|\bgo\s+install\b|\b(apt|apt-get|yum|dnf|pacman|brew|choco|winget|scoop)\s+(install|remove|uninstall)\b/i,
  ],
];

const strings = (v: unknown): string[] =>
  typeof v === "string" ? [v] : Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];

/** Teks yang relevan dari permintaan: pola + metadata yang umum dikirim engine (perintah, path). */
function texts(req: PermissionRequest): string[] {
  const m = req.metadata;
  return [...req.patterns, ...strings(m.command), ...strings(m.filepath), ...strings(m.filePath), ...strings(m.path)];
}

const isSecretPath = (p: string) => SECRET_PATH.some((re) => re.test(p.trim()));

/** Mengembalikan alasan risiko, atau `null` bila aman disetujui otomatis di mode "auto-safe". */
export function permissionRisk(req: PermissionRequest): PermissionRisk | null {
  switch (req.permission) {
    case "external_directory":
      return "outside";
    case "doom_loop":
      return "loop";
    case "edit":
    case "write":
    case "read":
    case "list":
    case "glob":
    case "grep":
      return texts(req).some(isSecretPath) ? "secret" : null;
    case "webfetch":
    case "websearch":
    case "codesearch":
    case "todowrite":
    case "todoread":
    case "task":
    case "skill":
    case "lsp":
      return null;
    case "bash": {
      const cmds = texts(req);
      for (const [risk, re] of BASH_RULES) if (cmds.some((c) => re.test(c))) return risk;
      return null;
    }
    default:
      return "unknown";
  }
}
