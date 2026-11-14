#!/usr/bin/env node
/**
 * Stages a Steam-ready Electron + app.asar + app.asar.unpacked runtime directory
 * at code/ui/release/steam-stage/ using @electron/asar with strict native unpack rules.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createPackageWithOptions } from "@electron/asar";

function makeElfStub(label) {
  // Minimal valid 64-byte ELF64 shared-object identification prefix + payload tag.
  const buf = Buffer.alloc(128, 0);
  buf[0] = 0x7f;
  buf[1] = 0x45; // 'E'
  buf[2] = 0x4c; // 'L'
  buf[3] = 0x46; // 'F'
  buf[4] = 2; // 64-bit
  buf[5] = 1; // little endian
  buf[6] = 1; // EV_CURRENT
  buf.write(`NG_NATIVE_STUB:${label}`, 16, "utf8");
  return buf;
}

function makePeStub(label) {
  const buf = Buffer.alloc(128, 0);
  buf[0] = 0x4d; // 'M'
  buf[1] = 0x5a; // 'Z'
  buf.write(`NG_DLL_STUB:${label}`, 16, "utf8");
  return buf;
}

export async function stageSteamPackage(options = {}) {
  const uiRoot = options.uiRoot ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const repoRoot = options.repoRoot ?? path.resolve(uiRoot, "../..");
  const stageDir = options.stageDir ?? path.join(uiRoot, "release", "steam-stage");
  const asarSrc = path.join(path.dirname(stageDir), "_asar-src");

  const electronDist = path.join(uiRoot, "node_modules", "electron", "dist");
  const simCliSrc =
    options.simCliPath ??
    path.join(repoRoot, "code", "engine", "target", "debug", process.platform === "win32" ? "sim-cli.exe" : "sim-cli");
  const noticesSrc = path.join(repoRoot, "dist", "THIRD_PARTY_NOTICES.txt");
  const distDir = path.join(uiRoot, "dist");
  const distElectronDir = path.join(uiRoot, "dist-electron");

  for (const required of [electronDist, simCliSrc, noticesSrc, distDir, distElectronDir]) {
    if (!fs.existsSync(required)) {
      throw new Error(`Required build/compliance input missing: ${required}`);
    }
  }

  fs.rmSync(stageDir, { recursive: true, force: true });
  fs.rmSync(asarSrc, { recursive: true, force: true });
  fs.mkdirSync(stageDir, { recursive: true });
  fs.mkdirSync(asarSrc, { recursive: true });

  // Copy Electron distribution runtime (includes LICENSES.chromium.html & electron executable).
  fs.cpSync(electronDist, stageDir, { recursive: true, verbatimSymlinks: true });

  const resourcesDir = path.join(stageDir, "resources");
  fs.rmSync(path.join(resourcesDir, "default_app.asar"), { force: true });

  // Prepare ASAR source tree with built frontend, main/preload scripts, and native binaries.
  const pkgJson = JSON.parse(fs.readFileSync(path.join(uiRoot, "package.json"), "utf8"));
  fs.writeFileSync(
    path.join(asarSrc, "package.json"),
    JSON.stringify({ name: pkgJson.name, version: pkgJson.version, main: "dist-electron/main.js" }, null, 2),
  );
  fs.cpSync(distDir, path.join(asarSrc, "dist"), { recursive: true });
  fs.cpSync(distElectronDir, path.join(asarSrc, "dist-electron"), { recursive: true });

  const nativeBinDir = path.join(asarSrc, "native", "bin");
  const nativeLibDir = path.join(asarSrc, "native", "lib");
  fs.mkdirSync(nativeBinDir, { recursive: true });
  fs.mkdirSync(nativeLibDir, { recursive: true });

  const simCliName = process.platform === "win32" ? "sim-cli.exe" : "sim-cli";
  const stagedSimCli = path.join(nativeBinDir, simCliName);
  fs.copyFileSync(simCliSrc, stagedSimCli);
  fs.chmodSync(stagedSimCli, 0o755);

  fs.writeFileSync(path.join(asarSrc, "native", "addon.node"), makeElfStub("addon.node"));
  fs.writeFileSync(path.join(nativeLibDir, "libsteam_api.so"), makeElfStub("libsteam_api.so"));
  fs.writeFileSync(path.join(nativeLibDir, "steam_api64.dll"), makePeStub("steam_api64.dll"));

  await createPackageWithOptions(asarSrc, path.join(resourcesDir, "app.asar"), {
    unpack: "{*.node,*.so,*.dll,**/sim-cli,**/sim-cli.exe}",
    unpackDir: "native",
  });

  fs.rmSync(asarSrc, { recursive: true, force: true });

  // Copy external compliance & data resources alongside app.asar.
  fs.mkdirSync(path.join(resourcesDir, "licenses"), { recursive: true });
  fs.copyFileSync(noticesSrc, path.join(resourcesDir, "licenses", "THIRD_PARTY_NOTICES.txt"));
  fs.cpSync(path.join(repoRoot, "mods"), path.join(resourcesDir, "mods"), { recursive: true });
  fs.cpSync(path.join(repoRoot, "skins"), path.join(resourcesDir, "skins"), { recursive: true });

  return { stageDir, resourcesDir };
}

async function main() {
  const { stageDir } = await stageSteamPackage();
  console.log(`PASS: Staged Steam package at ${stageDir}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error("FAIL: Could not stage Steam package:", err);
    process.exit(1);
  });
}
