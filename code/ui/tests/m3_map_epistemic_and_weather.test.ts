import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  KnownCellDto,
  UiIntentBus,
  generateGoldbergTopology,
} from "../src/spine";
import { createGlobeShaderController } from "../src/globe";
import { GlobeFeatureOverlayGroup } from "../src/globe/features";
import { summarizeOverlaySnapshot } from "../src/overlays";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const CANONICAL_M3_M4 = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

describe("M3 & M4 Diegetic Map Tiers, Live HQ Halo, Dead-Reckoning Convoys & Weather Overlays", () => {
  // @req UI-MAP-001
  it("visually encodes Unexplored (0), Rumored (42), Stale (85), and Surveyed (170+) shading tiers while preserving the air-gap (UI-MAP-001)", () => {
    const syntheticCells: KnownCellDto[] = [
      { tier: "unexplored", cellId: 10 },
      {
        tier: "rumored",
        cellId: 11,
        observedTick: 2,
        elevationM: 210,
        confidence01: 0.35,
        provenanceLabel: "Scout Rumor",
        biomeKey: "steppe",
        isOcean: false,
      },
      {
        tier: "stale",
        cellId: 12,
        observedTick: 4,
        lastLiveTick: 18,
        elevationM: 480,
        confidence01: 0.62,
        provenanceLabel: "2-Year-Old Survey",
        biomeKey: "temperate_forest",
        isOcean: false,
      },
      {
        tier: "surveyed",
        cellId: 13,
        observedTick: 70,
        isLiveHqSight: false,
        elevationM: 760,
        confidence01: 0.91,
        provenanceLabel: "Recent Expedition",
        biomeKey: "highland",
        isOcean: false,
      },
    ];

    const snap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      knownCells: syntheticCells,
      observedWeather: [],
    };

    const shader = createGlobeShaderController();
    shader.uploadSnapshot(snap, null, null);
    const texData = (shader.material.uniforms.uCellDataTex.value as { image: { data: Uint8Array } }).image.data;

    expect([texData[10 * 4], texData[10 * 4 + 1], texData[10 * 4 + 2], texData[10 * 4 + 3]]).toEqual([0, 0, 0, 0]);
    expect(texData[11 * 4 + 3]).toBe(42);
    expect(texData[12 * 4 + 3]).toBe(85);
    expect(texData[13 * 4 + 3]).toBe(170);

    const summary = summarizeOverlaySnapshot(snap, "epistemic");
    expect(summary.unexploredCount).toBe(1);
    expect(summary.rumoredCount).toBe(1);
    expect(summary.staleCount).toBe(1);
    expect(summary.surveyedCount).toBe(1);

    shader.dispose();
  });

  // @req UI-MAP-002
  it("distinguishes Live Immediate-HQ Sight Halo (alpha=192) from dated remote surveys and shifts on HQ relocation (UI-MAP-002)", () => {
    const initialSnap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      knownCells: [
        {
          tier: "surveyed",
          cellId: 82,
          observedTick: 72,
          isLiveHqSight: true,
          elevationM: 120,
          confidence01: 0.99,
          provenanceLabel: "Direct HQ Observation",
          biomeKey: "temperate_forest",
          isOcean: false,
        },
        {
          tier: "surveyed",
          cellId: 85,
          observedTick: 60,
          isLiveHqSight: false,
          elevationM: 340,
          confidence01: 0.85,
          provenanceLabel: "Forward Post Survey",
          biomeKey: "highland",
          isOcean: false,
        },
      ],
      observedWeather: [],
    };

    const shader = createGlobeShaderController();
    shader.uploadSnapshot(initialSnap, null, null);
    const texData = (shader.material.uniforms.uCellDataTex.value as { image: { data: Uint8Array } }).image.data;

    expect(texData[82 * 4 + 3]).toBe(192);
    expect(texData[85 * 4 + 3]).toBe(170);

    // Relocate HQ from Cell #82 to Cell #85: clarity halo shifts cleanly onto #85 while #82 dims to 170.
    const relocatedSnap: DiegeticSnapshot = {
      ...initialSnap,
      hqPresence: {
        ...initialSnap.hqPresence,
        cellId: 85,
        settlementName: "Forward Post #85",
      },
      knownCells: [
        {
          ...(initialSnap.knownCells[0] as Extract<KnownCellDto, { tier: "surveyed" }>),
          isLiveHqSight: false,
        },
        {
          ...(initialSnap.knownCells[1] as Extract<KnownCellDto, { tier: "surveyed" }>),
          isLiveHqSight: true,
          observedTick: 73,
        },
      ],
    };

    shader.uploadSnapshot(relocatedSnap, null, null);
    const nextData = (shader.material.uniforms.uCellDataTex.value as { image: { data: Uint8Array } }).image.data;
    expect(nextData[82 * 4 + 3]).toBe(170);
    expect(nextData[85 * 4 + 3]).toBe(192);

    shader.dispose();
  });

  // @req UI-MAP-003
  it("tracks dead-reckoning convoy progression from projected -> overdue -> confirmed and emits inspector sync (UI-MAP-003)", () => {
    const topo = generateGoldbergTopology(CANONICAL_M3_M4.globeTopology.subdivision);
    const features = new GlobeFeatureOverlayGroup();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();

    const projectedSnap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      trackedConvoys: [
        {
          convoyId: 401,
          label: "North Ridge Supply Column",
          status: "en_route_projected",
          projectedCellId: 85,
          originCellId: 82,
          destinationCellId: 81,
          routeCells: [82, 85, 81],
          departureTick: 50,
          expectedArrivalTick: 84,
          confidence01: 0.65,
          provenanceLabel: "Dead Reckoning Projection",
        },
      ],
    };

    store.ingestSnapshot(projectedSnap);
    features.rebuild(projectedSnap, topo, "logistics", null);
    expect(features.getSummary().projectedConvoyCount).toBe(1);
    expect(features.getSummary().overdueConvoyCount).toBe(0);

    const overdueSnap: DiegeticSnapshot = {
      ...projectedSnap,
      trackedConvoys: [
        {
          ...projectedSnap.trackedConvoys[0],
          status: "overdue",
          projectedCellId: 81,
          confidence01: 0.4,
        },
      ],
    };
    store.ingestSnapshot(overdueSnap);
    features.rebuild(overdueSnap, topo, "logistics", null);
    expect(features.getSummary().overdueConvoyCount).toBe(1);

    const confirmedSnap: DiegeticSnapshot = {
      ...projectedSnap,
      trackedConvoys: [
        {
          ...projectedSnap.trackedConvoys[0],
          status: "confirmed_sighting",
          projectedCellId: 81,
          confidence01: 0.99,
        },
      ],
    };
    store.ingestSnapshot(confirmedSnap);
    features.rebuild(confirmedSnap, topo, "logistics", null);
    expect(features.getSummary().confirmedConvoyCount).toBe(1);

    const focusedEvents: number[] = [];
    bus.subscribe((intent) => {
      if (intent.kind === "FocusEntityInspector" && intent.entityKind === "convoy") {
        focusedEvents.push(intent.entityId);
      }
    });
    bus.emit({ kind: "FocusEntityInspector", entityKind: "convoy", entityId: 401 });
    expect(focusedEvents).toEqual([401]);

    features.dispose();
  });

  // @req UI-MAP-004
  it("renders storm wetness bytes, flooded river widening, and wind vectors strictly inside observed weather cells (UI-MAP-004)", () => {
    const topo = generateGoldbergTopology(CANONICAL_M3_M4.globeTopology.subdivision);
    const shader = createGlobeShaderController();
    const features = new GlobeFeatureOverlayGroup();

    const floodedCellId = 82;
    const neighborCellId = topo.cells.find((c) => c.cellId === floodedCellId)?.neighbors[0] ?? 85;
    const unobservedCellId = 10;

    const stormSnap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      knownCells: [
        {
          tier: "surveyed",
          cellId: floodedCellId,
          observedTick: 72,
          isLiveHqSight: true,
          elevationM: 220,
          confidence01: 0.99,
          provenanceLabel: "HQ Weather Log",
          biomeKey: "temperate_forest",
          isOcean: false,
          riverDownstreamCellId: neighborCellId,
          riverFlowNorm01: 0.92,
        },
        {
          tier: "surveyed",
          cellId: neighborCellId,
          observedTick: 72,
          isLiveHqSight: true,
          elevationM: 120,
          confidence01: 0.99,
          provenanceLabel: "HQ Weather Log",
          biomeKey: "temperate_forest",
          isOcean: false,
          riverDownstreamCellId: null,
          riverFlowNorm01: 0.4,
        },
        {
          tier: "unexplored",
          cellId: unobservedCellId,
        },
      ],
      observedWeather: [
        {
          cellId: floodedCellId,
          observedTick: 72,
          precipMmHr: 18.5,
          mudIndex01: 0.88,
          tempC: 4.5,
          visibility01: 0.35,
          floodStage01: 0.72,
          snowDepthM: 0.12,
          iceThicknessM: 0.0,
          windUMps: 8.4,
          windVMps: -5.1,
        },
      ],
    };

    shader.uploadSnapshot(stormSnap, null, null);
    const texData = (shader.material.uniforms.uCellDataTex.value as { image: { data: Uint8Array } }).image.data;
    expect(texData[floodedCellId * 4 + 1]).toBeGreaterThan(180);
    expect(texData[neighborCellId * 4 + 1]).toBe(0);
    expect(texData[unobservedCellId * 4 + 1]).toBe(0);

    features.rebuild(stormSnap, topo, "weather", null);
    const summary = features.getSummary();
    expect(summary.windVectorCount).toBe(1);
    expect(summary.stormCellCount).toBe(1);
    expect(summary.floodedRiverCount).toBe(1);

    features.dispose();
    shader.dispose();
  });
});
