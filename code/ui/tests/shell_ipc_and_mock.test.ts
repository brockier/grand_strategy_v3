import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import { describe, expect, it, vi } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  MockSnapshotPlayer,
  isValidDiegeticSnapshot,
  selectClockAndNotifications,
  selectDelegationTree,
  selectDispatchInbox,
  selectGlobeTopology,
  selectHqPresence,
  selectKnownCells,
  selectLedgerRows,
  selectObservedWeather,
  selectTrackedConvoys,
  validatePlayerCommand,
} from "../src/spine";
import { ModDiagnosticDialog } from "../src/windows";

const UI_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(UI_ROOT, "../..");
const SIM_CLI_BIN = path.join(
  REPO_ROOT,
  "code",
  "engine",
  "target",
  "debug",
  process.platform === "win32" ? "sim-cli.exe" : "sim-cli",
);

describe("UI Spine Contracts, Air-Gap Parity & Mock Snapshot Player", () => {
  // @req UI-SHL-002
  it("validates live sim-cli DiegeticSnapshot parity and rejects air-gap leaks or malformed commands (UI-SHL-002)", () => {
    const rawJson = execFileSync(
      SIM_CLI_BIN,
      [
        "dump-snapshot",
        "--pack",
        path.join(REPO_ROOT, "mods", "core-schemas"),
        "--pack",
        path.join(REPO_ROOT, "mods", "baseline-era"),
        "--pack",
        path.join(REPO_ROOT, "mods", "unexplained-colony"),
        "--ticks",
        "0",
        "--seed",
        "42",
      ],
      { encoding: "utf8" },
    );

    const parsedSnapshot = JSON.parse(rawJson) as DiegeticSnapshot;
    expect(isValidDiegeticSnapshot(parsedSnapshot)).toBe(true);

    const store = new DiegeticStore();
    store.ingestSnapshot(parsedSnapshot);

    const known = selectKnownCells(store);
    const unexplored = known.filter((c) => c.tier === "unexplored");
    const surveyed = known.filter((c) => c.tier === "surveyed");
    expect(surveyed.length).toBeGreaterThan(0);
    expect(unexplored.length).toBeGreaterThan(0);
    for (const cell of unexplored) {
      expect("elevationM" in cell).toBe(false);
      expect("confidence01" in cell).toBe(false);
      expect("observedTick" in cell).toBe(false);
    }

    // Air-gap violation fixture must be rejected loudly by store guard.
    const leakedPayload = structuredClone(parsedSnapshot) as unknown as {
      knownCells: Array<Record<string, unknown>>;
    };
    const firstUnexplored = leakedPayload.knownCells.find((c) => c.tier === "unexplored");
    expect(firstUnexplored).toBeDefined();
    if (firstUnexplored) {
      firstUnexplored.elevationM = 999.0;
    }
    expect(isValidDiegeticSnapshot(leakedPayload)).toBe(false);
    expect(() => store.ingestSnapshot(leakedPayload as unknown as DiegeticSnapshot)).toThrow(/air-gap/i);

    // Command allowlist parity checks.
    expect(validatePlayerCommand({ kind: "setPaused", paused: true })).toEqual({
      kind: "setPaused",
      paused: true,
    });
    expect(validatePlayerCommand({ kind: "setSpeedMultiplier", multiplier: 99 })).toBeNull();
    expect(validatePlayerCommand({ kind: "evilArbitraryEval", payload: "1+1" })).toBeNull();
  });

  // @req UI-SHL-005
  it("runs MockSnapshotPlayer completely offline with zero Rust processes and populates all 9 selectors (UI-SHL-005)", async () => {
    const startMs = Date.now();
    const fixturePath = path.join(UI_ROOT, "tests", "fixtures", "colony_m0_snapshot.json");
    const timeline = JSON.parse(fs.readFileSync(fixturePath, "utf8")) as DiegeticSnapshot[];
    expect(timeline.length).toBe(2);

    const player = new MockSnapshotPlayer(timeline);
    const store = new DiegeticStore();
    const unsubscribe = player.subscribeSnapshots((snap) => store.ingestSnapshot(snap));

    const boot = await player.getInitialState();
    expect(boot.state).toBe("ready");

    // Tick 0 selectors check (covering all 9 DiegeticStore selectors).
    expect(selectGlobeTopology(store)?.cellCount).toBe(642);
    expect(selectKnownCells(store).length).toBe(6);
    expect(selectHqPresence(store)?.settlementName).toBe("First Landing Perimeter");
    expect(selectTrackedConvoys(store)).toHaveLength(0);
    expect(selectObservedWeather(store)).toHaveLength(1);
    expect(selectLedgerRows(store)).toHaveLength(2);
    expect(selectLedgerRows(store, 1)).toHaveLength(1);
    expect(selectDelegationTree(store)?.subordinates).toHaveLength(2);
    expect(selectDispatchInbox(store)).toHaveLength(0);
    expect(selectClockAndNotifications(store)?.tick).toBe(0);

    // Step to tick 12 deterministically.
    player.stepMockSnapshot(1);
    expect(selectClockAndNotifications(store)?.tick).toBe(12);
    expect(selectTrackedConvoys(store)).toHaveLength(1);
    expect(selectDispatchInbox(store)).toHaveLength(1);
    expect(selectLedgerRows(store, 0)[0].reportedMassKg).toBe(2790.0);

    // Command validation works deterministically offline.
    const ackOk = await player.sendPlayerCommand({ kind: "setPaused", paused: true });
    expect(ackOk.accepted).toBe(true);
    expect(player.recordedCommands).toHaveLength(1);

    const ackErr = await player.sendPlayerCommand({ kind: "bogusCmd" } as never);
    expect(ackErr.accepted).toBe(false);

    unsubscribe();
    expect(Date.now() - startMs).toBeLessThan(10000);
  });

  // @req UI-SHL-004
  it("renders ModDiagnosticDialog using accessible semantic DOM and safe textContent (UI-SHL-004)", () => {
    const dom = new JSDOM("<!doctype html><html><body><div id='host'></div></body></html>");
    const prevDoc = globalThis.document;
    globalThis.document = dom.window.document;
    try {
      const host = dom.window.document.getElementById("host") as HTMLElement;
      const reloadSpy = vi.fn(async () => undefined);
      const revertSpy = vi.fn(async () => undefined);
      const dlg = new ModDiagnosticDialog(host, {
        onReloadMods: reloadSpy,
        onRevertDefault: revertSpy,
      });

      const dialogEl = dom.window.document.getElementById("mod-diagnostic-dialog") as HTMLDialogElement;
      const showModalSpy = vi.fn(() => {
        dialogEl.setAttribute("open", "");
      });
      const closeSpy = vi.fn((val?: string) => {
        dialogEl.returnValue = val ?? "";
        dialogEl.removeAttribute("open");
        dialogEl.dispatchEvent(new dom.window.Event("close"));
      });
      dialogEl.showModal = showModalSpy;
      dialogEl.close = closeSpy;

      dlg.showDiagnostics({
        ok: false,
        diagnostics: [
          {
            severity: "Error",
            layer: "ScenarioPack",
            filePath: "mods/bad/scenario.toml",
            line: 7,
            column: 4,
            fieldPath: "starting_resources.<script>alert(1)</script>",
            message: "Unknown commodity reference",
          },
        ],
      });

      expect(showModalSpy).toHaveBeenCalledTimes(1);
      expect(dlg.isOpen()).toBe(true);
      const rows = dialogEl.querySelectorAll("tbody tr");
      expect(rows).toHaveLength(1);
      expect(rows[0].querySelector("th[scope='row']")?.textContent).toBe("mods/bad/scenario.toml");
      expect(dialogEl.querySelectorAll("script")).toHaveLength(0);

      dom.window.document.getElementById("mod-diag-revert-btn")?.click();
      expect(revertSpy).toHaveBeenCalledTimes(1);

      dlg.hide();
      expect(closeSpy).toHaveBeenCalledWith("recovered");
      expect(dialogEl.returnValue).toBe("");
      expect(dlg.isOpen()).toBe(false);
    } finally {
      globalThis.document = prevDoc;
    }
  });
});
