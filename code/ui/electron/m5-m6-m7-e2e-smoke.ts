import type { BrowserWindow } from "electron";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Live Chromium + Electron + persistent `sim-cli serve-stdio` E2E verification for
 * Option B continuous globe + M5 trade/congestion + M6 institutional reforms +
 * M7 empirical archive, operational warfare, and native `<dialog>` succession transition.
 */
export async function runM5M6M7E2ESmoke(
  mainWindow: BrowserWindow,
  phase: string,
): Promise<Record<string, unknown>> {
  for (let i = 0; i < 120; i += 1) {
    const ready = await mainWindow.webContents.executeJavaScript(
      "Boolean(window.__ngBootStatus?.ready && window.__ngM2Runtime && window.simBridge)",
    );
    if (ready) break;
    await delay(40);
  }

  return (await mainWindow.webContents.executeJavaScript(`(async () => {
    const rt = window.__ngM2Runtime;
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    const snap0 = rt.store.getSnapshot();
    const initialTick = snap0?.clockAndNotifications?.tick ?? -1;
    const initialPaused = Boolean(snap0?.clockAndNotifications?.paused);
    const initialGovModel = snap0?.governance?.activeModelKey ?? null;
    const actorCount = snap0?.nonStateEconomy?.actors?.length ?? 0;

    // 1. Live serve-stdio clock progression when unpaused, then clean re-pause.
    const pauseBtn = document.getElementById("hud-pause-btn");
    pauseBtn.click();
    for (let i = 0; i < 60; i += 1) {
      await wait(40);
      const cur = rt.store.getSnapshot()?.clockAndNotifications;
      if (cur && !cur.paused && cur.tick > initialTick) break;
    }
    const snapRunning = rt.store.getSnapshot();
    const afterUnpauseTick = snapRunning?.clockAndNotifications?.tick ?? -1;
    pauseBtn.click();
    for (let i = 0; i < 40; i += 1) {
      await wait(40);
      if (rt.store.getSnapshot()?.clockAndNotifications?.paused) break;
    }
    const snapRepaused = rt.store.getSnapshot();
    const repausedState = Boolean(snapRepaused?.clockAndNotifications?.paused);

    // 1b. Photorealistic vs Parchment lens, Equal-Area & Mercator projection toggles, and DOM labels.
    const strideBytes = (s) => {
      const out = [];
      if (!s) return out;
      for (let i = 0; i < s.rgba.length; i += 32) {
        out.push(s.rgba[i], s.rgba[i + 1], s.rgba[i + 2]);
      }
      return out;
    };
    const hqCid = snap0?.hqPresence?.cellId ?? 0;
    const hqGeom = rt.globe.topology.cells.find((c) => c.cellId === hqCid) ?? rt.globe.topology.cells[0];
    rt.globe.camera.flyToUnitVector(hqGeom.centerUnit, 0.28, 0, true);
    rt.globe.stepRender(16);
    await wait(40);
    const topologyCellCount = snap0?.globeTopology?.cellCount ?? 0;
    const shaderCompiled = Boolean(window.__ngBootStatus?.shaderCompiled);
    const renderedLabelCount = document.querySelectorAll("#map-feature-labels .ng-map-label:not([hidden])").length;

    rt.globe.camera.flyToUnitVector(hqGeom.centerUnit, 0.45, 0, true);
    rt.globe.stepRender(16);
    const terrainSample = strideBytes(rt.globe.captureFrameSample(96));

    document.querySelector('[data-overlay-mode="parchment"]')?.click();
    await wait(40);
    rt.globe.stepRender(16);
    const parchmentSample = strideBytes(rt.globe.captureFrameSample(96));

    document.querySelector('[data-overlay-mode="terrain"]')?.click();
    await wait(40);
    rt.globe.stepRender(16);

    document.querySelector('[data-projection-mode="equal_area"]')?.click();
    await wait(40);
    rt.globe.stepRender(16);
    const projEqualArea = rt.globe.status.projectionMode;
    const equalAreaSample = strideBytes(rt.globe.captureFrameSample(96));

    document.querySelector('[data-projection-mode="mercator"]')?.click();
    await wait(40);
    rt.globe.stepRender(16);
    const projMercator = rt.globe.status.projectionMode;
    const mercatorSample = strideBytes(rt.globe.captureFrameSample(96));

    document.querySelector('[data-projection-mode="globe"]')?.click();
    await wait(40);
    rt.globe.stepRender(16);
    const projRestoredGlobe = rt.globe.status.projectionMode;

    // 2. Switch map overlays to Trade and Warfare modes & verify legend telemetry.
    const tradeBtn = document.querySelector('[data-overlay-mode="trade"]');
    tradeBtn?.click();
    await wait(60);
    const tradeLegend = document.getElementById("map-overlay-legend-summary")?.textContent ?? "";

    const warfareBtn = document.querySelector('[data-overlay-mode="warfare"]');
    warfareBtn?.click();
    await wait(60);
    const warfareLegend = document.getElementById("map-overlay-legend-summary")?.textContent ?? "";

    // 3. Open Institutions Inspector (UI-DAT-006.1, UI-DAT-006.2), check accessible tabs & dispatch live governance reform.
    document.querySelector('[data-open-plugin="institutions_inspector"]')?.click();
    await wait(60);
    const instTablist = document.querySelector('[aria-label="Institutional and Economic Sections"]');
    const instInactivePanel = document.querySelector('[role="tabpanel"][data-panel-key="markets"]');
    const instPanelStyles = instInactivePanel ? getComputedStyle(instInactivePanel) : null;
    const instSummaryHeadings = Array.from(document.querySelectorAll("details.ng-disclosure summary"))
      .map((s) => Boolean(s.querySelector("h1, h2, h3, h4, h5, h6, [role='heading']")));
    const govTab = document.querySelector('[aria-label="Institutional and Economic Sections"] [role="tab"][data-tab-key="governance"]');
    govTab?.click();
    await wait(40);
    const modelSel = document.querySelector('[data-gov-model-select="true"]');
    if (modelSel) modelSel.value = "constitutional_cabinet";
    document.querySelector('[data-gov-model-submit="true"]')?.click();
    await wait(160);
    const updatedGovModel = rt.store.getSnapshot()?.governance?.activeModelKey ?? null;

    // 4. Open Empirical Knowledge Archive (UI-DAT-007.1, UI-DAT-007.2) & update innovation mode on live engine.
    document.querySelector('[data-open-plugin="knowledge_archive"]')?.click();
    await wait(60);
    const knowledgeBadges = document.querySelectorAll(".ng-fragility-badge").length;
    const hasTechTreeText = /Tech Tree|Science Points|Buy Tech/i.test(document.body.textContent ?? "");
    const patentsTab = document.querySelector('[aria-label="Empirical Knowledge Archive Sections"] [role="tab"][data-tab-key="patents"]');
    patentsTab?.click();
    await wait(40);
    const modeSel = document.querySelector('[data-innovation-mode-select="true"]');
    const modeTarget = modeSel?.options?.[1]?.value || modeSel?.value || "";
    if (modeSel && modeTarget) modeSel.value = modeTarget;
    document.querySelector('[data-innovation-submit="true"]')?.click();
    await wait(160);
    const updatedInnovationMode = rt.store.getSnapshot()?.knowledge?.settlements?.[0]?.innovationModeKey ?? null;

    // 5. Open Warfare Command (UI-HUD-007.1), dispatch theater directive, & verify structural IPC prohibitions.
    document.querySelector('[data-open-plugin="warfare_command"]')?.click();
    await wait(60);
    const churchillPreview = document.querySelector('[data-warfare-attention-preview="churchill"]')?.textContent ?? "";
    const hitlerPreview = document.querySelector('[data-warfare-attention-preview="hitler"]')?.textContent ?? "";
    const objInput = document.querySelector('[data-theater-objective-input="true"]');
    if (objInput) objInput.value = "4";
    document.querySelector('[data-theater-submit="true"]')?.click();
    await wait(160);
    const theaterAckSnap = rt.store.getSnapshot();
    const updatedTheaterObjective = theaterAckSnap?.warfare?.theaters?.[0]?.objectiveCellId ?? null;

    const forbiddenMicroAck = await window.__ngSendTestCommand({ kind: "issueTacticalMicroCommand", unitId: 1 });
    const forbiddenTechAck = await window.__ngSendTestCommand({ kind: "buyOrSelectTechTarget", techKey: "rail" });

    // 6. Trigger live Generational Succession modal <dialog> (UI-HUD-007.2) via sim:load-mock-fixture & test real :modal state.
    const liveBase = JSON.parse(JSON.stringify(rt.store.getSnapshot()));
    liveBase.succession = {
      effectiveCommandMultiplier: 0.78,
      roster: liveBase.succession?.roster?.length
        ? liveBase.succession.roster
        : [
            {
              characterId: 1,
              name: "Governor R. Halstead",
              roleTitle: "Acting Regent",
              ageYears: 45,
              health01: 0.9,
              isAlive: true,
              isRuler: true,
              settlementId: 0,
              settlementName: "Haven Landing Perimeter",
            },
          ],
      pendingTransition: {
        ruleKey: "chartered_seniority",
        ruleDisplayName: "Chartered Council Seniority",
        deceasedRulerName: "Expedition Director K. Sato",
        successorName: "Governor R. Halstead",
        transitionTick: liveBase.clockAndNotifications.tick,
        disruptionUntilTick: liveBase.clockAndNotifications.tick + 24,
        inheritedArchiveEntries: ["Founding Colony Charter", "Copper Smelting Flux Notes"],
        lostUnrecordedRapportCount: 3,
        awaitingPlayerAcknowledgement: true,
      },
    };
    await window.simBridge.loadMockFixture(JSON.stringify(liveBase));
    await wait(100);

    const dialog = document.getElementById("succession-transition-dialog");
    const dialogOpenBeforeAck = Boolean(dialog?.open);
    const dialogMatchesModalBeforeAck = Boolean(dialog?.matches(":modal"));
    const dialogRoleAttr = dialog?.getAttribute("role");
    const dialogAriaModalAttr = dialog?.getAttribute("aria-modal");
    const dialogAriaLabelledBy = dialog?.getAttribute("aria-labelledby");
    const dialogLossText = dialog?.querySelector('[data-succession-unrecorded-loss="true"]')?.textContent ?? "";

    const closedPromise = new Promise((resolve) => {
      if (!dialog) {
        resolve();
        return;
      }
      dialog.addEventListener("close", () => resolve(), { once: true });
      setTimeout(() => resolve(), 1500);
    });
    document.querySelector('[data-acknowledge-succession="true"]')?.click();
    await closedPromise;
    await wait(160);

    const dialogOpenAfterAck = Boolean(dialog?.open);
    const dialogMatchesModalAfterAck = Boolean(dialog?.matches(":modal"));
    const dialogReturnValueAfterAck = dialog?.returnValue ?? "unset";
    const postAckPending = rt.store.getSnapshot()?.succession?.pendingTransition ?? null;

    return {
      phase: ${JSON.stringify(phase)},
      initialTick,
      initialPaused,
      initialGovModel,
      actorCount,
      afterUnpauseTick,
      repausedState,
      topologyCellCount,
      shaderCompiled,
      renderedLabelCount,
      terrainSample,
      parchmentSample,
      projEqualArea,
      equalAreaSample,
      projMercator,
      mercatorSample,
      projRestoredGlobe,
      tradeLegend,
      warfareLegend,
      hasInstTablist: Boolean(instTablist),
      inactivePanelHiddenAttr: instInactivePanel?.getAttribute("hidden") ?? null,
      inactivePanelPadding: instPanelStyles?.padding ?? "",
      instSummaryHasHeading: instSummaryHeadings.some(Boolean),
      updatedGovModel,
      knowledgeBadges,
      hasTechTreeText,
      modeTarget,
      updatedInnovationMode,
      churchillPreview,
      hitlerPreview,
      updatedTheaterObjective,
      forbiddenMicroAck,
      forbiddenTechAck,
      dialogOpenBeforeAck,
      dialogMatchesModalBeforeAck,
      dialogRoleAttr,
      dialogAriaModalAttr,
      dialogAriaLabelledBy,
      dialogLossText,
      dialogOpenAfterAck,
      dialogMatchesModalAfterAck,
      dialogReturnValueAfterAck,
      postAckPending,
    };
  })()`)) as Record<string, unknown>;
}
