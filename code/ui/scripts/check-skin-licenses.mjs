#!/usr/bin/env node
/**
 * UI-LIC-002 verifier: audits bundled font and baseline SVG asset licenses across
 * every skin directory under skins/.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const ALLOWED_SKIN_LICENSES = new Set([
  "OFL-1.1",
  "Apache-2.0",
  "MIT",
  "CC0-1.0",
]);

const MONITORED_EXTENSIONS = new Set([".ttf", ".woff2", ".otf", ".svg"]);

function parseBaselineAssetsToml(tomlText) {
  const entries = [];
  let currentSection = null;
  let currentObj = null;

  for (const rawLine of tomlText.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) continue;
    if (line === "[[fonts]]" || line === "[[baseline_assets]]") {
      if (currentObj) entries.push(currentObj);
      currentSection = line.slice(2, -2);
      currentObj = { section: currentSection };
      continue;
    }
    const eqIdx = line.indexOf("=");
    if (eqIdx !== -1 && currentObj) {
      const key = line.slice(0, eqIdx).trim();
      const val = line.slice(eqIdx + 1).trim().replace(/^"(.*)"$/, "$1");
      currentObj[key] = val;
    }
  }
  if (currentObj) entries.push(currentObj);
  return entries;
}

function walkSkinFiles(rootDir, currentDir = rootDir, out = []) {
  for (const entry of fs.readdirSync(currentDir, { withFileTypes: true })) {
    const fullPath = path.join(currentDir, entry.name);
    if (entry.isDirectory()) {
      walkSkinFiles(rootDir, fullPath, out);
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name).toLowerCase();
      if (MONITORED_EXTENSIONS.has(ext)) {
        out.push(path.relative(rootDir, fullPath).replace(/\\/g, "/"));
      }
    }
  }
  return out;
}

export function verifySkinLicenses(skinsRootDir) {
  if (!fs.existsSync(skinsRootDir)) {
    return { ok: false, skinCount: 0, assetCount: 0, errors: [`Missing skins root directory: ${skinsRootDir}`] };
  }

  const skinDirs = fs
    .readdirSync(skinsRootDir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort();

  const errors = [];
  let totalAssets = 0;

  for (const skinName of skinDirs) {
    const skinDir = path.join(skinsRootDir, skinName);
    const manifestPath = path.join(skinDir, "licenses", "baseline_assets.toml");
    if (!fs.existsSync(manifestPath)) {
      errors.push(`[${skinName}] Missing license manifest: ${manifestPath}`);
      continue;
    }

    const entries = parseBaselineAssetsToml(fs.readFileSync(manifestPath, "utf8"));
    const declaredRelPaths = new Set();

    for (const item of entries) {
      const rel = item.rel_path;
      const spdx = item.spdx_license;
      const licFile = item.license_file;
      if (!rel || !spdx || !licFile) {
        errors.push(`[${skinName}] Incomplete asset license entry: ${JSON.stringify(item)}`);
        continue;
      }
      declaredRelPaths.add(rel);
      totalAssets += 1;

      if (!ALLOWED_SKIN_LICENSES.has(spdx)) {
        errors.push(`[${skinName}] Disallowed SPDX license '${spdx}' for asset '${rel}'`);
      }

      const fullAssetPath = path.join(skinDir, rel);
      if (!fs.existsSync(fullAssetPath)) {
        errors.push(`[${skinName}] Declared asset missing on disk: ${rel}`);
      }

      const fullLicensePath = path.join(skinDir, licFile);
      if (!fs.existsSync(fullLicensePath) || fs.statSync(fullLicensePath).size === 0) {
        errors.push(`[${skinName}] Missing or empty license file '${licFile}' referenced by '${rel}'`);
      }
    }

    for (const diskRel of walkSkinFiles(skinDir)) {
      if (!declaredRelPaths.has(diskRel)) {
        errors.push(`[${skinName}] Uncatalogued font/SVG asset on disk without license entry: ${diskRel}`);
      }
    }
  }

  return {
    ok: errors.length === 0,
    skinCount: skinDirs.length,
    assetCount: totalAssets,
    errors,
  };
}

function main() {
  const dirIdx = process.argv.indexOf("--skins-dir");
  const defaultSkinsDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../skins");
  const skinsRootDir = dirIdx !== -1 && process.argv[dirIdx + 1]
    ? path.resolve(process.argv[dirIdx + 1])
    : defaultSkinsDir;

  const result = verifySkinLicenses(skinsRootDir);
  if (!result.ok) {
    console.error("FAIL: Skin asset license verification failed:");
    for (const err of result.errors) {
      console.error(`  - ${err}`);
    }
    process.exit(1);
  }
  console.log(`PASS: ${result.assetCount} skin font/SVG assets across ${result.skinCount} skins verified.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
