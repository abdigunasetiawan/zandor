// Hasilkan docs/manual/settings.schema.json dan docs/manual/settings.md dari SETTINGS_SCHEMA.
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { schemaJson, schemaMarkdown } from "../packages/core/src/settings";

const dir = join(import.meta.dir, "..", "docs", "manual");
writeFileSync(join(dir, "settings.schema.json"), schemaJson());
writeFileSync(join(dir, "settings.md"), schemaMarkdown());
console.log("docs/manual/settings.schema.json dan settings.md diperbarui");
