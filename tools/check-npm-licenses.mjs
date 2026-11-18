#!/usr/bin/env node
// Fails if any npm dependency of the desktop app has a licence outside the allow-list.
// Usage (from apps/desktop): pnpm licenses list --json | node ../../tools/check-npm-licenses.mjs
// Keep ALLOWED in sync with deny.toml [licenses].allow.

const ALLOWED = new Set([
  "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "0BSD",
  "BlueOak-1.0.0", "MPL-2.0", "CC0-1.0", "CC-BY-4.0", "Unicode-3.0", "OFL-1.1", "Python-2.0",
]);

/** True if an SPDX expression is satisfiable with allowed licences (handles OR / AND / parens). */
function allowed(expr) {
  const e = expr.trim().replace(/^\((.*)\)$/, "$1");
  if (/\s+OR\s+/.test(e)) return e.split(/\s+OR\s+/).some(allowed);
  if (/\s+AND\s+/.test(e)) return e.split(/\s+AND\s+/).every(allowed);
  return ALLOWED.has(e);
}

let input = "";
process.stdin.setEncoding("utf8");
for await (const chunk of process.stdin) input += chunk;

const byLicence = JSON.parse(input);
const bad = [];
for (const [licence, pkgs] of Object.entries(byLicence)) {
  if (!allowed(licence)) for (const p of pkgs) bad.push(`${p.name}@${p.versions.join(",")}: ${licence}`);
}

if (bad.length) {
  console.error(`Disallowed npm licences:\n  ${bad.join("\n  ")}`);
  process.exit(1);
}
console.log(`npm licences OK (${Object.keys(byLicence).length} licence types).`);
