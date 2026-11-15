import {
  DiegeticStoreAccess,
  UiIntentBus,
  selectClockAndNotifications,
  selectDelegationTree,
  selectHqPresence,
  selectOpenProposals,
} from "../spine";

export {
  createArbitrationWindowPlugin,
  createAttentionZoomWindowPlugin,
  createHqRelocationPlannerWindowPlugin,
  createMandateComposerWindowPlugin,
  createNotificationMatrixWindowPlugin,
} from "./m4-panels";

const SPEED_TIERS: readonly number[] = [1, 4, 24, 168];

/**
 * Top command HUD bar displaying scenario identity, clickable HQ fly-to anchor,
 * Personal Attention summary, pause/resume toggle, `+1 Hour` single step, discrete
 * speed controls (`1x`/`4x`/`24x`/`168x`), compass north reset, and auto-pause banner (`UI-HUD-001`).
 */
export function mountCommandHud(
  host: HTMLElement,
  store: DiegeticStoreAccess,
  bus: UiIntentBus,
): () => void {
  const bar = document.createElement("header");
  bar.className = "ng-hud-bar";
  bar.setAttribute("data-chrome-occluder", "true");

  const leftCluster = document.createElement("div");
  leftCluster.className = "ng-hud-cluster";

  const scenarioLabel = document.createElement("strong");
  scenarioLabel.id = "hud-scenario-title";
  scenarioLabel.textContent = "Awaiting scenario";

  const hqLinkBtn = document.createElement("button");
  hqLinkBtn.type = "button";
  hqLinkBtn.id = "hud-hq-link";
  hqLinkBtn.className = "ng-hud-link";
  hqLinkBtn.textContent = "HQ: Unknown";

  const attentionBadge = document.createElement("span");
  attentionBadge.id = "hud-attention-badge";
  attentionBadge.className = "ng-hud-badge";
  attentionBadge.textContent = "Attention: 0/0";

  leftCluster.append(scenarioLabel, hqLinkBtn, attentionBadge);

  const clockCluster = document.createElement("div");
  clockCluster.className = "ng-hud-cluster";

  const clockLabel = document.createElement("span");
  clockLabel.id = "hud-clock-text";
  clockLabel.className = "ng-hud-badge";
  clockLabel.textContent = "Year 1";

  const pauseBtn = document.createElement("button");
  pauseBtn.type = "button";
  pauseBtn.id = "hud-pause-btn";
  pauseBtn.className = "ng-btn";
  pauseBtn.setAttribute("aria-pressed", "false");
  pauseBtn.textContent = "Pause";

  const stepBtn = document.createElement("button");
  stepBtn.type = "button";
  stepBtn.id = "hud-step-btn";
  stepBtn.className = "ng-btn";
  stepBtn.textContent = "+1 Hour";

  const speedButtons = new Map<number, HTMLButtonElement>();
  for (const tier of SPEED_TIERS) {
    const sBtn = document.createElement("button");
    sBtn.type = "button";
    sBtn.className = "ng-btn";
    sBtn.setAttribute("data-speed-tier", String(tier));
    sBtn.setAttribute("aria-pressed", tier === 1 ? "true" : "false");
    sBtn.textContent = `${tier}x`;
    sBtn.addEventListener("click", () => {
      bus.emit({
        kind: "DispatchSimCommand",
        command: { kind: "setSpeedMultiplier", multiplier: tier },
      });
      bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
    });
    speedButtons.set(tier, sBtn);
  }

  const compassBtn = document.createElement("button");
  compassBtn.type = "button";
  compassBtn.id = "hud-compass-btn";
  compassBtn.className = "ng-btn";
  compassBtn.textContent = "N 0°";
  compassBtn.setAttribute("aria-label", "Realign camera north");
  compassBtn.addEventListener("click", () => {
    bus.emit({ kind: "ResetGlobeNorth" });
    bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
  });

  const autoPauseBanner = document.createElement("div");
  autoPauseBanner.id = "hud-autopause-banner";
  autoPauseBanner.className = "ng-hud-autopause-banner ng-autopause-banner";
  autoPauseBanner.setAttribute("role", "status");
  autoPauseBanner.hidden = true;

  clockCluster.append(
    clockLabel,
    pauseBtn,
    stepBtn,
    ...speedButtons.values(),
    compassBtn,
    autoPauseBanner,
  );

  const windowsCluster = document.createElement("div");
  windowsCluster.className = "ng-hud-cluster";
  const launchers: ReadonlyArray<{ readonly pluginId: string; readonly label: string }> = [
    { pluginId: "physical_ledger", label: "Ledger" },
    { pluginId: "time_series_chart", label: "Charts" },
    { pluginId: "delegation_org_tree", label: "Org Tree" },
    { pluginId: "entity_inspector", label: "Inspector" },
    { pluginId: "dispatch_inbox", label: "Dispatches" },
    { pluginId: "mandate_composer", label: "Mandates" },
    { pluginId: "arbitration_panel", label: "Arbitration" },
    { pluginId: "attention_zoom", label: "Attention" },
    { pluginId: "hq_relocation", label: "Relocate HQ" },
    { pluginId: "notification_matrix", label: "Alerts" },
    { pluginId: "skin_appearance", label: "Skins" },
    { pluginId: "audio_mixer", label: "Audio" },
    { pluginId: "layout_presets", label: "Layouts" },
  ];

  const launcherButtons = new Map<string, HTMLButtonElement>();
  for (const item of launchers) {
    const wBtn = document.createElement("button");
    wBtn.type = "button";
    wBtn.className = "ng-btn";
    wBtn.setAttribute("data-open-plugin", item.pluginId);
    wBtn.textContent = item.label;
    wBtn.addEventListener("click", () => {
      bus.emit({ kind: "OpenSystemWindow", pluginId: item.pluginId });
    });
    launcherButtons.set(item.pluginId, wBtn);
    windowsCluster.appendChild(wBtn);
  }

  bar.append(leftCluster, clockCluster, windowsCluster);
  host.appendChild(bar);

  let currentHqCellId = 0;
  let currentlyPaused = false;

  hqLinkBtn.addEventListener("click", () => {
    bus.emit({ kind: "FlyGlobeToCell", cellId: currentHqCellId });
    bus.emit({
      kind: "FocusEntityInspector",
      entityKind: "settlement",
      entityId: 0,
    });
  });

  pauseBtn.addEventListener("click", () => {
    bus.emit({
      kind: "DispatchSimCommand",
      command: { kind: "setPaused", paused: !currentlyPaused },
    });
    bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
  });

  stepBtn.addEventListener("click", () => {
    bus.emit({ kind: "DispatchSimCommand", command: { kind: "stepSingleTick" } });
    bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
  });

  const unsubStore = store.subscribe((snap) => {
    const hq = selectHqPresence(store);
    const clk = selectClockAndNotifications(store);
    const delegation = selectDelegationTree(store);

    currentHqCellId = hq?.cellId ?? 0;
    scenarioLabel.textContent = `${snap.scenarioName} — HQ: ${hq?.settlementName ?? "Unknown"}`;
    hqLinkBtn.textContent = hq ? `HQ: ${hq.settlementName} (#${hq.cellId})` : "HQ: Unknown";

    if (delegation) {
      attentionBadge.textContent = `Attention: ${delegation.usedAttention.toFixed(1)}/${delegation.capacityAttention.toFixed(1)}`;
    }

    const proposalCount = selectOpenProposals(store).length;
    const arbBtn = launcherButtons.get("arbitration_panel");
    if (arbBtn) {
      arbBtn.textContent = proposalCount > 0 ? `Arbitration (${proposalCount})` : "Arbitration";
    }

    if (clk) {
      currentlyPaused = clk.paused;
      clockLabel.textContent = `${clk.calendarText} [Tick ${clk.tick}]`;
      pauseBtn.setAttribute("aria-pressed", clk.paused ? "true" : "false");
      pauseBtn.textContent = clk.paused ? "Resume" : "Pause";
      for (const [tier, btn] of speedButtons.entries()) {
        btn.setAttribute("aria-pressed", clk.speedMultiplier === tier ? "true" : "false");
      }
      if (clk.autoPauseReason && clk.autoPauseReason.trim().length > 0) {
        autoPauseBanner.hidden = false;
        autoPauseBanner.textContent = `Auto-Paused: ${clk.autoPauseReason}`;
      } else {
        autoPauseBanner.hidden = true;
        autoPauseBanner.textContent = "";
      }
    }
  });

  const unsubBus = bus.subscribe((intent) => {
    if (intent.kind === "UpdateCameraTelemetry") {
      const deg = Math.round((intent.telemetry.headingRad * 180) / Math.PI);
      compassBtn.textContent = `N ${deg}°`;
    }
  });

  return () => {
    unsubStore();
    unsubBus();
    bar.remove();
  };
}
