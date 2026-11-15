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

describe("Option B & Milestones M5-M7 Live Chromium Electron End-to-End Integration", () => {
  // @req UI-GLB-001.1
  // @req UI-MAP-005.1
  // @req UI-MAP-006.1
  // @req UI-MAP-006.2
  // @req UI-MAP-007.1
  // @req UI-MAP-007.2
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
      tradeLegend: string;
      warfareLegend: string;
      hasInstTablist: boolean;
      inactivePanelHiddenAttr: string | null;
      inactivePanelPadding: string;
      instSummaryHasHeading: boolean;
      updatedGovModel: string | null;
      knowledgeBadges: number;
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

    expect(result.tradeLegend.length).toBeGreaterThan(0);
    expect(result.warfareLegend).toContain("Theaters");

    expect(result.hasInstTablist).toBe(true);
    expect(result.inactivePanelHiddenAttr).toBe("until-found");
    expect(result.inactivePanelPadding).toBe("0px");
    expect(result.instSummaryHasHeading).toBe(false);
    expect(result.updatedGovModel).toBe("constitutional_cabinet");

    expect(result.knowledgeBadges).toBeGreaterThanOrEqual(0);
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
  });
});
