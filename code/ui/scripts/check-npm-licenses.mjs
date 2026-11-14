#!/usr/bin/env node
/**
 * UI-LIC-001 verifier: audits every non-dev dependency in package-lock.json
 * against the strict permissive Steam-compatible SPDX allowlist.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const ALLOWED_NPM_LICENSES = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "0BSD",
  "CC0-1.0",
]);

/**
 * Evaluates simple SPDX license expressions supporting parentheses, `OR`, and `AND`.
 */
export function isSpdxAllowed(rawLicense, allowlist = ALLOWED_NPM_LICENSES) {
  if (typeof rawLicense !== "string") return false;
  const trimmed = rawLicense.trim();
  if (trimmed.length === 0) return false;

  const normalized = trimmed.replace(/^\((.+)\)$/, "$1").trim();
  if (normalized.includes(" OR ")) {
    return normalized.split(" OR ").some((clause) => isSpdxAllowed(clause, allowlist));
  }
  if (normalized.includes(" AND ")) {
    return normalized.split(" AND ").every((clause) => isSpdxAllowed(clause, allowlist));
  }
  return allowlist.has(normalized);
}

export function verifyNpmLockfile(lockfilePath) {
  const lockData = JSON.parse(fs.readFileSync(lockfilePath, "utf8"));
  const packages = lockData.packages ?? {};
  const violations = [];
  let checkedCount = 0;

  for (const [pkgPath, pkgMeta] of Object.entries(packages)) {
    if (pkgPath === "") continue;
    if (pkgMeta.dev === true) continue;
    checkedCount += 1;

    const spdx = pkgMeta.license;
    if (!isSpdxAllowed(spdx)) {
      violations.push({
        packagePath: pkgPath,
        version: pkgMeta.version ?? "unknown",
        license: spdx ?? "<missing>",
      });
    }
  }

  return {
    ok: violations.length === 0,
    checkedCount,
    violations,
  };
}

function main() {
  const lockIdx = process.argv.indexOf("--lockfile");
  const defaultLockfile = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../package-lock.json");
  const lockfilePath = lockIdx !== -1 && process.argv[lockIdx + 1]
    ? path.resolve(process.argv[lockIdx + 1])
    : defaultLockfile;

  const result = verifyNpmLockfile(lockfilePath);
  if (!result.ok) {
    console.error("FAIL: Disallowed or missing production npm licenses found:");
    for (const v of result.violations) {
      console.error(`  - ${v.packagePath}@${v.version}: ${v.license}`);
    }
    process.exit(1);
  }
  console.log(`PASS: ${result.checkedCount} production npm packages verified against SPDX allowlist.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
