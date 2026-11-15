import type { BrowserWindow } from "electron";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function runM3M4E2ESmokePhase(
  mainWindow: BrowserWindow,
  phase: string,
): Promise<Record<string, unknown>> {
  for (let i = 0; i < 120; i += 1) {
    const ready = await mainWindow.webContents.executeJavaScript(
      "Boolean(window.__ngBootStatus?.ready && window.__ngM2Runtime)",
    );
    if (ready) break;
    await delay(40);
  }

  if (phase === "m3-m4-phase2") {
    return (await mainWindow.webContents.executeJavaScript(`(async () => {
      const rt = window.__ngM2Runtime;
      await new Promise((r) => setTimeout(r, 80));
      const snap = rt.store.getSnapshot();
      const prefs = await window.simBridge.readUserPreferences();
      const matrixWinId = rt.windowManager.openWindow("notification_matrix", { x: 60, y: 80, width: 480, height: 320 });
      const selVal = document.querySelector("#" + matrixWinId + " [data-notif-rule='survey_report_arrived']")?.value ?? "";
      return {
        phase: "m3-m4-phase2",
        persistedPrefRule: prefs.notificationRules?.survey_report_arrived ?? "",
        snapshotRule: (snap?.notificationRules ?? []).find((r) => r.eventClassKey === "survey_report_arrived")?.behavior ?? "",
        matrixSelectValue: selVal,
      };
    })()`)) as Record<string, unknown>;
  }

  return (await mainWindow.webContents.executeJavaScript(`(async () => {
    const rt = window.__ngM2Runtime;
    const boot = window.__ngBootStatus;
    const initialSnap = rt.store.getSnapshot();

    // 1. Step 1 tick first so initial snapshot telemetry, weather, and ledgers populate cleanly.
    document.getElementById("hud-step-btn")?.click();
    await new Promise((r) => setTimeout(r, 180));

    // 2. Test Overlay Switcher Toolbar (UI-MAP-001..004).
    const weatherBtn = document.querySelector("[data-overlay-mode='weather']");
    weatherBtn?.click();
    const afterWeatherMode = rt.globe.status.activeOverlayMode;
    const weatherLegendText = document.getElementById("map-overlay-legend-summary")?.textContent ?? "";

    const logisticsBtn = document.querySelector("[data-overlay-mode='logistics']");
    logisticsBtn?.click();
    const afterLogisticsMode = rt.globe.status.activeOverlayMode;

    const epistemicBtn = document.querySelector("[data-overlay-mode='epistemic']");
    epistemicBtn?.click();
    const afterEpistemicMode = rt.globe.status.activeOverlayMode;

    // 3. Test Virtualized Physical Ledger + Cohorts tab + Provenance Badge (UI-DAT-001, UI-DAT-005).
    const ledgerId = rt.windowManager.openWindow("physical_ledger", { x: 30, y: 70, width: 520, height: 320 });
    const ledgerSortBtn = document.querySelector("#" + ledgerId + " [data-sort-col='2']");
    ledgerSortBtn?.click();
    const ledgerRowCount = document.querySelectorAll("#" + ledgerId + " tbody tr").length;
    const provBadgeCount = document.querySelectorAll("#" + ledgerId + " .ng-prov-badge").length;
    const cohortsTab = document.querySelector("#" + ledgerId + " [data-ledger-tab='cohorts']");
    cohortsTab?.click();
    const cohortRowCount = document.querySelectorAll("#" + ledgerId + " tbody tr").length;

    // 4. Open TimeSeriesChart & DelegationOrgTree (UI-DAT-002, UI-DAT-003).
    const chartId = rt.windowManager.openWindow("time_series_chart", { x: 560, y: 70, width: 460, height: 300 });
    const chartCanvasPresent = Boolean(document.querySelector("#" + chartId + " canvas.ng-timeseries-canvas"));
    const orgId = rt.windowManager.openWindow("delegation_org_tree", { x: 40, y: 400, width: 460, height: 300 });
    const orgCardsCount = document.querySelectorAll("#" + orgId + " .ng-subordinate-card").length;

    // 5. Open HQ Relocation Planner -> verify route preview highlight & latency output (UI-HUD-005).
    const relocId = rt.windowManager.openWindow("hq_relocation", { x: 520, y: 390, width: 440, height: 280 });
    const relocPreviewText = document.querySelector("#" + relocId + " [data-hq-latency-preview='true']")?.textContent ?? "";
    const previewTargetBeforeClose = rt.globe.status.featureSummary.previewDestinationCellId;

    // 6. Open MandateComposer & seal an expeditionary survey mandate (UI-HUD-002, UI-AUD-003).
    const mandateId = rt.windowManager.openWindow("mandate_composer", { x: 100, y: 110, width: 450, height: 340 });
    const mandatePreviewText = document.querySelector("#" + mandateId + " [data-mandate-preview='true']")?.textContent ?? "";
    const submitBtn = document.querySelector("#" + mandateId + " button[type='submit']");
    submitBtn?.click();
    await new Promise((r) => setTimeout(r, 80));

    // 7. Open NotificationMatrix -> switch rule for restart persistence test (UI-HUD-006).
    const matrixId = rt.windowManager.openWindow("notification_matrix", { x: 580, y: 120, width: 460, height: 300 });
    const ruleSelect = document.querySelector("#" + matrixId + " [data-notif-rule='survey_report_arrived']");
    if (ruleSelect) {
      ruleSelect.value = "DispatchPopupAndAutoPause";
      ruleSelect.dispatchEvent(new Event("change", { bubbles: true }));
    }
    await new Promise((r) => setTimeout(r, 120));

    // 8. Step +1 Hour twice so sim-cli executes queued M4 mandate + rule cleanly.
    document.getElementById("hud-step-btn")?.click();
    await new Promise((r) => setTimeout(r, 180));
    document.getElementById("hud-step-btn")?.click();
    await new Promise((r) => setTimeout(r, 180));

    const finalSnap = rt.store.getSnapshot();
    const foleyLog = rt.audioEngine.getPlayedFoleyLog().map((f) => f.cueId);
    const persistedPrefs = await window.simBridge.readUserPreferences();

    return {
      phase: "m3-m4-phase1",
      hasNodeRequire: boot.hasNodeRequire,
      hasProcessGlobal: boot.hasProcessGlobal,
      initialCellCount: initialSnap?.knownCells?.length ?? 0,
      afterWeatherMode,
      afterLogisticsMode,
      afterEpistemicMode,
      weatherLegendText,
      ledgerRowCount,
      provBadgeCount,
      cohortRowCount,
      chartCanvasPresent,
      orgCardsCount,
      relocPreviewText,
      previewTargetBeforeClose,
      mandatePreviewText,
      finalTick: finalSnap?.clockAndNotifications.tick ?? 0,
      activeMandateCount: finalSnap?.delegationTree.activeMandates?.length ?? 0,
      persistedRuleInSnapshot: (finalSnap?.notificationRules ?? []).find((r) => r.eventClassKey === "survey_report_arrived")?.behavior ?? "",
      persistedRuleInPrefs: persistedPrefs.notificationRules?.survey_report_arrived ?? "",
      foleyLog,
    };
  })()`)) as Record<string, unknown>;
}
