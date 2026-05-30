import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const messagesDir = join(root, "messages");
const referenceLocale = "zh-CN";
const comparedLocales = ["en-US"];

function flatten(value, prefix = "") {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return Object.entries(value).flatMap(([key, child]) =>
      flatten(child, prefix ? `${prefix}.${key}` : key),
    );
  }

  return [prefix];
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

const namespaces = readdirSync(join(messagesDir, referenceLocale))
  .filter((file) => file.endsWith(".json"))
  .sort();

let failed = false;

for (const locale of comparedLocales) {
  for (const namespace of namespaces) {
    const referencePath = join(messagesDir, referenceLocale, namespace);
    const comparedPath = join(messagesDir, locale, namespace);

    if (!existsSync(comparedPath)) {
      console.error(`${locale}/${namespace} is missing`);
      failed = true;
      continue;
    }

    const referenceKeys = new Set(flatten(readJson(referencePath)));
    const comparedKeys = new Set(flatten(readJson(comparedPath)));
    const missing = [...referenceKeys].filter((key) => !comparedKeys.has(key));
    const extra = [...comparedKeys].filter((key) => !referenceKeys.has(key));

    for (const key of missing) {
      console.error(`${locale}/${namespace} missing key: ${key}`);
      failed = true;
    }

    for (const key of extra) {
      console.error(`${locale}/${namespace} extra key: ${key}`);
      failed = true;
    }
  }
}

if (failed) {
  process.exit(1);
}

console.log("i18n messages are in sync");
