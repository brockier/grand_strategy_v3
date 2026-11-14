#!/usr/bin/env node
/**
 * UI-LIC-003 verifier: validates Chromium & third-party notices, inspects ASAR
 * archive headers to ensure zero native libraries are trapped inside app.asar,
 * checks non-empty unpacked native files, and optionally boots the staged runtime.
 */
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { getRawHeader } from "@electron/asar";

const NATIVE_EXT_RE = /\.(node|so|dll)$/i;

function walkAsarFiles(node, prefix = "", results = []) {
  const files = node?.files ?? {};
  for (const [name, entry] of Object.entries(files)) {
    const rel = prefix ? `${prefix}/${name}` : name;
    if (entry.files) {
      walkAsarFiles(entry, rel, results);
    } else {
      results.push({
        rel,
        unpacked: entry.unpacked === true,
        size: entry.size ?? 0,
      });
    }
  }
  return results;
}

export function inspectSteamStageContents(stageDir) {
  const errors = [];
  const chromiumNotices = path.join(stageDir, "LICENSES.chromium.html");
  if (!fs.existsSync(chromiumNotices) || fs.statSync(chromiumNotices).size === 0) {
    errors.push(`Missing or empty Chromium notices: ${chromiumNotices}`);
  }

  const thirdPartyNotices = path.join(stageDir, "resources", "licenses", "THIRD_PARTY_NOTICES.txt");
  if (!fs.existsSync(thirdPartyNotices) || fs.statSync(thirdPartyNotices).size === 0) {
    errors.push(`Missing or empty engine/mod/UI notices: ${thirdPartyNotices}`);
  }

  const asarPath = path.join(stageDir, "resources", "app.asar");
  const unpackedRoot = path.join(stageDir, "resources", "app.asar.unpacked");
  if (!fs.existsSync(asarPath)) {
    errors.push(`Missing ASAR archive: ${asarPath}`);
    return { ok: false, errors, asarEntries: [] };
  }
  if (!fs.existsSync(unpackedRoot)) {
    errors.push(`Missing ASAR unpacked directory: ${unpackedRoot}`);
  }

  const header = getRawHeader(asarPath);
  const asarEntries = walkAsarFiles(header.header);
  const byRel = new Map(asarEntries.map((e) => [e.rel, e]));

  for (const expectedPacked of ["dist/index.html", "dist-electron/main.js"]) {
    const found = byRel.get(expectedPacked);
    if (!found) {
      errors.push(`Expected entry missing from app.asar: ${expectedPacked}`);
    } else if (found.unpacked) {
      errors.push(`Expected app file should stay packed in app.asar: ${expectedPacked}`);
    }
  }

  for (const entry of asarEntries) {
    const isNative = NATIVE_EXT_RE.test(entry.rel) || entry.rel.startsWith("native/");
    if (isNative && !entry.unpacked) {
      errors.push(`Native binary/library is trapped inside app.asar instead of app.asar.unpacked: ${entry.rel}`);
    }
  }

  const simCliRel = process.platform === "win32" ? "native/bin/sim-cli.exe" : "native/bin/sim-cli";
  const requiredUnpacked = [
    simCliRel,
    "native/addon.node",
    "native/lib/libsteam_api.so",
    "native/lib/steam_api64.dll",
  ];

  for (const rel of requiredUnpacked) {
    const asarRec = byRel.get(rel);
    if (!asarRec || !asarRec.unpacked) {
      errors.push(`Expected unpacked ASAR marker missing for ${rel}`);
    }
    const unpackedFile = path.join(unpackedRoot, rel);
    if (!fs.existsSync(unpackedFile) || fs.statSync(unpackedFile).size === 0) {
      errors.push(`Expected non-empty unpacked file missing on disk: ${unpackedFile}`);
    }
  }

  const simCliDisk = path.join(unpackedRoot, simCliRel);
  if (fs.existsSync(simCliDisk) && process.platform !== "win32") {
    try {
      fs.accessSync(simCliDisk, fs.constants.X_OK);
    } catch {
      errors.push(`Unpacked sim-cli binary is not executable: ${simCliDisk}`);
    }
  }

  return { ok: errors.length === 0, errors, asarEntries };
}

export function launchPackagedElectronSmoke(stageDir) {
  const exeName = process.platform === "win32" ? "electron.exe" : "electron";
  const electronExe = path.join(stageDir, exeName);
  const env = { ...process.env, NG_HEADLESS_TEST: "1" };
  delete env.NG_SIM_CLI_PATH;

  const stdout = execFileSync(electronExe, ["--smoke-mode=healthy"], {
    env,
    encoding: "utf8",
    timeout: 20000,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const smokeLine = stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("SMOKE_RESULT:"));
  if (!smokeLine) {
    throw new Error(`Packaged Electron smoke boot omitted SMOKE_RESULT line.\nOutput:\n${stdout}`);
  }
  const parsed = JSON.parse(smokeLine.slice("SMOKE_RESULT:".length));
  if (!parsed?.initial?.status?.ready || !parsed?.initial?.status?.shaderCompiled || !parsed?.ackGood?.accepted || parsed?.ackBad?.accepted) {
    throw new Error(`Packaged Electron smoke boot assertions failed: ${smokeLine}`);
  }
  return parsed;
}

async function main() {
  const uiRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const stageIdx = process.argv.indexOf("--stage-dir");
  const stageDir = stageIdx !== -1 && process.argv[stageIdx + 1]
    ? path.resolve(process.argv[stageIdx + 1])
    : path.join(uiRoot, "release", "steam-stage");

  const skipLaunch = process.argv.includes("--skip-launch");
  const inspection = inspectSteamStageContents(stageDir);
  if (!inspection.ok) {
    console.error("FAIL: Steam stage package verification failed:");
    for (const err of inspection.errors) {
      console.error(`  - ${err}`);
    }
    process.exit(1);
  }

  let smokeGpu = "skipped";
  if (!skipLaunch) {
    const smoke = launchPackagedElectronSmoke(stageDir);
    smokeGpu = smoke.initial.status.gpuBackend;
  }

  console.log(
    `PASS: Verified Steam package at ${stageDir} (${inspection.asarEntries.length} ASAR entries inspected, packaged smoke GPU=${smokeGpu}).`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error("FAIL: Steam package verification threw an error:", err);
    process.exit(1);
  });
}
