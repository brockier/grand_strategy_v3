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

describe("Option B & Milestones M5-M7 Live Chromium Electron End-to-End Integration", () => {
  // @req UI-GLB-001.1
  // @req UI-GLB-006
  // @req UI-GLB-007
  // @req UI-MAP-005.1
  // @req UI-MAP-006.1
  // @req UI-MAP-006.2
  // @req UI-MAP-007.1
  // @req UI-MAP-007.2
  // @req UI-MAP-008
  // @req UI-DAT-006.1
  // @req UI-DAT-006.2
  // @req UI-DAT-007.1
  // @req UI-DAT-007.2
  // @req UI-HUD-007.1
  // @req UI-HUD-007.2
  it("drives live serve-stdio clock, trade/warfare overlays, institutional governance, empirical archive, IPC prohibitions, and native modal <dialog> succession", () => {
    const tempDir = makeTempDir("ng-m5-m6-m7-e2e-");
    const prefsPath = path.join(tempDir, "user_preferences.json");

    const launchArgs = [
      ...(process.platform === "linux"
        ? ["--ozone-platform=headless", "--enable-unsafe-swiftshader"]
        : []),
      MAIN_JS,
      "--smoke-mode=m5-m6-m7",
    ];
    const stdout = execFileSync(ELECTRON_BIN, launchArgs, {
      cwd: UI_ROOT,
      env: {
        ...process.env,
        NG_HEADLESS_TEST: "1",
        NG_SOFTWARE_GL: "1",
        NG_USER_PREFS_PATH: prefsPath,
      },
      encoding: "utf8",
      timeout: 35000,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const line = stdout.split(/\r?\n/).find((l) => l.startsWith("M5_M6_M7_SMOKE_RESULT:"));
    if (!line) {
      throw new Error(`Missing M5_M6_M7_SMOKE_RESULT in Electron stdout:\n${stdout}`);
    }
    const result = JSON.parse(line.slice("M5_M6_M7_SMOKE_RESULT:".length)) as {
      phase: string;
      initialTick: number;
      initialPaused: boolean;
      initialGovModel: string | null;
      actorCount: number;
      afterUnpauseTick: number;
      repausedState: boolean;
      topologyCellCount: number;
      shaderCompiled: boolean;
      renderedLabelCount: number;
      terrainSample: number[];
      parchmentSample: number[];
      projEqualArea: string;
      equalAreaSample: number[];
      projMercator: string;
      mercatorSample: number[];
      projRestoredGlobe: string;
      closePitchRad: number;
      closeLod4Count: number;
      close3dSample: number[];
      tradeLegend: string;
      warfareLegend: string;
      hasInstTablist: boolean;
      inactivePanelHiddenAttr: string | null;
      inactivePanelPadding: string;
      instSummaryHasHeading: boolean;
      updatedGovModel: string | null;
      knowledgeBadges: number;
      knowledgeRowCount: number;
      knowledgeCardEnclosesRows: boolean;
      knowledgeAllRowsVisibleOnOpen: boolean;
      hasTechTreeText: boolean;
      modeTarget: string;
      updatedInnovationMode: string | null;
      churchillPreview: string;
      hitlerPreview: string;
      updatedTheaterObjective: number | null;
      forbiddenMicroAck: { accepted: boolean; errorCode: string | null };
      forbiddenTechAck: { accepted: boolean; errorCode: string | null };
      dialogOpenBeforeAck: boolean;
      dialogMatchesModalBeforeAck: boolean;
      dialogRoleAttr: string | null;
      dialogAriaModalAttr: string | null;
      dialogAriaLabelledBy: string | null;
      dialogLossText: string;
      dialogOpenAfterAck: boolean;
      dialogMatchesModalAfterAck: boolean;
      dialogReturnValueAfterAck: string;
      postAckPending: unknown;
    };

    expect(result.phase).toBe("m5-m6-m7");
    expect(result.initialPaused).toBe(true);
    expect(result.afterUnpauseTick).toBeGreaterThan(result.initialTick);
    expect(result.repausedState).toBe(true);
    expect(result.actorCount).toBeGreaterThan(0);

    expect(result.topologyCellCount).toBe(5762);
    expect(result.shaderCompiled).toBe(true);
    expect(result.renderedLabelCount).toBeGreaterThanOrEqual(1);
    expect(result.terrainSample.length).toBeGreaterThanOrEqual(800);
    expect(countChangedBytes(result.terrainSample, result.parchmentSample)).toBeGreaterThan(0);
    expect(result.projEqualArea).toBe("equal_area");
    expect(result.projMercator).toBe("mercator");
    expect(result.projRestoredGlobe).toBe("globe");
    expect(countChangedBytes(result.terrainSample, result.equalAreaSample)).toBeGreaterThan(0);
    expect(countChangedBytes(result.equalAreaSample, result.mercatorSample)).toBeGreaterThan(0);
    expect(result.closePitchRad).toBeGreaterThan(0.65);
    expect(result.closeLod4Count).toBeGreaterThan(0);
    expect(result.close3dSample.length).toBeGreaterThanOrEqual(800);
    expect(countChangedBytes(result.terrainSample, result.close3dSample)).toBeGreaterThan(0);

    expect(result.tradeLegend.length).toBeGreaterThan(0);
    expect(result.warfareLegend).toContain("Theaters");

    expect(result.hasInstTablist).toBe(true);
    expect(result.inactivePanelHiddenAttr).toBe("until-found");
    expect(result.inactivePanelPadding).toBe("0px");
    expect(result.instSummaryHasHeading).toBe(false);
    expect(result.updatedGovModel).toBe("constitutional_cabinet");

    expect(result.knowledgeBadges).toBeGreaterThanOrEqual(0);
    expect(result.knowledgeRowCount).toBeGreaterThanOrEqual(6);
    expect(result.knowledgeCardEnclosesRows).toBe(true);
    expect(result.knowledgeAllRowsVisibleOnOpen).toBe(true);
    expect(result.hasTechTreeText).toBe(false);
    if (result.modeTarget) {
      expect(result.updatedInnovationMode).toBe(result.modeTarget);
    }

    expect(result.churchillPreview).toContain("Low");
    expect(result.hitlerPreview).toContain("High");
    expect(result.updatedTheaterObjective).toBe(4);

    expect(result.forbiddenMicroAck.accepted).toBe(false);
    expect(result.forbiddenMicroAck.errorCode).toBe("ERR_TACTICAL_MICRO_FORBIDDEN");
    expect(result.forbiddenTechAck.accepted).toBe(false);
    expect(result.forbiddenTechAck.errorCode).toBe("ERR_BOUGHT_TECH_FORBIDDEN");

    expect(result.dialogOpenBeforeAck).toBe(true);
    expect(result.dialogMatchesModalBeforeAck).toBe(true);
    expect(result.dialogRoleAttr).toBeNull();
    expect(result.dialogAriaModalAttr).toBeNull();
    expect(result.dialogAriaLabelledBy).toBe("succession-dialog-title");
    expect(result.dialogLossText).toContain("3");
    expect(result.dialogOpenAfterAck).toBe(false);
    expect(result.dialogMatchesModalAfterAck).toBe(false);
    expect(result.dialogReturnValueAfterAck).toBe("");
    expect(result.postAckPending).toBeNull();
  }, 45000);
});
