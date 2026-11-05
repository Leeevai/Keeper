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
