import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import * as THREE from "three";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  GlobeLayerHost,
  GlobeRayHit,
  MockSnapshotPlayer,
  UiIntent,
  UiIntentBus,
  generateGoldbergTopology,
  isValidDiegeticSnapshot,
} from "../src/spine";
import { unitToLatLonDegrees } from "../src/globe";
import { GlobeFeatureOverlayGroup } from "../src/globe/features";
import {
  OcclusionAwareCellPicker,
  OperationalWarfareOverlayLayer,
  OverlayToolbarLayer,
  TradeAndCongestionOverlayLayer,
} from "../src/overlays";
import {
  createEntityInspectorWindowPlugin,
  createInstitutionsInspectorWindowPlugin,
  createKnowledgeArchiveWindowPlugin,
} from "../src/views";
import { createWarfareCommandWindowPlugin, mountSuccessionTransitionModal } from "../src/hud";
import { parsePlayerCommand } from "../electron/ipc-validator";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const CANONICAL_M3_M4 = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

function setupJsdom(): JSDOM {
  const dom = new JSDOM("<!doctype html><html><body><div id='root'></div></body></html>", {
    url: "http://localhost/",
  });
  const proto = dom.window.HTMLDialogElement?.prototype as unknown as {
    open?: boolean;
    returnValue?: string;
    showModal?: () => void;
    close?: (returnValue?: string) => void;
  };
  if (proto && typeof proto.showModal !== "function") {
    Object.defineProperty(proto, "open", {
      configurable: true,
      get(this: HTMLElement) {
        return this.hasAttribute("open");
      },
      set(this: HTMLElement, value: boolean) {
        if (value) this.setAttribute("open", "");
        else this.removeAttribute("open");
      },
    });
    let rv = "";
    Object.defineProperty(proto, "returnValue", {
      configurable: true,
      get() {
        return rv;
      },
      set(v: string) {
        rv = String(v ?? "");
      },
    });
    proto.showModal = function (this: HTMLElement) {
      this.setAttribute("open", "");
    };
    proto.close = function (this: HTMLElement & { returnValue: string }, returnValue?: string) {
      if (returnValue !== undefined) {
        this.returnValue = returnValue;
      }
      this.removeAttribute("open");
      this.dispatchEvent(new dom.window.Event("close"));
    };
  }

  (globalThis as unknown as { document: Document }).document = dom.window.document;
  (globalThis as unknown as { window: Window }).window = dom.window as unknown as Window;
  (globalThis as unknown as { HTMLElement: typeof HTMLElement }).HTMLElement = dom.window.HTMLElement;
  (globalThis as unknown as { getComputedStyle: typeof getComputedStyle }).getComputedStyle =
    dom.window.getComputedStyle.bind(dom.window);
  return dom;
}

function buildM5M6M7Snapshot(): DiegeticSnapshot {
  const topo = generateGoldbergTopology(CANONICAL_M3_M4.globeTopology.subdivision);
  const c0 = topo.cells[0]?.centerUnit ?? [0, 1, 0];
  const c1 = topo.cells[1]?.centerUnit ?? [0.2, 0.96, 0.1];
  const c2 = topo.cells[2]?.centerUnit ?? [0.4, 0.9, 0.15];
  const c4 = topo.cells[4]?.centerUnit ?? [0.55, 0.8, 0.22];

  const baseKnownIds = new Set(CANONICAL_M3_M4.knownCells.map((c) => c.cellId));
  const extraCells = [0, 1, 2, 4]
    .filter((id) => !baseKnownIds.has(id))
    .map((id) => ({
      tier: "surveyed" as const,
      cellId: id,
      observedTick: 72,
      isLiveHqSight: id === 0,
      elevationM: 180 + id * 45,
      confidence01: 0.94,
      provenanceLabel: "HQ Telemetry Relay",
      biomeKey: "temperate_grassland",
      isOcean: false,
    }));

  return {
    ...CANONICAL_M3_M4,
    clockAndNotifications: {
      ...CANONICAL_M3_M4.clockAndNotifications,
      tick: 72,
      paused: true,
    },
    knownCells: [...CANONICAL_M3_M4.knownCells, ...extraCells],
    knownSettlements: [
      { settlementId: 0, settlementName: "Haven Landing Perimeter", cellId: 0, isHq: true },
      { settlementId: 1, settlementName: "Highland Quarry Depot", cellId: 2, isHq: false },
    ],
    trackedConvoys: [
      {
        convoyId: 401,
        label: "Quarry Ore Train",
        status: "en_route_projected",
        projectedCellId: 1,
        originCellId: 0,
        destinationCellId: 2,
        routeCells: [0, 1, 2],
        currentUnitXyz: c1,
        originUnitXyz: c0,
        destinationUnitXyz: c2,
        routeWaypointsXyz: [c0, c1, c2],
        progress01: 0.5,
        departureTick: 65,
        expectedArrivalTick: 78,
        confidence01: 0.78,
        provenanceLabel: "Dispatch Waybill",
      },
    ],
    nonStateEconomy: {
      actors: [
        {
          actorId: 100,
          name: "Haven Free Carters",
          kind: "MerchantHouse",
          homeSettlementId: 0,
          settlementName: "Haven Landing Perimeter",
          workingCapitalCoins: 640,
          heldCommodityKey: "refined_copper",
          heldMassKg: 420,
          hoardedMassKg: 110,
          privateCarriersOwned: 6,
          charteredToStateCarriers: 1,
          compliance01: 0.82,
          observedTick: 71,
          confidence01: 0.88,
          provenanceLabel: "Harbor Customs Ledger",
        },
        {
          actorId: 101,
          name: "Highland Smelters Guild",
          kind: "CraftGuild",
          homeSettlementId: 1,
          settlementName: "Highland Quarry Depot",
          workingCapitalCoins: 510,
          heldCommodityKey: "tools",
          heldMassKg: 260,
          hoardedMassKg: 45,
          privateCarriersOwned: 3,
          charteredToStateCarriers: 0,
          compliance01: 0.91,
          observedTick: 69,
          confidence01: 0.79,
          provenanceLabel: "Guild Factor Receipt",
        },
      ],
      informalMarkets: [
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          commodityKey: "refined_copper",
          commodityName: "Refined Copper",
          clearingPriceIndex: 1.25,
          blackMarketPremium01: 0.15,
          blackMarketLeakageKg: 42,
          hoardedReserveKg: 95,
          observedTick: 71,
          confidence01: 0.84,
          provenanceLabel: "Market Warden Tally",
        },
        {
          settlementId: 1,
          settlementName: "Highland Quarry Depot",
          commodityKey: "refined_copper",
          commodityName: "Refined Copper",
          clearingPriceIndex: 1.72,
          blackMarketPremium01: 0.45,
          blackMarketLeakageKg: 68,
          hoardedReserveKg: 140,
          observedTick: 68,
          confidence01: 0.74,
          provenanceLabel: "Post Courier Price Slip",
        },
      ],
      congestedEdges: [
        {
          edgeId: 14,
          fromCellId: 0,
          toCellId: 2,
          hourlyFlowTonnes: 19.4,
          capacityTonnes: 10.5,
          congestionMultiplier: 1.85,
        },
        {
          edgeId: 15,
          fromCellId: 0,
          toCellId: 4,
          hourlyFlowTonnes: 3.0,
          capacityTonnes: 10.0,
          congestionMultiplier: 1.0,
        },
      ],
    },
    governance: {
      activeModelKey: "provisional_expedition_council",
      activeModelName: "Provisional Expedition Authority",
      previousModelKey: null,
      adaptationProgress01: 0.75,
      activePolicyKey: "charter_monopoly",
      activePolicyName: "Charter Monopoly",
      taxOrTributeRate01: 0.12,
      patentDurationYears: 4,
      skimmingLeakageFraction: 0.08,
      effectiveAttentionCapacity: 4.5,
      clericalOverheadPerMandate: 0.35,
      hierarchyHopLatencyHours: 3.0,
      charteredCarriersTotal: 1,
      bureaucraticCaptureEvents: 0,
      availableModels: [
        {
          modelKey: "provisional_expedition_council",
          displayName: "Provisional Expedition Authority",
          baseAttentionCapacity: 4.5,
          clericalOverheadPerMandate: 0.35,
          hierarchyHopLatencyHours: 3.0,
        },
        {
          modelKey: "constitutional_cabinet",
          displayName: "Constitutional Cabinet",
          baseAttentionCapacity: 6.0,
          clericalOverheadPerMandate: 0.45,
          hierarchyHopLatencyHours: 4.0,
        },
      ],
      availablePolicies: [
        {
          policyKey: "charter_monopoly",
          displayName: "Charter Monopoly",
          taxOrTributeRate01: 0.12,
          patentDurationYears: 4,
        },
        {
          policyKey: "customs_toll",
          displayName: "Customs Toll",
          taxOrTributeRate01: 0.18,
          patentDurationYears: 5,
        },
      ],
    },
    knowledge: {
      availableModes: ["artisan_secrecy", "public_academy", "patron_patent"],
      settlements: [
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          innovationModeKey: "artisan_secrecy",
          bottleneckPressure01: 0.62,
          leisureSurplus01: 0.28,
          knowHow: [
            {
              componentId: 1,
              componentKey: "bronze_smelting",
              displayName: "Field Bronze Smelting",
              knowHow01: 0.72,
            },
            {
              componentId: 2,
              componentKey: "water_wheel",
              displayName: "Overshot Water Wheel",
              knowHow01: 0.85,
            },
          ],
          knowWhy: [
            {
              domainId: 1,
              domainKey: "metallurgy_assays",
              displayName: "Applied Metallurgy & Slag Flux",
              knowWhy01: 0.44,
            },
          ],
          activePatentHolderActorId: 100,
          patentExpiryTick: 240,
          regressionLosses: 0,
          observedTick: 72,
          confidence01: 0.91,
          provenanceLabel: "Workshop Bench Inspection",
        },
      ],
      carriers: [
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          componentKey: "bronze_smelting",
          carrierKind: "PractitionerCohort",
          retentionStrength01: 0.8,
          holderLabel: "Smelter Apprentice Group",
        },
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          componentKey: "water_wheel",
          carrierKind: "PractitionerCohort",
          retentionStrength01: 0.88,
          holderLabel: "Millwright Crew",
        },
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          componentKey: "water_wheel",
          carrierKind: "WorkingPrototypeArtifact",
          retentionStrength01: 0.95,
          holderLabel: "Creek Mill Frame 1",
        },
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          componentKey: "water_wheel",
          carrierKind: "PhysicalTreatise",
          retentionStrength01: 0.92,
          holderLabel: "Millwright Ledger Foliation",
        },
      ],
    },
    warfare: {
      availableDoctrines: [
        {
          doctrineKey: "elastic_defense",
          displayName: "Elastic Defense",
          initiativeBonus: 0.25,
          strictObedienceWeight: 0.4,
          rainPenaltyMitigation: 0.15,
        },
        {
          doctrineKey: "relentless_hold",
          displayName: "Relentless Hold",
          initiativeBonus: 0.05,
          strictObedienceWeight: 0.95,
          rainPenaltyMitigation: 0.05,
        },
      ],
      theaters: [
        {
          theaterId: 1,
          commanderId: 1,
          commanderName: "Marshal E. Vance",
          objectiveCellId: 4,
          supplyPriority01: 0.8,
          issuedTick: 60,
          autonomousWithdrawals: 0,
        },
      ],
      formations: [
        {
          formationId: 1,
          label: "First Perimeter Column",
          templateKey: "light_infantry_column",
          theaterId: 1,
          commanderId: 1,
          commanderName: "Marshal E. Vance",
          homeSettlementId: 0,
          currentCellId: 2,
          currentUnitXyz: c2,
          targetCellId: 4,
          targetUnitXyz: c4,
          personnelCount: 240,
          equipmentMassKg: 4800,
          munitionsDays: 14.0,
          rationsDays: 18.0,
          cohesion01: 0.88,
          doctrineKey: "relentless_hold",
          noRetreatOrder: true,
          encircled: false,
          insubordinateWithdrawal: false,
          staleOrderTicks: 2,
          observedTick: 70,
          confidence01: 0.9,
          provenanceLabel: "Dispatch Rider Sighting",
        },
        {
          formationId: 2,
          label: "Frontier Survey Detachment",
          templateKey: "mounted_scout_band",
          theaterId: 1,
          commanderId: 1,
          commanderName: "Marshal E. Vance",
          homeSettlementId: 1,
          currentCellId: 1,
          currentUnitXyz: c1,
          targetCellId: 1,
          targetUnitXyz: c1,
          personnelCount: 85,
          equipmentMassKg: 1450,
          munitionsDays: 9.0,
          rationsDays: 11.0,
          cohesion01: 0.72,
          doctrineKey: "elastic_defense",
          noRetreatOrder: false,
          encircled: false,
          insubordinateWithdrawal: false,
          staleOrderTicks: 16,
          observedTick: 50,
          confidence01: 0.48,
          provenanceLabel: "Dated Beacon Log",
        },
      ],
    },
    succession: {
      effectiveCommandMultiplier: 0.76,
      roster: [
        {
          characterId: 1,
          name: "Governor R. Halstead",
          roleTitle: "Acting Regent",
          ageYears: 44,
          health01: 0.91,
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
        transitionTick: 72,
        disruptionUntilTick: 96,
        inheritedArchiveEntries: [
          "Haven Grain & Slag Ledger Vol. I",
          "Water Wheel Timber Pattern Plates",
        ],
        lostUnrecordedRapportCount: 4,
        awaitingPlayerAcknowledgement: true,
      },
    },
  };
}

describe("M5, M6, M7 & Option B UI Overlays, Institutional Views, Warfare & Succession", () => {
  // @req UI-GLB-001.1
  // @req UI-MAP-005.1
  it("converts spherical surface coordinates continuously and dispatches formation/continuous picking intents (UI-GLB-001.1, UI-MAP-005.1)", () => {
    const dom = setupJsdom();
    const snap = buildM5M6M7Snapshot();
    expect(isValidDiegeticSnapshot(snap)).toBe(true);

    const northPole = unitToLatLonDegrees([0, 1, 0]);
    expect(northPole.latDeg).toBeCloseTo(90, 4);
    const primeMeridian = unitToLatLonDegrees([0, 0, 1]);
    expect(primeMeridian.latDeg).toBeCloseTo(0, 4);
    expect(primeMeridian.lonDeg).toBeCloseTo(0, 4);
    const eastMeridian = unitToLatLonDegrees([1, 0, 0]);
    expect(eastMeridian.latDeg).toBeCloseTo(0, 4);
    expect(eastMeridian.lonDeg).toBeCloseTo(90, 4);

    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();
    const emitted: UiIntent[] = [];
    bus.subscribe((intent) => emitted.push(intent));

    const root = dom.window.document.getElementById("root")!;
    const canvas = dom.window.document.createElement("canvas");
    Object.defineProperty(canvas, "getBoundingClientRect", {
      value: () => ({ left: 0, top: 0, width: 400, height: 300, right: 400, bottom: 300 }),
    });
    root.appendChild(canvas);

    const c2 = snap.warfare!.formations[0].currentUnitXyz;
    const coords = unitToLatLonDegrees(c2);
    const fakeHit: GlobeRayHit = {
      hitKind: "formation",
      formationId: 1,
      cellId: 2,
      label: "First Perimeter Column (Marshal E. Vance)",
      unitXyz: c2,
      latDeg: coords.latDeg,
      lonDeg: coords.lonDeg,
    };

    const host: GlobeLayerHost = {
      overlayRootId: "root",
      getCanvasElement: () => canvas,
      isPointOccludedByChrome: () => false,
      pickGlobeAtNdc: () => fakeHit,
      setOverlayMode: () => {},
    };

    const picker = new OcclusionAwareCellPicker();
    picker.mount(host, store, bus);
    picker.onSnapshotUpdate(snap);

    canvas.dispatchEvent(new dom.window.MouseEvent("pointermove", { clientX: 120, clientY: 90 }));
    const tip = dom.window.document.getElementById("map-hover-tooltip");
    expect(tip?.hidden).toBe(false);
    expect(tip?.textContent).toContain("First Perimeter Column");

    canvas.dispatchEvent(new dom.window.MouseEvent("click", { clientX: 120, clientY: 90 }));
    const focus = emitted.find((i) => i.kind === "FocusEntityInspector");
    expect(focus).toEqual({
      kind: "FocusEntityInspector",
      entityKind: "formation",
      entityId: 1,
    });
    picker.dispose();
  });

  // @req UI-MAP-006.1
  // @req UI-MAP-006.2
  it("renders M5 trade flow ribbons and highlights congested corridors exceeding capacity (UI-MAP-006.1, UI-MAP-006.2)", () => {
    const snap = buildM5M6M7Snapshot();
    const topo = generateGoldbergTopology(snap.globeTopology.subdivision);

    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();

    const tradeLayer = new TradeAndCongestionOverlayLayer();
    expect(tradeLayer.supportsMode("trade")).toBe(true);
    tradeLayer.onSnapshotUpdate(snap);
    const layerSummary = tradeLayer.getSummary()!;
    expect(layerSummary.tradeRibbonCount).toBe(2);
    expect(layerSummary.regionalMarketCount).toBe(2);
    expect(layerSummary.maxPricePremiumPct).toBe(45);
    expect(layerSummary.congestedEdgeCount).toBe(1);
    expect(layerSummary.maxCongestionMultiplier).toBeCloseTo(1.85, 2);
    expect(layerSummary.latestMarketObservedTick).toBe(71);

    const features = new GlobeFeatureOverlayGroup();
    features.rebuild(snap, topo, "trade", null);
    const geomSummary = features.getSummary();
    expect(geomSummary.tradeRibbonCount).toBe(2);
    expect(geomSummary.congestedCorridorCount).toBe(1);
    expect(features.group.children.length).toBeGreaterThan(0);

    features.dispose();
    tradeLayer.dispose();
    bus.emit({ kind: "SelectOverlayMode", modeId: "trade" });
  });

  // @req UI-MAP-007.1
  // @req UI-MAP-007.2
  it("renders M7 theater vectors, phase lines, no-retreat rings, and stale vs fresh formation observation age (UI-MAP-007.1, UI-MAP-007.2)", () => {
    const dom = setupJsdom();
    const snap = buildM5M6M7Snapshot();
    const topo = generateGoldbergTopology(snap.globeTopology.subdivision);

    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();

    const warLayer = new OperationalWarfareOverlayLayer();
    expect(warLayer.supportsMode("warfare")).toBe(true);
    warLayer.onSnapshotUpdate(snap);
    const summary = warLayer.getSummary()!;
    expect(summary.theaterCount).toBe(1);
    expect(summary.formationCount).toBe(2);
    expect(summary.staleFormationCount).toBe(1);
    expect(summary.noRetreatFormationCount).toBe(1);

    const features = new GlobeFeatureOverlayGroup();
    features.rebuild(snap, topo, "warfare", null);
    const fSummary = features.getSummary();
    expect(fSummary.theaterVectorCount).toBe(1);
    expect(fSummary.formationCount).toBe(2);
    expect(fSummary.noRetreatCount).toBe(1);

    const boxMarkers = features.group.children.filter(
      (c) => c instanceof THREE.Mesh && c.geometry instanceof THREE.BoxGeometry,
    );
    const octaMarkers = features.group.children.filter(
      (c) => c instanceof THREE.Mesh && c.geometry instanceof THREE.OctahedronGeometry,
    );
    expect(boxMarkers.length).toBeGreaterThanOrEqual(1);
    expect(octaMarkers.length).toBeGreaterThanOrEqual(1);

    const host: GlobeLayerHost = {
      overlayRootId: "root",
      getCanvasElement: () => dom.window.document.createElement("canvas"),
      isPointOccludedByChrome: () => false,
      pickGlobeAtNdc: () => null,
      setOverlayMode: () => {},
    };
    const toolbar = new OverlayToolbarLayer();
    toolbar.mount(host, store, bus);
    toolbar.onSnapshotUpdate(snap);
    const tradeBtn = dom.window.document.querySelector<HTMLButtonElement>("[data-overlay-mode='trade']")!;
    const warBtn = dom.window.document.querySelector<HTMLButtonElement>("[data-overlay-mode='warfare']")!;
    expect(tradeBtn).toBeTruthy();
    expect(warBtn).toBeTruthy();
    warBtn.click();
    expect(dom.window.document.getElementById("map-overlay-legend-summary")?.textContent).toContain("Theaters 1");

    toolbar.dispose();
    features.dispose();
    warLayer.dispose();
  });

  // @req UI-DAT-006.1
  // @req UI-DAT-006.2
  it("renders accessible institutional tabs, separates non-state actors from state warehouses, and dispatches M6 governance reforms (UI-DAT-006.1, UI-DAT-006.2)", async () => {
    const dom = setupJsdom();
    const snap = buildM5M6M7Snapshot();
    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();
    const mockPlayer = new MockSnapshotPlayer([snap]);

    const emitted: UiIntent[] = [];
    bus.subscribe((intent) => emitted.push(intent));

    const host = dom.window.document.getElementById("root")!;
    const plugin = createInstitutionsInspectorWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    const tabs = host.querySelectorAll<HTMLButtonElement>('[role="tab"]');
    expect(tabs.length).toBe(3);

    const marketsPanel = host.querySelector<HTMLElement>('[role="tabpanel"][data-panel-key="markets"]')!;
    expect(marketsPanel.getAttribute("hidden")).toBe("until-found");

    // Non-state actor disclosures must NEVER contain heading elements inside <summary>.
    const summaries = host.querySelectorAll("details.ng-disclosure summary");
    expect(summaries.length).toBeGreaterThanOrEqual(2);
    for (const s of Array.from(summaries)) {
      expect(s.querySelector("h1, h2, h3, h4, h5, h6, [role='heading']")).toBeNull();
    }
    expect(host.textContent).toContain("State Official Ledgers");
    expect(host.textContent).toContain("Haven Free Carters");

    // Verify find-in-page beforematch reveal activates the hidden tab panel cleanly.
    marketsPanel.dispatchEvent(new dom.window.Event("beforematch"));
    expect(marketsPanel.hasAttribute("hidden")).toBe(false);
    expect(marketsPanel.textContent).toContain("Spread +45%");
    expect(marketsPanel.textContent).toContain("Congestion 1.85x");

    const govTab = host.querySelector<HTMLButtonElement>('[role="tab"][data-tab-key="governance"]')!;
    govTab.click();
    const progress = host.querySelector<HTMLElement>(".ng-inertia-bar[role='progressbar']");
    expect(progress?.getAttribute("aria-valuenow")).toBe("75");

    const modelSelect = host.querySelector<HTMLSelectElement>('[data-gov-model-select="true"]')!;
    modelSelect.value = "constitutional_cabinet";
    host.querySelector<HTMLButtonElement>('[data-gov-model-submit="true"]')!.click();

    const policySelect = host.querySelector<HTMLSelectElement>('[data-econ-policy-select="true"]')!;
    policySelect.value = "customs_toll";
    host.querySelector<HTMLInputElement>('[data-econ-rate-input="true"]')!.value = "0.22";
    host.querySelector<HTMLButtonElement>('[data-econ-policy-submit="true"]')!.click();

    host.querySelector<HTMLInputElement>('[data-charter-count-input="true"]')!.value = "3";
    host.querySelector<HTMLButtonElement>('[data-charter-submit="true"]')!.click();

    const dispatched = emitted.filter(
      (i): i is Extract<UiIntent, { kind: "DispatchSimCommand" }> => i.kind === "DispatchSimCommand",
    );
    expect(dispatched.map((d) => d.command.kind)).toEqual([
      "switchGovernanceModel",
      "enactEconomicPolicy",
      "charterPrivateCarriers",
    ]);

    for (const d of dispatched) {
      const ack = await mockPlayer.sendPlayerCommand(d.command);
      expect(ack.accepted).toBe(true);
    }
    const updatedSnap = (await mockPlayer.getInitialState());
    expect(updatedSnap.state).toBe("ready");
    if (updatedSnap.state === "ready") {
      expect(updatedSnap.snapshot.governance?.activeModelKey).toBe("constitutional_cabinet");
      expect(updatedSnap.snapshot.governance?.activePolicyKey).toBe("customs_toll");
      expect(updatedSnap.snapshot.governance?.charteredCarriersTotal).toBe(4);
    }

    dispose();
  });

  // @req UI-DAT-007.1
  // @req UI-DAT-007.2
  it("renders empirical 4-carrier fragility badges without exposing bought tech-trees and enforces IPC prohibition (UI-DAT-007.1, UI-DAT-007.2)", async () => {
    const dom = setupJsdom();
    const snap = buildM5M6M7Snapshot();
    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();
    const mockPlayer = new MockSnapshotPlayer([snap]);

    const emitted: UiIntent[] = [];
    bus.subscribe((intent) => emitted.push(intent));

    const host = dom.window.document.getElementById("root")!;
    const plugin = createKnowledgeArchiveWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    // No future tech tree or science point currency exists anywhere in UI markup.
    const fullText = host.textContent ?? "";
    expect(fullText).not.toMatch(/Tech Tree|Science Points|Buy Tech|Unlock Node/i);

    const bronzeItem = host.querySelector<HTMLElement>('[data-component-key="bronze_smelting"]')!;
    const bronzeBadge = bronzeItem.querySelector<HTMLElement>(".ng-fragility-badge")!;
    expect(bronzeBadge.getAttribute("data-fragility-risk")).toBe("endangered");
    expect(bronzeItem.textContent).toContain("Cohorts 1 | Specialists 0 | Prototypes 0 | Archives 0");

    const wheelItem = host.querySelector<HTMLElement>('[data-component-key="water_wheel"]')!;
    const wheelBadge = wheelItem.querySelector<HTMLElement>(".ng-fragility-badge")!;
    expect(wheelBadge.getAttribute("data-fragility-risk")).toBe("replicated");
    expect(wheelBadge.textContent).toContain("3/4 Carriers");

    const treatiseTab = host.querySelector<HTMLButtonElement>('[role="tab"][data-tab-key="treatises"]')!;
    treatiseTab.click();
    expect(host.textContent).toContain("Applied Metallurgy & Slag Flux");

    const patentsTab = host.querySelector<HTMLButtonElement>('[role="tab"][data-tab-key="patents"]')!;
    patentsTab.click();
    const modeSelect = host.querySelector<HTMLSelectElement>('[data-innovation-mode-select="true"]')!;
    modeSelect.value = "public_academy";
    host.querySelector<HTMLButtonElement>('[data-innovation-submit="true"]')!.click();

    const dispatched = emitted.filter(
      (i): i is Extract<UiIntent, { kind: "DispatchSimCommand" }> => i.kind === "DispatchSimCommand",
    );
    expect(dispatched).toHaveLength(1);
    expect(dispatched[0].command).toEqual({
      kind: "setInnovationMode",
      settlement: 0,
      modeKey: "public_academy",
    });
    const ack = await mockPlayer.sendPlayerCommand(dispatched[0].command);
    expect(ack.accepted).toBe(true);

    const forbidden = parsePlayerCommand({ kind: "buyOrSelectTechTarget", techKey: "steam_engine" });
    expect(forbidden.ok).toBe(false);
    if (!forbidden.ok) {
      expect(forbidden.errorCode).toBe("ERR_BOUGHT_TECH_FORBIDDEN");
    }

    dispose();
  });

  // @req UI-HUD-007.1
  it("contrasts Churchill theater delegation vs Hitler operational battle plans while forbidding tactical unit micro (UI-HUD-007.1)", async () => {
    const dom = setupJsdom();
    const snap = buildM5M6M7Snapshot();
    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();
    const mockPlayer = new MockSnapshotPlayer([snap]);

    const emitted: UiIntent[] = [];
    bus.subscribe((intent) => emitted.push(intent));

    const host = dom.window.document.getElementById("root")!;
    const plugin = createWarfareCommandWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    const churchill = host.querySelector<HTMLElement>('[data-warfare-attention-preview="churchill"]')!;
    const hitler = host.querySelector<HTMLElement>('[data-warfare-attention-preview="hitler"]')!;
    expect(churchill.textContent).toContain("Low (1.5 slices)");
    expect(hitler.textContent).toContain("High (6.0 slices)");

    const buttonLabels = Array.from(host.querySelectorAll("button")).map((b) => b.textContent ?? "");
    expect(buttonLabels.some((t) => /Micro|Attack Squad|Move Platoon/i.test(t))).toBe(false);

    host.querySelector<HTMLInputElement>('[data-theater-objective-input="true"]')!.value = "4";
    host.querySelector<HTMLButtonElement>('[data-theater-submit="true"]')!.click();

    host.querySelector<HTMLButtonElement>('[role="tab"][data-tab-key="hitler_operational"]')!.click();
    host.querySelector<HTMLInputElement>('[data-plan-target-input="true"]')!.value = "4";
    host.querySelector<HTMLInputElement>('[data-plan-noretreat-checkbox="true"]')!.checked = true;
    host.querySelector<HTMLButtonElement>('[data-plan-submit="true"]')!.click();

    const dispatched = emitted.filter(
      (i): i is Extract<UiIntent, { kind: "DispatchSimCommand" }> => i.kind === "DispatchSimCommand",
    );
    expect(dispatched.map((d) => d.command.kind)).toEqual([
      "issueTheaterDirective",
      "issueOperationalBattlePlan",
    ]);

    for (const d of dispatched) {
      const ack = await mockPlayer.sendPlayerCommand(d.command);
      expect(ack.accepted).toBe(true);
    }

    const forbiddenMicro = parsePlayerCommand({ kind: "issueTacticalMicroCommand", unitId: 1 });
    expect(forbiddenMicro.ok).toBe(false);
    if (!forbiddenMicro.ok) {
      expect(forbiddenMicro.errorCode).toBe("ERR_TACTICAL_MICRO_FORBIDDEN");
    }

    // Inspector opens formation card readouts cleanly when clicked from map.
    const inspHost = dom.window.document.createElement("div");
    dom.window.document.body.appendChild(inspHost);
    const insp = createEntityInspectorWindowPlugin();
    const disposeInsp = insp.mountContent(inspHost, "formation:1", { store, bus });
    expect(inspHost.textContent).toContain("First Perimeter Column");
    expect(inspHost.textContent).toContain("No Retreat");
    disposeInsp();
    dispose();
  });

  // @req UI-HUD-007.2
  it("opens native <dialog> via showModal() on generational succession with accessible headings and resets returnValue cleanly (UI-HUD-007.2)", async () => {
    const dom = setupJsdom();
    const snap = buildM5M6M7Snapshot();
    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const bus = new UiIntentBus();
    const mockPlayer = new MockSnapshotPlayer([snap]);

    const emitted: UiIntent[] = [];
    bus.subscribe((intent) => emitted.push(intent));

    const host = dom.window.document.getElementById("root")!;
    const dispose = mountSuccessionTransitionModal(host, store, bus);

    const dialog = dom.window.document.getElementById("succession-transition-dialog") as HTMLDialogElement;
    expect(dialog).toBeTruthy();
    expect(dialog.open).toBe(true);
    expect(dialog.getAttribute("aria-labelledby")).toBe("succession-dialog-title");
    expect(dialog.getAttribute("role")).toBeNull();
    expect(dialog.getAttribute("aria-modal")).toBeNull();

    for (const summary of Array.from(dialog.querySelectorAll("summary"))) {
      expect(summary.querySelector("h1, h2, h3, h4, h5, h6, [role='heading']")).toBeNull();
    }

    const lossBanner = dialog.querySelector<HTMLElement>('[data-succession-unrecorded-loss="true"]')!;
    expect(lossBanner.textContent).toContain("4");
    expect(dialog.textContent).toContain("Haven Grain & Slag Ledger Vol. I");

    const sealBtn = dialog.querySelector<HTMLButtonElement>('[data-acknowledge-succession="true"]')!;
    sealBtn.click();

    expect(dialog.open).toBe(false);
    expect(dialog.returnValue).toBe("");
    const cmd = emitted.find(
      (i): i is Extract<UiIntent, { kind: "DispatchSimCommand" }> => i.kind === "DispatchSimCommand",
    );
    expect(cmd?.command).toEqual({ kind: "acknowledgeSuccession" });
    expect(emitted.some((i) => i.kind === "PlayUiFoley" && i.cueId === "seal_stamp")).toBe(true);

    const ack = await mockPlayer.sendPlayerCommand({ kind: "acknowledgeSuccession" });
    expect(ack.accepted).toBe(true);
    const nextState = await mockPlayer.getInitialState();
    if (nextState.state === "ready") {
      expect(nextState.snapshot.succession?.pendingTransition).toBeNull();
      expect(nextState.snapshot.clockAndNotifications.autoPauseReason).toBeNull();
    }

    dispose();
  });
});
