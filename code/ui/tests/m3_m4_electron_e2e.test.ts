// @req UI-MAP-001
// @req UI-DAT-001
// @req UI-DAT-002
// @req UI-DAT-003
// @req UI-DAT-005
// @req UI-HUD-002
// @req UI-HUD-005
// @req UI-HUD-006
// @req UI-AUD-003

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, describe, expect, it } from "vitest";

const UI_ROOT = path.resolve(__dirname, "..");
const ELECTRON_BIN = path.join(UI_ROOT, "node_modules", ".bin", "electron");
const MAIN_JS = path.join(UI_ROOT, "dist-electron", "main.js");
const DIST_INDEX = path.join(UI_ROOT, "dist", "index.html");

const TEMP_DIR = fs.mkdtempSync(path.join(os.tmpdir(), "ng-m3-m4-electron-e2e-"));
const PREFS_PATH = path.join(TEMP_DIR, "user_preferences.json");

function runM3M4Smoke(
  phase: "m3-m4-phase1" | "m3-m4-phase2",
  prefsPath: string,
): Record<string, unknown> {
  const stdout = execFileSync(ELECTRON_BIN, [MAIN_JS, `--smoke-mode=${phase}`], {
    cwd: UI_ROOT,
    env: {
      ...process.env,
      NG_HEADLESS_TEST: "1",
      NG_SOFTWARE_GL: "1",
      NG_USER_PREFS_PATH: prefsPath,
    },
    encoding: "utf8",
    timeout: 45000,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const line = stdout.split(/\r?\n/).find((l) => l.startsWith("M3_M4_SMOKE_RESULT:"));
  if (!line) {
    throw new Error(`Missing M3_M4_SMOKE_RESULT line in stdout:\n${stdout}`);
  }
  return JSON.parse(line.slice("M3_M4_SMOKE_RESULT:".length)) as Record<string, unknown>;
}

describe("Electron real-process M3+M4 live sim-cli integration and restart persistence", () => {
  afterAll(() => {
    fs.rmSync(TEMP_DIR, { recursive: true, force: true });
  });

  it("drives overlays, virtualized ledgers, charts, org tree, HQ route preview, live mandate sealing, foley cues, and restart persistence", () => {
    expect(fs.existsSync(MAIN_JS)).toBe(true);
    expect(fs.existsSync(DIST_INDEX)).toBe(true);

    const phase1 = runM3M4Smoke("m3-m4-phase1", PREFS_PATH);
    expect(phase1.phase).toBe("m3-m4-phase1");
    expect(phase1.hasNodeRequire).toBe(false);
    expect(phase1.hasProcessGlobal).toBe(false);

    expect(Number(phase1.initialCellCount)).toBeGreaterThan(0);
    expect(phase1.afterWeatherMode).toBe("weather");
    expect(phase1.afterLogisticsMode).toBe("logistics");
    expect(phase1.afterEpistemicMode).toBe("epistemic");
    expect(String(phase1.weatherLegendText).length).toBeGreaterThan(0);

    expect(Number(phase1.ledgerRowCount)).toBeGreaterThan(0);
    expect(Number(phase1.ledgerRowCount)).toBeLessThan(80);
    expect(Number(phase1.provBadgeCount)).toBeGreaterThan(0);
    expect(Number(phase1.cohortRowCount)).toBeGreaterThan(0);
    expect(Number(phase1.cohortRowCount)).toBeLessThan(80);

    expect(phase1.chartCanvasPresent).toBe(true);
    expect(Number(phase1.orgCardsCount)).toBeGreaterThan(0);

    expect(String(phase1.relocPreviewText).length).toBeGreaterThan(0);
    expect(phase1.previewTargetBeforeClose).not.toBeNull();
    expect(String(phase1.mandatePreviewText).length).toBeGreaterThan(0);

    expect(Number(phase1.finalTick)).toBe(3);
    expect(Number(phase1.activeMandateCount)).toBeGreaterThanOrEqual(1);
    expect(phase1.persistedRuleInSnapshot).toBe("DispatchPopupAndAutoPause");
    expect(phase1.persistedRuleInPrefs).toBe("DispatchPopupAndAutoPause");

    const foleyLog = Array.isArray(phase1.foleyLog) ? (phase1.foleyLog as string[]) : [];
    expect(foleyLog).toContain("clock_step");
    expect(foleyLog).toContain("mandate_seal");

    const phase2 = runM3M4Smoke("m3-m4-phase2", PREFS_PATH);
    expect(phase2.phase).toBe("m3-m4-phase2");
    expect(phase2.persistedPrefRule).toBe("DispatchPopupAndAutoPause");
    expect(phase2.snapshotRule).toBe("DispatchPopupAndAutoPause");
    expect(phase2.matrixSelectValue).toBe("DispatchPopupAndAutoPause");
  }, 90000);
});
