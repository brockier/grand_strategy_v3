import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const UI_ROOT = path.resolve(__dirname, "..");
const ELECTRON_BIN = path.join(UI_ROOT, "node_modules", ".bin", "electron");
const MAIN_JS = path.join(UI_ROOT, "dist-electron", "main.js");

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

function countChangedBytes(a: number[], b: number[]): number {
  const len = Math.min(a.length, b.length);
  let diff = 0;
  for (let i = 0; i < len; i += 1) {
    if (Math.abs(a[i] - b[i]) > 2) diff += 1;
  }
  return diff;
}

function runM2Smoke(phase: "m2-phase1" | "m2-phase2", prefsPath: string): Record<string, unknown> {
  const stdout = execFileSync(ELECTRON_BIN, [MAIN_JS, `--smoke-mode=${phase}`], {
    cwd: UI_ROOT,
    env: {
      ...process.env,
      NG_HEADLESS_TEST: "1",
      NG_SOFTWARE_GL: "1",
      NG_USER_PREFS_PATH: prefsPath,
    },
    encoding: "utf8",
    timeout: 30000,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const line = stdout.split(/\r?\n/).find((l) => l.startsWith("M2_SMOKE_RESULT:"));
  if (!line) {
    throw new Error(`Missing M2_SMOKE_RESULT in Electron stdout:\n${stdout}`);
  }
  return JSON.parse(line.slice("M2_SMOKE_RESULT:".length)) as Record<string, unknown>;
}

describe("Milestone 2 Headless Chromium Electron End-to-End Integration", () => {
  // @req UI-GLB-001
  // @req UI-GLB-002
  // @req UI-WIN-005
  // @req UI-SKN-002
  // @req UI-SKN-003
  // @req UI-SKN-004
  // @req UI-AUD-004
  it("drives live WebGL2 globe, window isolation, 9-slice rendering, skin switching, and cross-restart audio/layout persistence in real Electron", () => {
    const tempDir = makeTempDir("ng-m2-e2e-");
    const prefsPath = path.join(tempDir, "user_preferences.json");

    // Launch 1: Real WebGL2 GPU rendering, real pointer drag, real wheel occlusion, skin swap, and save prefs
    const p1 = runM2Smoke("m2-phase1", prefsPath);
    const preDrag = p1.preDrag as {
      skinId: string;
      hudBg: string;
      hudFont: string;
      focusStart: number[];
      rgbaStart: number[];
    };
    const gpuZoomAndDrag = p1.gpuZoomAndDrag as {
      focusAfterDrag: number[];
      rgbaAfterDrag: number[];
      surfaceBlend: number;
      orbitalBlend: number;
      rgbaSurface: number[];
      rgbaOrbital: number[];
    };
    const winSetup = p1.winSetup as {
      initialTick: number;
      afterStepTick: number;
      smallBorderImage: string;
      smallBorderSlice: string;
      smallBorderWidth: string;
      largeBorderImage: string;
      largeBorderWidth: string;
      preservedFilter: string;
      altBeforeWheelOverWindow: number;
    };
    const skinAndAudio = p1.skinAndAudio as {
      altAfterWheelOverWindow: number;
      openWindowsAfterSkinSwap: number;
      skinAfterSwap: string;
      hudBgBrass: string;
      hudFontBrass: string;
      rgbaBeforeSkin: number[];
      rgbaAfterSkin: number[];
      audioNodes: { master: number; ambience: number };
      persistedPrefs: {
        activeSkinId: string;
        audioMixer: { masterVolume: number; ambienceMuted: boolean };
      };
    };

    // 1. GPU pixel buffer changed after genuine mouse drag across sphere (`UI-GLB-001`)
    expect(preDrag.skinId).toBe("cartographer-parchment");
    expect(preDrag.hudFont).toContain("Liberation Serif");
    expect(countChangedBytes(preDrag.rgbaStart, gpuZoomAndDrag.rgbaAfterDrag)).toBeGreaterThan(0);
    expect(Math.hypot(
      gpuZoomAndDrag.focusAfterDrag[0] - preDrag.focusStart[0],
      gpuZoomAndDrag.focusAfterDrag[1] - preDrag.focusStart[1],
      gpuZoomAndDrag.focusAfterDrag[2] - preDrag.focusStart[2],
    )).toBeGreaterThan(0.05);

    // 2. Surface zoom pixels & cartographicBlend (0 -> 1) differ from orbital cartographic map (`UI-GLB-002`)
    expect(gpuZoomAndDrag.surfaceBlend).toBe(0);
    expect(gpuZoomAndDrag.orbitalBlend).toBe(1);
    expect(countChangedBytes(gpuZoomAndDrag.rgbaSurface, gpuZoomAndDrag.rgbaOrbital)).toBeGreaterThan(0);

    // 3. +1 Hour HUD button stepped simulation cleanly & window state survived collapse
    expect(winSetup.afterStepTick).toBeGreaterThan(winSetup.initialTick);
    expect(winSetup.preservedFilter).toBe("grain");

    // 4. 9-slice border-image CSS active cleanly at both compact (320x220) and large (840x560) aspect ratios (`UI-SKN-002`)
    expect(winSetup.smallBorderImage).toContain("window_9slice.png");
    expect(winSetup.smallBorderSlice).toContain("16");
    expect(winSetup.smallBorderWidth).toBe("16px");
    expect(winSetup.largeBorderImage).toContain("window_9slice.png");
    expect(winSetup.largeBorderWidth).toBe("16px");

    // 5. Real Chromium wheel over open ledger window did NOT zoom underlying globe (`UI-WIN-005`)
    expect(skinAndAudio.altAfterWheelOverWindow).toBeCloseTo(winSetup.altBeforeWheelOverWindow, 5);

    // 6. Live skin switch updated DOM typography/palette AND WebGL GPU pixels while keeping open windows intact (`UI-SKN-003`, `UI-SKN-004`)
    expect(skinAndAudio.skinAfterSwap).toBe("industrial-brass");
    expect(skinAndAudio.openWindowsAfterSkinSwap).toBeGreaterThanOrEqual(2);
    expect(skinAndAudio.hudBgBrass).not.toBe(preDrag.hudBg);
    expect(skinAndAudio.hudFontBrass).toContain("Liberation Mono");
    expect(countChangedBytes(skinAndAudio.rgbaBeforeSkin, skinAndAudio.rgbaAfterSkin)).toBeGreaterThan(0);

    // 7. Audio nodes & persisted prefs updated cleanly (`UI-AUD-004`)
    expect(skinAndAudio.audioNodes.master).toBeCloseTo(0.35, 2);
    expect(skinAndAudio.audioNodes.ambience).toBe(0);
    expect(fs.existsSync(prefsPath)).toBe(true);

    // Launch 2: Fresh Electron boot reading persisted NG_USER_PREFS_PATH (`UI-AUD-004`, `UI-WIN-007`)
    const p2 = runM2Smoke("m2-phase2", prefsPath) as {
      activeSkinId: string;
      documentSkinAttr: string;
      audioSettings: { masterVolume: number; ambienceMuted: boolean };
      audioNodes: { master: number; ambience: number };
      presets: string[];
      appliedSavedPreset: boolean;
      restoredFrameCount: number;
    };

    expect(p2.activeSkinId).toBe("industrial-brass");
    expect(p2.documentSkinAttr).toBe("industrial-brass");
    expect(p2.audioSettings.masterVolume).toBeCloseTo(0.35, 2);
    expect(p2.audioSettings.ambienceMuted).toBe(true);
    expect(p2.audioNodes.master).toBeCloseTo(0.35, 2);
    expect(p2.audioNodes.ambience).toBe(0);
    expect(p2.presets).toContain("e2e-saved-desk");
    expect(p2.appliedSavedPreset).toBe(true);
    expect(p2.restoredFrameCount).toBeGreaterThanOrEqual(2);
  });
});
