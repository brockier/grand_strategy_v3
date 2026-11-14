import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { createPackageWithOptions } from "@electron/asar";
import { isSpdxAllowed, verifyNpmLockfile } from "../scripts/check-npm-licenses.mjs";
import { verifySkinLicenses } from "../scripts/check-skin-licenses.mjs";
import { stageSteamPackage } from "../scripts/stage-steam-package.mjs";
import { inspectSteamStageContents, launchPackagedElectronSmoke } from "../scripts/verify-steam-package.mjs";

const UI_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(UI_ROOT, "../..");

const tempDirs: string[] = [];
function makeTempDir(prefix: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir) fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("M0 UI Licensing & Steam Packaging Compliance", () => {
  // @req UI-LIC-001
  it("audits production npm lockfile entries and rejects disallowed licenses (UI-LIC-001)", () => {
    const prodAudit = verifyNpmLockfile(path.join(UI_ROOT, "package-lock.json"));
    expect(prodAudit.ok).toBe(true);
    expect(prodAudit.checkedCount).toBeGreaterThanOrEqual(1);
    expect(prodAudit.violations).toEqual([]);

    expect(isSpdxAllowed("MIT")).toBe(true);
    expect(isSpdxAllowed("Apache-2.0")).toBe(true);
    expect(isSpdxAllowed("GPL-3.0-only")).toBe(false);

    const poisonedLock = makeTempDir("ng-bad-npm-");
    const poisonedPath = path.join(poisonedLock, "package-lock.json");
    fs.writeFileSync(
      poisonedPath,
      JSON.stringify({
        name: "poisoned",
        lockfileVersion: 3,
        packages: {
          "": { name: "poisoned" },
          "node_modules/good-pkg": { version: "1.0.0", license: "MIT", dev: false },
          "node_modules/gpl-leak": { version: "0.9.0", license: "GPL-3.0-only", dev: false },
        },
      }),
    );

    const failedAudit = verifyNpmLockfile(poisonedPath);
    expect(failedAudit.ok).toBe(false);
    expect(failedAudit.violations).toHaveLength(1);
    expect(failedAudit.violations[0].packagePath).toBe("node_modules/gpl-leak");
  });

  // @req UI-LIC-002
  it("verifies all bundled skin font and SVG assets have permissive SPDX manifests (UI-LIC-002)", () => {
    const liveAudit = verifySkinLicenses(path.join(REPO_ROOT, "skins"));
    expect(liveAudit.ok).toBe(true);
    expect(liveAudit.skinCount).toBeGreaterThanOrEqual(1);
    expect(liveAudit.assetCount).toBeGreaterThanOrEqual(4);
    expect(liveAudit.errors).toEqual([]);

    const badSkinsRoot = makeTempDir("ng-bad-skin-");
    const sampleSkin = path.join(badSkinsRoot, "broken-skin");
    fs.mkdirSync(path.join(sampleSkin, "fonts"), { recursive: true });
    fs.mkdirSync(path.join(sampleSkin, "licenses"), { recursive: true });
    fs.writeFileSync(path.join(sampleSkin, "fonts", "uncatalogued.ttf"), "FAKE_TTF_DATA");
    fs.writeFileSync(path.join(sampleSkin, "licenses", "baseline_assets.toml"), "schema_version = 1\n");

    const badAudit = verifySkinLicenses(badSkinsRoot);
    expect(badAudit.ok).toBe(false);
    expect(badAudit.errors.some((e) => e.includes("uncatalogued.ttf"))).toBe(true);
  });

  // @req UI-LIC-003
  it("verifies Steam stage ASAR unpack rules, third-party notices, and packaged Chromium boot (UI-LIC-003)", async () => {
    const { stageDir } = await stageSteamPackage({
      uiRoot: UI_ROOT,
      repoRoot: REPO_ROOT,
      stageDir: path.join(UI_ROOT, "release", "steam-stage"),
    });

    const inspection = inspectSteamStageContents(stageDir);
    expect(inspection.ok).toBe(true);
    expect(inspection.errors).toEqual([]);

    const smoke = launchPackagedElectronSmoke(stageDir);
    expect(smoke.initial.status.ready).toBe(true);
    expect(smoke.initial.status.hasSimBridge).toBe(true);
    expect(smoke.initial.status.hasNodeRequire).toBe(false);
    expect(smoke.initial.status.shaderCompiled).toBe(true);

    // Negative check: verify inspector catches a poisoned .asar packing a native binary without unpacking it.
    const poisonRoot = makeTempDir("ng-poison-asar-");
    const srcTree = path.join(poisonRoot, "src");
    const outStage = path.join(poisonRoot, "stage");
    fs.mkdirSync(path.join(srcTree, "dist"), { recursive: true });
    fs.mkdirSync(path.join(srcTree, "dist-electron"), { recursive: true });
    fs.mkdirSync(path.join(srcTree, "native", "lib"), { recursive: true });
    fs.mkdirSync(path.join(outStage, "resources", "licenses"), { recursive: true });
    fs.writeFileSync(path.join(outStage, "LICENSES.chromium.html"), "<html>Chromium</html>");
    fs.writeFileSync(path.join(outStage, "resources", "licenses", "THIRD_PARTY_NOTICES.txt"), "MIT");
    fs.writeFileSync(path.join(srcTree, "dist", "index.html"), "<html></html>");
    fs.writeFileSync(path.join(srcTree, "dist-electron", "main.js"), "// main");
    fs.writeFileSync(path.join(srcTree, "native", "lib", "libsteam_api.so"), Buffer.from([0x7f, 0x45, 0x4c, 0x46]));

    await createPackageWithOptions(srcTree, path.join(outStage, "resources", "app.asar"), {});
    const poisonResult = inspectSteamStageContents(outStage);
    expect(poisonResult.ok).toBe(false);
    expect(poisonResult.errors.some((e) => e.includes("trapped inside app.asar"))).toBe(true);
  });
});
