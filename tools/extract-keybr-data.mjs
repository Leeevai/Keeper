#!/usr/bin/env node
// Extracts the data Keeper needs from a checkout of https://github.com/aradzie/keybr.com
// (AGPL-3.0) and writes it to ./assets as plain JSON and binary files.
//
// Usage:
//   git clone --depth 1 https://github.com/aradzie/keybr.com /path/to/keybr.com
//   node tools/extract-keybr-data.mjs /path/to/keybr.com [assets-dir]
//
// The keybr sources are TypeScript files whose data is written as object
// literals. This script never executes them: it tokenizes each literal and
// converts it to JSON, so a checkout can be processed safely.

import fs from "node:fs";
import path from "node:path";

const [, , srcArg, outArg] = process.argv;
if (!srcArg) {
  console.error("usage: node tools/extract-keybr-data.mjs <keybr.com checkout> [assets dir]");
  process.exit(2);
}
const SRC = path.resolve(srcArg);
const PKG = path.join(SRC, "packages");
const OUT = path.resolve(outArg ?? path.join(import.meta.dirname, "..", "assets"));

const read = (...p) => fs.readFileSync(path.join(PKG, ...p), "utf8");
const write = (rel, data) => {
  const file = path.join(OUT, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, data);
};
const writeJson = (rel, value) => write(rel, JSON.stringify(value) + "\n");

const regionNames = new Intl.DisplayNames(["en"], { type: "region" });
const languageNames = new Intl.DisplayNames(["en"], { type: "language" });

// ---------------------------------------------------------------------------
// A tiny converter from JavaScript object/array literals to JSON.

/** Returns the index just past the literal that starts at `start` (a `{` or `[`). */
function literalEnd(src, start) {
  let depth = 0;
  for (let i = start; i < src.length; i++) {
    const c = src[i];
    if (c === "/" && src[i + 1] === "*") {
      i = src.indexOf("*/", i + 2) + 1;
      continue;
    }
    if (c === "/" && src[i + 1] === "/") {
      i = src.indexOf("\n", i);
      continue;
    }
    if (c === '"' || c === "'" || c === "`") {
      for (i++; src[i] !== c; i++) {
        if (src[i] === "\\") i++;
      }
      continue;
    }
    if (c === "{" || c === "[") depth++;
    if (c === "}" || c === "]") {
      depth--;
      if (depth === 0) return i + 1;
    }
  }
  throw new Error("unterminated literal");
}

/** Converts a JavaScript literal (no code, only data) into a JSON string. */
function literalToJson(src) {
  let out = "";
  const isIdStart = (c) => /[A-Za-z_$]/.test(c);
  const isId = (c) => /[A-Za-z0-9_$]/.test(c);
  const dropTrailingComma = () => {
    out = out.replace(/,\s*$/, "");
  };
  for (let i = 0; i < src.length; ) {
    const c = src[i];
    if (c === "/" && src[i + 1] === "*") {
      i = src.indexOf("*/", i + 2) + 2;
      continue;
    }
    if (c === "/" && src[i + 1] === "/") {
      i = src.indexOf("\n", i);
      if (i < 0) i = src.length;
      continue;
    }
    if (c === '"' || c === "'") {
      let s = "";
      for (i++; src[i] !== c; i++) {
        if (src[i] === "\\") {
          const n = src[++i];
          s += n === "'" ? "'" : "\\" + n;
        } else if (src[i] === '"') {
          s += '\\"';
        } else {
          s += src[i];
        }
      }
      i++;
      out += '"' + s + '"';
      continue;
    }
    if (c === "." && src.startsWith("...", i)) {
      i += 3;
      let name = "";
      while (isId(src[i])) name += src[i++];
      out += JSON.stringify("\u0000spread:" + name) + ":null";
      continue;
    }
    if (/[0-9]/.test(c) || (c === "-" && /[0-9.]/.test(src[i + 1])) || (c === "." && /[0-9]/.test(src[i + 1]))) {
      let num = "";
      while (i < src.length && /[-0-9a-fA-FxX._]/.test(src[i])) num += src[i++];
      num = num.replaceAll("_", "");
      const neg = num.startsWith("-");
      const body = neg ? num.slice(1) : num;
      const value = /^0x/i.test(body) ? parseInt(body, 16) : Number(body);
      if (!Number.isFinite(value)) throw new Error(`bad number ${num}`);
      out += String(neg ? -value : value);
      continue;
    }
    if (isIdStart(c)) {
      let name = "";
      while (i < src.length && isId(src[i])) name += src[i++];
      let j = i;
      while (/\s/.test(src[j])) j++;
      if (src[j] === ":") {
        out += JSON.stringify(name);
      } else if (name === "true" || name === "false" || name === "null") {
        out += name;
      } else {
        out += JSON.stringify({ $ref: name });
      }
      continue;
    }
    if (c === "}" || c === "]") {
      dropTrailingComma();
    }
    out += c;
    i++;
  }
  return out;
}

/** Parses `export const NAME ... = <literal>` (or the default export) from a file. */
function parseExport(src, name = null) {
  const re = name == null ? /export default\s*/ : new RegExp(`export const ${name}\\b[^=]*=\\s*`);
  const m = re.exec(src);
  if (m == null) throw new Error(`export ${name ?? "default"} not found`);
  let start = m.index + m[0].length;
  while (src[start] !== "{" && src[start] !== "[") start++;
  const end = literalEnd(src, start);
  return JSON.parse(literalToJson(src.slice(start, end)));
}

/** Merges `...SPREAD` entries using the given resolver. */
function resolveSpreads(value, resolve) {
  if (Array.isArray(value)) return value.map((v) => resolveSpreads(v, resolve));
  if (value != null && typeof value === "object") {
    const result = {};
    for (const [key, v] of Object.entries(value)) {
      if (key.startsWith("\u0000spread:")) {
        Object.assign(result, resolveSpreads(resolve(key.slice(8)), resolve));
      } else {
        result[key] = resolveSpreads(v, resolve);
      }
    }
    return result;
  }
  return value;
}

/** Splits the arguments of every `static readonly X = new Class(...)` in a registry file. */
function registryEntries(src, className) {
  const entries = [];
  const re = new RegExp(`static readonly (\\w+) = new ${className}\\(`, "g");
  let m;
  while ((m = re.exec(src)) != null) {
    let depth = 1;
    let i = re.lastIndex;
    for (; depth > 0; i++) {
      if (src[i] === '"') {
        for (i++; src[i] !== '"'; i++) if (src[i] === "\\") i++;
      } else if (src[i] === "(") depth++;
      else if (src[i] === ")") depth--;
    }
    entries.push({ name: m[1], body: src.slice(re.lastIndex, i - 1) });
  }
  return entries;
}

/** Splits a comma separated argument list at the top level. */
function splitArgs(body) {
  const args = [];
  let depth = 0;
  let cur = "";
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if (c === "/" && body[i + 1] === "*") {
      i = body.indexOf("*/", i + 2) + 1;
      continue;
    }
    if (c === "/" && body[i + 1] === "/") {
      i = body.indexOf("\n", i);
      continue;
    }
    if (c === '"') {
      let j = i + 1;
      for (; body[j] !== '"'; j++) if (body[j] === "\\") j++;
      cur += body.slice(i, j + 1);
      i = j;
      continue;
    }
    if (c === "(" || c === "[" || c === "{") depth++;
    if (c === ")" || c === "]" || c === "}") depth--;
    if (c === "," && depth === 0) {
      args.push(cur.trim());
      cur = "";
      continue;
    }
    cur += c;
  }
  if (cur.trim() !== "") args.push(cur.trim());
  return args;
}

const str = (arg) => JSON.parse(arg);
const refs = (arg, prefix) => [...arg.matchAll(new RegExp(`${prefix}\\.(\\w+)`, "g"))].map((m) => m[1]);

// ---------------------------------------------------------------------------
// Languages.

const languageSrc = read("keybr-keyboard", "lib", "language.ts");
const languages = registryEntries(languageSrc, "Language").map(({ name, body }) => {
  const [id, script, direction, alphabet] = splitArgs(body).map(str);
  return { key: name, id, name: languageNames.of(id), script, direction, alphabet };
});
const languageByKey = new Map(languages.map((l) => [l.key, l]));

// ---------------------------------------------------------------------------
// Geometries, zone mods and layouts.

const geometrySrc = read("keybr-keyboard", "lib", "geometry.ts");
const zoneMods = registryEntries(geometrySrc, "ZoneMod").map(({ name, body }) => {
  const [id, label, dict] = splitArgs(body);
  return { key: name, id: str(id), name: str(label), dict: dict.trim() };
});
const geometryRegistry = registryEntries(geometrySrc, "Geometry").map(({ name, body }) => {
  const [id, label, form, zones] = splitArgs(body);
  const zoneKeys = zones.includes("ZoneMod.ALL") ? zoneMods.map((z) => z.key) : refs(zones, "ZoneMod");
  return { key: name, id: str(id), name: str(label), form: str(form), zones: zoneKeys };
});

const geometryDir = path.join(PKG, "keybr-keyboard", "lib", "geometry");
const geometryExports = new Map();
for (const file of fs.readdirSync(geometryDir)) {
  if (!file.endsWith(".ts") || file.endsWith(".test.ts")) continue;
  const src = fs.readFileSync(path.join(geometryDir, file), "utf8");
  for (const m of src.matchAll(/export const (\w+)\b/g)) {
    geometryExports.set(m[1], () => parseExport(src, m[1]));
  }
}
const resolveGeometry = (name) => {
  const load = geometryExports.get(name);
  if (load == null) throw new Error(`geometry export ${name} not found`);
  return load();
};

const keyboardLoadSrc = read("keybr-keyboard", "lib", "load.ts");
const geometryDictName = new Map(
  [...keyboardLoadSrc.matchAll(/\[\s*Geometry\.(\w+),\s*(\w+),?\s*\]/g)].map((m) => [m[1], m[2]]),
);
const layoutDictName = new Map(
  [...keyboardLoadSrc.matchAll(/\[\s*Layout\.(\w+),\s*(\w+),?\s*\]/g)].map((m) => [m[1], m[2]]),
);

const geometries = geometryRegistry.map((g) => {
  const dict = resolveSpreads(resolveGeometry(geometryDictName.get(g.key)), resolveGeometry);
  const keys = Object.entries(dict).map(([id, k]) => ({
    id,
    x: k.x,
    y: k.y,
    w: k.w ?? 1,
    h: k.h ?? 1,
    labels: (k.labels ?? []).map((l) => l.text),
    shape: k.shape ?? null,
    zones: k.zones ?? [],
    homing: k.homing ?? false,
  }));
  const zones = g.zones.map((key) => zoneMods.find((z) => z.key === key).id);
  return { id: g.id, name: g.name, form: g.form, zones, keys };
});

const zoneModDicts = Object.fromEntries(
  zoneMods.map((z) => [z.id, { name: z.name, zones: resolveGeometry(z.dict) }]),
);

const layoutModSrc = read("keybr-keyboard", "lib", "mod.ts");
const layoutMods = {};
for (const decl of layoutModSrc.split("export const ").slice(1)) {
  const m = /^(\w+): Mod = [\s\S]*?remapZones\(dict, (\w+)\)/.exec(decl);
  if (m != null) {
    layoutMods[m[1]] = resolveGeometry(m[2]);
  }
}

const layoutDir = path.join(PKG, "keybr-keyboard", "lib", "layout");
const layoutExports = new Map();
for (const file of fs.readdirSync(layoutDir)) {
  if (!file.endsWith(".ts")) continue;
  const src = fs.readFileSync(path.join(layoutDir, file), "utf8");
  for (const m of src.matchAll(/export const (\w+)\b/g)) {
    layoutExports.set(m[1], () => parseExport(src, m[1]));
  }
}

const layoutSrc = read("keybr-keyboard", "lib", "layout.ts");
const geometryIdByKey = new Map(geometryRegistry.map((g) => [g.key, g.id]));
const layouts = [];
for (const { name, body } of registryEntries(layoutSrc, "Layout")) {
  const args = splitArgs(body);
  if (args[0] === "id" || !args[0].startsWith('"')) continue; // the custom() template
  const [id, xid, label, family, language, emulate, geometryArg, mod = "nullMod"] = args;
  const dictName = layoutDictName.get(name);
  if (dictName == null) throw new Error(`no character dict for layout ${name}`);
  const chars = layoutExports.get(dictName)();
  const languageKey = language.replace("Language.", "");
  const formattedName = str(label).replaceAll(/\{([-A-Z]+)\}/g, (_, region) => regionNames.of(region));
  layouts.push({
    id: str(id),
    xid: Number(xid),
    name: formattedName,
    family: str(family),
    language: languageByKey.get(languageKey).id,
    emulate: emulate === "true",
    geometries: refs(geometryArg, "Geometry").map((key) => geometryIdByKey.get(key)),
    mod: mod.trim() === "nullMod" ? null : mod.trim(),
    chars,
  });
}
const layoutOrder = refs(layoutSrc.slice(layoutSrc.indexOf("static readonly ALL")), "Layout");
const layoutKeyById = new Map(
  registryEntries(layoutSrc, "Layout")
    .filter(({ body }) => splitArgs(body)[0].startsWith('"'))
    .map(({ name, body }) => [str(splitArgs(body)[0]), name]),
);
layouts.sort((a, b) => layoutOrder.indexOf(layoutKeyById.get(a.id)) - layoutOrder.indexOf(layoutKeyById.get(b.id)));

writeJson("keyboard/languages.json", languages.map(({ key, ...rest }) => rest));
writeJson("keyboard/geometries.json", { geometries, zoneMods: zoneModDicts, layoutMods });
writeJson("keyboard/layouts.json", layouts);

// ---------------------------------------------------------------------------
// Phonetic models, blacklists, word lists, books and text normalization.

const modelDir = path.join(PKG, "keybr-phonetic-model", "assets");
for (const file of fs.readdirSync(modelDir)) {
  if (file.startsWith("model-") && file.endsWith(".data")) {
    write(`models/${file}`, fs.readFileSync(path.join(modelDir, file)));
  }
}

const blacklistDir = path.join(PKG, "keybr-phonetic-model", "lib", "blacklist");
for (const file of fs.readdirSync(blacklistDir)) {
  const m = /^blacklist-(.+)\.json$/.exec(file);
  if (m != null) {
    const words = JSON.parse(fs.readFileSync(path.join(blacklistDir, file), "utf8"));
    if (words.length > 0) writeJson(`blacklist/blacklist-${m[1]}.json`, words);
  }
}

const wordsDir = path.join(PKG, "keybr-content-words", "lib", "data");
for (const file of fs.readdirSync(wordsDir)) {
  if (file.endsWith(".json")) {
    writeJson(`words/${file}`, JSON.parse(fs.readFileSync(path.join(wordsDir, file), "utf8")));
  }
}

const bookSrc = read("keybr-content", "lib", "books", "book.ts");
const books = registryEntries(bookSrc, "Book").map(({ body }) => {
  const [id, language, title, author] = splitArgs(body);
  return {
    id: str(id),
    language: languageByKey.get(language.replace("Language.", "")).id,
    title: str(title),
    author: str(author),
  };
});
const booksDir = path.join(PKG, "keybr-content-books", "lib", "data");
for (const book of books) {
  writeJson(`books/${book.id}.json`, JSON.parse(fs.readFileSync(path.join(booksDir, `${book.id}.json`), "utf8")));
}
writeJson("books/books.json", books);

const normalizeSrc = read("keybr-unicode", "lib", "normalize.ts");
const tableAfter = (marker) => {
  const at = normalizeSrc.indexOf(marker);
  const start = normalizeSrc.indexOf("})([", at) + 3;
  return JSON.parse(literalToJson(normalizeSrc.slice(start, literalEnd(normalizeSrc, start))));
};
writeJson("unicode/normalize.json", {
  expand: tableAfter("const expandMap"),
  replace: tableAfter("const replaceMap"),
});

// ---------------------------------------------------------------------------
// Source code grammars.

const syntaxDir = path.join(PKG, "keybr-code", "lib", "syntax");
const grammarFiles = {};
for (const file of fs.readdirSync(syntaxDir)) {
  if (file.endsWith(".ts") && file !== "grammars.ts") {
    const src = fs.readFileSync(path.join(syntaxDir, file), "utf8");
    grammarFiles[file.replace(/\.ts$/, "")] = parseExport(src);
  }
}
const grammarsSrc = fs.readFileSync(path.join(syntaxDir, "grammars.ts"), "utf8");
const grammars = {};
for (const m of grammarsSrc.matchAll(/export const (\w+) = compose\(([^)]*)\)/g)) {
  grammars[m[1]] = m[2].split(",").map((s) => s.trim());
}
for (const [name, rules] of Object.entries(grammarFiles)) {
  writeJson(`code/${name}.json`, rules);
}
const syntaxSrc = read("keybr-code", "lib", "syntax.ts");
const syntaxes = registryEntries(syntaxSrc, "Syntax").map(({ body }) => {
  const [id, label, grammar, start = '"start"'] = splitArgs(body);
  return { id: str(id), name: str(label), grammar: grammars[grammar.trim()], start: str(start) };
});
writeJson("code/syntaxes.json", syntaxes);

// ---------------------------------------------------------------------------
// User speed and accuracy distributions, and typing sounds.

for (const name of ["dist_speed.json", "dist_accuracy.json"]) {
  writeJson(`dist/${name}`, JSON.parse(read("keybr-chart", "lib", "dist", name)));
}

const soundDir = path.join(PKG, "keybr-textinput-sounds", "assets");
for (const file of fs.readdirSync(soundDir)) {
  if (file.endsWith(".mp3")) write(`sounds/${file}`, fs.readFileSync(path.join(soundDir, file)));
}
