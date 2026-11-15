import {
  DiegeticSnapshot,
  DiegeticStoreAccess,
  GlobeLayerHost,
  GlobeRayHit,
  MapLayerPlugin,
  UiIntentBus,
  describeProvenanceBadge,
} from "../spine";

export interface OverlayTelemetrySummary {
  readonly activeModeId: string;
  readonly unexploredCount: number;
  readonly rumoredCount: number;
  readonly staleCount: number;
  readonly surveyedCount: number;
  readonly liveSightCount: number;
  readonly weatherStationCount: number;
  readonly stormAlertCount: number;
  readonly windVectorCount: number;
  readonly convoyCount: number;
  readonly projectedConvoyCount: number;
  readonly overdueConvoyCount: number;
  readonly tradeRibbonCount: number;
  readonly regionalMarketCount: number;
  readonly maxPricePremiumPct: number;
  readonly congestedEdgeCount: number;
  readonly maxCongestionMultiplier: number;
  readonly latestMarketObservedTick: number;
  readonly theaterCount: number;
  readonly formationCount: number;
  readonly staleFormationCount: number;
  readonly noRetreatFormationCount: number;
}

/** Baseline map overlay plugin cartridge for M0..M4 shell verification. */
export function createBaselineOverlayPlugin(): MapLayerPlugin {
  return {
    layerId: "epistemic_fog_stub",
    displayName: "Survey Horizon",
    supportsMode: () => true,
    mount(_host: GlobeLayerHost, _store: DiegeticStoreAccess, _bus: UiIntentBus) {},
    onSnapshotUpdate(_snapshot: DiegeticSnapshot) {},
    onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number) {},
    dispose() {},
  };
}

/** Milestone M5 trade & logistics congestion overlay contract helper (`UI-MAP-006.1`, `UI-MAP-006.2`). */
export function createTradeAndCongestionStubLayer(): MapLayerPlugin {
  return new TradeAndCongestionOverlayLayer();
}

/** Milestone M7 operational warfare fronts overlay contract helper (`UI-MAP-007.1`, `UI-MAP-007.2`). */
export function createOperationalWarfareStubLayer(): MapLayerPlugin {
  return new OperationalWarfareOverlayLayer();
}

/**
 * Computes real-time diegetic overlay metrics strictly from `DiegeticSnapshot`
 * without ever referencing hidden engine ground truth (`UI-MAP-001`..`004`).
 */
export function summarizeOverlaySnapshot(
  snapshot: DiegeticSnapshot,
  activeModeId = "terrain",
): OverlayTelemetrySummary {
  let unexploredCount = 0;
  let rumoredCount = 0;
  let staleCount = 0;
  let surveyedCount = 0;
  let liveSightCount = 0;

  for (const cell of snapshot.knownCells) {
    if (cell.tier === "unexplored") unexploredCount += 1;
    else if (cell.tier === "rumored") rumoredCount += 1;
    else if (cell.tier === "stale") staleCount += 1;
    else {
      surveyedCount += 1;
      if (cell.isLiveHqSight) liveSightCount += 1;
    }
  }

  let stormAlertCount = 0;
  let windVectorCount = 0;
  for (const w of snapshot.observedWeather) {
    if (w.precipMmHr > 1.5 || (w.floodStage01 ?? 0) > 0.25) {
      stormAlertCount += 1;
    }
    if (Math.hypot(w.windUMps ?? 0, w.windVMps ?? 0) > 0.4) {
      windVectorCount += 1;
    }
  }

  let projectedConvoyCount = 0;
  let overdueConvoyCount = 0;
  for (const convoy of snapshot.trackedConvoys) {
    const s = convoy.status.toLowerCase();
    if (s.includes("overdue")) overdueConvoyCount += 1;
    else if (!s.includes("confirmed") && !s.includes("live")) projectedConvoyCount += 1;
  }

  const markets = snapshot.nonStateEconomy?.informalMarkets ?? [];
  const settlementByCommodity = new Map<string, Set<number>>();
  let maxPricePremiumPct = 0;
  let latestMarketObservedTick = 0;
  for (const market of markets) {
    const bucket = settlementByCommodity.get(market.commodityKey) ?? new Set<number>();
    bucket.add(market.settlementId);
    settlementByCommodity.set(market.commodityKey, bucket);
    maxPricePremiumPct = Math.max(maxPricePremiumPct, Math.round(market.blackMarketPremium01 * 100));
    latestMarketObservedTick = Math.max(latestMarketObservedTick, market.observedTick);
  }
  let marketPairRibbons = 0;
  for (const set of settlementByCommodity.values()) {
    if (set.size > 1) {
      marketPairRibbons += (set.size * (set.size - 1)) / 2;
    }
  }
  const convoyRibbons = snapshot.trackedConvoys.filter(
    (c) => c.originCellId !== undefined && c.destinationCellId !== undefined,
  ).length;
  const tradeRibbonCount = convoyRibbons + marketPairRibbons;

  const congested = (snapshot.nonStateEconomy?.congestedEdges ?? []).filter(
    (e) => e.congestionMultiplier > 1.05,
  );
  const maxCongestionMultiplier = congested.reduce(
    (acc, e) => Math.max(acc, e.congestionMultiplier),
    1.0,
  );

  const theaters = snapshot.warfare?.theaters ?? [];
  const formations = snapshot.warfare?.formations ?? [];
  const currentTick = snapshot.clockAndNotifications.tick;
  let staleFormationCount = 0;
  let noRetreatFormationCount = 0;
  for (const f of formations) {
    if (currentTick - f.observedTick > 6) {
      staleFormationCount += 1;
    }
    if (f.noRetreatOrder) {
      noRetreatFormationCount += 1;
    }
  }

  return {
    activeModeId,
    unexploredCount,
    rumoredCount,
    staleCount,
    surveyedCount,
    liveSightCount,
    weatherStationCount: snapshot.observedWeather.length,
    stormAlertCount,
    windVectorCount,
    convoyCount: snapshot.trackedConvoys.length,
    projectedConvoyCount,
    overdueConvoyCount,
    tradeRibbonCount,
    regionalMarketCount: markets.length,
    maxPricePremiumPct,
    congestedEdgeCount: congested.length,
    maxCongestionMultiplier,
    latestMarketObservedTick,
    theaterCount: theaters.length,
    formationCount: formations.length,
    staleFormationCount,
    noRetreatFormationCount,
  };
}

/** Epistemic Currency & HQ Courier Delay Contour map layer (`UI-MAP-001`). */
export class EpistemicCurrencyOverlayLayer implements MapLayerPlugin {
  readonly layerId = "epistemic_currency";
  readonly displayName = "Epistemic Horizon & Delay";
  private lastSummary: OverlayTelemetrySummary | null = null;

  public supportsMode(modeId: string): boolean {
    return modeId === "epistemic" || modeId === "terrain";
  }

  public mount(_host: GlobeLayerHost, store: DiegeticStoreAccess, _bus: UiIntentBus): void {
    const snap = store.getSnapshot();
    if (snap) this.onSnapshotUpdate(snap);
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.lastSummary = summarizeOverlaySnapshot(snapshot, "epistemic");
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public getSummary(): OverlayTelemetrySummary | null {
    return this.lastSummary;
  }

  public dispose(): void {
    this.lastSummary = null;
  }
}

/** Observed Weather & Hydrology overlay plugin (`UI-MAP-004`). */
export class WeatherHydrologyOverlayLayer implements MapLayerPlugin {
  readonly layerId = "weather_hydrology";
  readonly displayName = "Weather & Hydrology";
  private lastSummary: OverlayTelemetrySummary | null = null;

  public supportsMode(modeId: string): boolean {
    return modeId === "weather";
  }

  public mount(_host: GlobeLayerHost, store: DiegeticStoreAccess, _bus: UiIntentBus): void {
    const snap = store.getSnapshot();
    if (snap) this.onSnapshotUpdate(snap);
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.lastSummary = summarizeOverlaySnapshot(snapshot, "weather");
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public getSummary(): OverlayTelemetrySummary | null {
    return this.lastSummary;
  }

  public dispose(): void {
    this.lastSummary = null;
  }
}

/** Dead-Reckoning Logistics & Convoys overlay plugin (`UI-MAP-002`). */
export class LogisticsConvoyOverlayLayer implements MapLayerPlugin {
  readonly layerId = "logistics_convoys";
  readonly displayName = "Logistics & Dead-Reckoning";
  private lastSummary: OverlayTelemetrySummary | null = null;

  public supportsMode(modeId: string): boolean {
    return modeId === "logistics";
  }

  public mount(_host: GlobeLayerHost, store: DiegeticStoreAccess, _bus: UiIntentBus): void {
    const snap = store.getSnapshot();
    if (snap) this.onSnapshotUpdate(snap);
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.lastSummary = summarizeOverlaySnapshot(snapshot, "logistics");
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public getSummary(): OverlayTelemetrySummary | null {
    return this.lastSummary;
  }

  public dispose(): void {
    this.lastSummary = null;
  }
}

/** Trade Flows, Regional Price Spreads & Corridor Congestion overlay plugin (`UI-MAP-006.1`, `UI-MAP-006.2`). */
export class TradeAndCongestionOverlayLayer implements MapLayerPlugin {
  readonly layerId = "trade_and_congestion";
  readonly displayName = "Trade & Corridor Congestion";
  private lastSummary: OverlayTelemetrySummary | null = null;

  public supportsMode(modeId: string): boolean {
    return modeId === "trade";
  }

  public mount(_host: GlobeLayerHost, store: DiegeticStoreAccess, _bus: UiIntentBus): void {
    const snap = store.getSnapshot();
    if (snap) this.onSnapshotUpdate(snap);
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.lastSummary = summarizeOverlaySnapshot(snapshot, "trade");
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public getSummary(): OverlayTelemetrySummary | null {
    return this.lastSummary;
  }

  public dispose(): void {
    this.lastSummary = null;
  }
}

/** Operational Warfare Theaters, Battle Plans & Courier-Lagged Formations overlay plugin (`UI-MAP-007.1`, `UI-MAP-007.2`). */
export class OperationalWarfareOverlayLayer implements MapLayerPlugin {
  readonly layerId = "operational_warfare";
  readonly displayName = "Operational Warfare Fronts";
  private lastSummary: OverlayTelemetrySummary | null = null;

  public supportsMode(modeId: string): boolean {
    return modeId === "warfare";
  }

  public mount(_host: GlobeLayerHost, store: DiegeticStoreAccess, _bus: UiIntentBus): void {
    const snap = store.getSnapshot();
    if (snap) this.onSnapshotUpdate(snap);
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.lastSummary = summarizeOverlaySnapshot(snapshot, "warfare");
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public getSummary(): OverlayTelemetrySummary | null {
    return this.lastSummary;
  }

  public dispose(): void {
    this.lastSummary = null;
  }
}

/** Diegetic bottom-left map overlay lens selector & legend rendered over the globe (`UI-MAP-001`..`004`). */
export class OverlayToolbarLayer implements MapLayerPlugin {
  readonly layerId = "overlay_mode_switcher";
  readonly displayName = "Map Lens Selector";
  private rootEl: HTMLDivElement | null = null;
  private legendEl: HTMLDivElement | null = null;
  private activeModeId = "terrain";
  private latestSnapshot: DiegeticSnapshot | null = null;
  private unsubs: Array<() => void> = [];

  public supportsMode(): boolean {
    return true;
  }

  public mount(host: GlobeLayerHost, store: DiegeticStoreAccess, bus: UiIntentBus): void {
    this.dispose();
    const canvas = host.getCanvasElement();
    const overlayRoot = document.getElementById(host.overlayRootId) ?? canvas.parentElement ?? document.body;

    const bar = document.createElement("div");
    bar.id = "map-overlay-toolbar";
    bar.className = "ng-overlay-legend ng-hud-chrome-interactive";
    bar.setAttribute("data-chrome-occluder", "true");
    bar.setAttribute("role", "region");
    bar.setAttribute("aria-label", "Map Lens Modes");

    const modes = [
      { id: "terrain", label: "Terrain" },
      { id: "epistemic", label: "Recon & Delay" },
      { id: "weather", label: "Weather" },
      { id: "logistics", label: "Logistics" },
      { id: "trade", label: "Trade" },
      { id: "warfare", label: "Warfare" },
    ];

    const buttons = new Map<string, HTMLButtonElement>();
    const syncButtons = () => {
      for (const [id, btn] of buttons) {
        btn.setAttribute("aria-pressed", id === this.activeModeId ? "true" : "false");
        btn.classList.toggle("is-active", id === this.activeModeId);
      }
    };

    for (const mode of modes) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "ng-hud-btn";
      btn.dataset.overlayMode = mode.id;
      btn.textContent = mode.label;
      btn.addEventListener("click", () => {
        this.activeModeId = mode.id;
        syncButtons();
        this.refreshLegend();
        bus.emit({ kind: "SwitchMapOverlayMode", modeId: mode.id });
        bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
      });
      buttons.set(mode.id, btn);
      bar.appendChild(btn);
    }

    const legend = document.createElement("div");
    legend.id = "map-overlay-legend-summary";
    legend.className = "ng-hud-stat";
    bar.appendChild(legend);

    overlayRoot.appendChild(bar);
    this.rootEl = bar;
    this.legendEl = legend;
    syncButtons();

    this.unsubs.push(
      bus.subscribe((intent) => {
        if (intent.kind === "SwitchMapOverlayMode") {
          this.activeModeId = intent.modeId;
          syncButtons();
          this.refreshLegend();
        }
      }),
    );

    const snap = store.getSnapshot();
    if (snap) {
      this.onSnapshotUpdate(snap);
    }
  }

  private refreshLegend(): void {
    if (!this.legendEl || !this.latestSnapshot) return;
    const summary = summarizeOverlaySnapshot(this.latestSnapshot, this.activeModeId);
    if (this.activeModeId === "weather") {
      this.legendEl.textContent =
        `Obs ${summary.weatherStationCount} · Storms ${summary.stormAlertCount} · Wind Vectors ${summary.windVectorCount}`;
    } else if (this.activeModeId === "logistics") {
      this.legendEl.textContent =
        `Convoys ${summary.convoyCount} · Projected ${summary.projectedConvoyCount} · Overdue ${summary.overdueConvoyCount}`;
    } else if (this.activeModeId === "trade") {
      this.legendEl.textContent =
        `Ribbons ${summary.tradeRibbonCount} · Markets ${summary.regionalMarketCount} (Spread +${summary.maxPricePremiumPct}%, Tick ${summary.latestMarketObservedTick}) · Bottlenecks ${summary.congestedEdgeCount} (Max ${summary.maxCongestionMultiplier.toFixed(2)}x)`;
    } else if (this.activeModeId === "warfare") {
      this.legendEl.textContent =
        `Theaters ${summary.theaterCount} · Formations ${summary.formationCount} (Stale ${summary.staleFormationCount}) · No-Retreat ${summary.noRetreatFormationCount}`;
    } else if (this.activeModeId === "epistemic") {
      this.legendEl.textContent =
        `Live ${summary.liveSightCount} · Surveyed ${summary.surveyedCount} · Stale ${summary.staleCount} · Rumored ${summary.rumoredCount}`;
    } else {
      this.legendEl.textContent =
        `Surveyed ${summary.surveyedCount} · Unexplored ${summary.unexploredCount}`;
    }
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.latestSnapshot = snapshot;
    this.refreshLegend();
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public dispose(): void {
    while (this.unsubs.length > 0) {
      this.unsubs.pop()?.();
    }
    this.rootEl?.remove();
    this.rootEl = null;
    this.legendEl = null;
  }
}

function clientPointToNdc(canvas: HTMLCanvasElement, clientX: number, clientY: number): {
  readonly ndcX: number;
  readonly ndcY: number;
} {
  const rect = canvas.getBoundingClientRect();
  const width = rect.width > 0 ? rect.width : Math.max(canvas.clientWidth || canvas.width || 1280, 1);
  const height = rect.height > 0 ? rect.height : Math.max(canvas.clientHeight || canvas.height || 720, 1);
  const left = rect.width > 0 ? rect.left : 0;
  const top = rect.height > 0 ? rect.top : 0;
  const ndcX = ((clientX - left) / width) * 2 - 1;
  const ndcY = -(((clientY - top) / height) * 2 - 1);
  return { ndcX, ndcY };
}

function formatHoverTooltipText(hit: GlobeRayHit, snapshot: DiegeticSnapshot | null): string {
  const coordSuffix =
    hit.latDeg !== undefined && hit.lonDeg !== undefined
      ? ` (${hit.latDeg.toFixed(1)}°, ${hit.lonDeg.toFixed(1)}°)`
      : "";
  if (!snapshot) return `${hit.label}${coordSuffix}`;
  const currentTick = snapshot.clockAndNotifications.tick;
  if (hit.hitKind === "formation") {
    const formation = snapshot.warfare?.formations.find((f) => f.formationId === hit.formationId);
    if (formation) {
      const desc = describeProvenanceBadge({
        currentTick,
        observedTick: formation.observedTick,
        confidence01: formation.confidence01,
        provenanceLabel: formation.provenanceLabel,
      });
      return `${hit.label} — ${formation.personnelCount} personnel (${desc.badgeText})${coordSuffix}`;
    }
  }
  if (hit.hitKind === "convoy") {
    const convoy = snapshot.trackedConvoys.find((c) => c.convoyId === hit.convoyId);
    if (convoy && convoy.provenanceLabel) {
      const desc = describeProvenanceBadge({
        currentTick,
        observedTick: convoy.departureTick ?? currentTick,
        confidence01: convoy.confidence01 ?? 0.65,
        provenanceLabel: convoy.provenanceLabel,
      });
      return `${hit.label} — ${convoy.status} (${desc.badgeText})${coordSuffix}`;
    }
  }
  const cell = snapshot.knownCells.find((k) => k.cellId === hit.cellId);
  if (cell && cell.tier !== "unexplored") {
    const desc = describeProvenanceBadge({
      currentTick,
      observedTick: cell.observedTick,
      confidence01: cell.confidence01,
      provenanceLabel: cell.provenanceLabel,
      isLiveSight: cell.tier === "surveyed" ? cell.isLiveHqSight : false,
    });
    return `${hit.label} [${desc.badgeText}]${coordSuffix}`;
  }
  return `${hit.label}${coordSuffix}`;
}

/**
 * Chrome-occlusion-aware ray-picking & hover tooltip controller (`UI-MAP-005`).
 * Aborts hover/click picking whenever a floating window, HUD bar, or modal dialog
 * occludes the cursor coordinate.
 */
export class OcclusionAwareCellPicker implements MapLayerPlugin {
  readonly layerId = "diegetic_picking_and_hover";
  readonly displayName = "Diegetic Picking & Hover";
  private tooltip: HTMLDivElement | null = null;
  private cleanups: Array<() => void> = [];
  private lastHoveredHit: GlobeRayHit | null = null;
  private latestSnapshot: DiegeticSnapshot | null = null;

  public supportsMode(): boolean {
    return true;
  }

  public mount(host: GlobeLayerHost, store: DiegeticStoreAccess, bus: UiIntentBus): void {
    this.dispose();
    this.latestSnapshot = store.getSnapshot();
    const canvas = host.getCanvasElement();
    const overlayRoot = document.getElementById(host.overlayRootId) ?? canvas.parentElement ?? document.body;

    const tip = document.createElement("div");
    tip.id = "map-hover-tooltip";
    tip.className = "ng-map-hover-tooltip";
    tip.setAttribute("role", "status");
    tip.hidden = true;
    overlayRoot.appendChild(tip);
    this.tooltip = tip;

    let downX = 0;
    let downY = 0;
    let pointerDownActive = false;

    const hideHover = (clientX: number, clientY: number) => {
      if (this.tooltip) {
        this.tooltip.hidden = true;
      }
      if (this.lastHoveredHit !== null) {
        this.lastHoveredHit = null;
        bus.emit({ kind: "HoverMapFeature", hit: null, clientX, clientY });
      }
    };

    const onPointerDown = (ev: PointerEvent) => {
      downX = ev.clientX;
      downY = ev.clientY;
      pointerDownActive = !host.isPointOccludedByChrome(ev.clientX, ev.clientY);
    };

    const onPointerMove = (ev: PointerEvent) => {
      if (host.isPointOccludedByChrome(ev.clientX, ev.clientY)) {
        hideHover(ev.clientX, ev.clientY);
        return;
      }
      const { ndcX, ndcY } = clientPointToNdc(canvas, ev.clientX, ev.clientY);
      const hit = host.pickGlobeAtNdc(ndcX, ndcY);
      this.lastHoveredHit = hit;
      if (hit && this.tooltip) {
        this.tooltip.hidden = false;
        this.tooltip.textContent = formatHoverTooltipText(hit, this.latestSnapshot);
        this.tooltip.style.left = `${Math.round(ev.clientX + 14)}px`;
        this.tooltip.style.top = `${Math.round(ev.clientY + 14)}px`;
      } else if (this.tooltip) {
        this.tooltip.hidden = true;
      }
      bus.emit({ kind: "HoverMapFeature", hit, clientX: ev.clientX, clientY: ev.clientY });
    };

    const onPointerLeave = () => {
      hideHover(0, 0);
    };

    const onClick = (ev: MouseEvent) => {
      if (host.isPointOccludedByChrome(ev.clientX, ev.clientY)) {
        hideHover(ev.clientX, ev.clientY);
        return;
      }
      const distMoved = pointerDownActive ? Math.hypot(ev.clientX - downX, ev.clientY - downY) : 0;
      pointerDownActive = false;
      if (distMoved >= 6) return;

      const { ndcX, ndcY } = clientPointToNdc(canvas, ev.clientX, ev.clientY);
      const hit = host.pickGlobeAtNdc(ndcX, ndcY);
      if (!hit) return;

      bus.emit({ kind: "SelectMapFeature", hit });
      bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });

      if (hit.hitKind === "settlement") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "settlement",
          entityId: hit.settlementId,
        });
      } else if (hit.hitKind === "convoy") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "convoy",
          entityId: hit.convoyId,
        });
      } else if (hit.hitKind === "formation") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "formation",
          entityId: hit.formationId,
        });
      } else if (hit.hitKind === "cell" || hit.hitKind === "river" || hit.hitKind === "road") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "cell",
          entityId: hit.cellId,
        });
      }
    };

    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerleave", onPointerLeave);
    canvas.addEventListener("click", onClick);

    this.cleanups.push(() => {
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerleave", onPointerLeave);
      canvas.removeEventListener("click", onClick);
      tip.remove();
    });
  }

  public onSnapshotUpdate(snapshot: DiegeticSnapshot): void {
    this.latestSnapshot = snapshot;
  }

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public dispose(): void {
    while (this.cleanups.length > 0) {
      const fn = this.cleanups.pop();
      fn?.();
    }
    this.tooltip = null;
    this.lastHoveredHit = null;
    this.latestSnapshot = null;
  }
}
