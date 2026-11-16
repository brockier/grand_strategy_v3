import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import * as THREE from "three";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  GoldbergCellGeometry,
  KnownCellDto,
  ObservedWeatherDto,
  UiIntentBus,
  generateGoldbergTopology,
  greatCircleRadians,
  isValidDiegeticSnapshot,
  projectUnitToFlat2D,
  unprojectFlat2DToUnit,
} from "../src/spine";
import {
  GlobePatchLodManager,
  SettlementDetailClusterGroup,
  classifySemanticZoomTier,
  computeDeclusteredEntityOffsets,
  createGlobeShaderController,
  initializeGlobeViewport,
} from "../src/globe";
import { buildHydrologyRibbonMesh } from "../src/globe/hydrology-ribbons";

const M2_FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m2_snapshot.json");
const M2_SNAPSHOTS = JSON.parse(fs.readFileSync(M2_FIXTURE_PATH, "utf8")) as DiegeticSnapshot[];

const M3_M4_FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const CANONICAL_M3_M4 = JSON.parse(fs.readFileSync(M3_M4_FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

describe("Photorealistic Planet Presentation, Multi-Tier Semantic Zoom LOD, De-Clustered Ribbons & Projections", () => {
  // @req UI-GLB-006
  it("subdivides 5,762-cell planet patches across LOD 0..3 around focus and keeps unexplored biophysical slots zeroed (UI-GLB-006)", () => {
    const topo = generateGoldbergTopology(24);
    expect(topo.cellCount).toBe(5762);
    expect(topo.pentagonCount).toBe(12);
    expect(topo.hexagonCount).toBe(5750);
    expect(topo.triangles.length).toBe(11520);

    const shader = createGlobeShaderController();
    const patchLod = new GlobePatchLodManager(topo, shader.material);
    const focus = topo.cells[0].centerUnit;

    patchLod.updateForCamera(focus, 0.92, 0);
    const orbitalCounts = patchLod.getLodCounts();
    const orbitalTris = patchLod.getTotalVisibleTriangles();
    expect(orbitalCounts[0]).toBeGreaterThan(0);
    expect(orbitalCounts[3]).toBe(0);

    patchLod.updateForCamera(focus, 0.05, 0);
    const closeCounts = patchLod.getLodCounts();
    const closeTris = patchLod.getTotalVisibleTriangles();
    expect(closeCounts[3]).toBeGreaterThan(0);
    expect(closeTris).toBeGreaterThan(orbitalTris * 4);

    // 2D flat mode shows all 20 sectors simultaneously.
    patchLod.updateForCamera(focus, 0.85, 1);
    const flatCounts = patchLod.getLodCounts();
    expect(flatCounts[0] + flatCounts[1] + flatCounts[2] + flatCounts[3]).toBe(20);

    // Biophysical DataTexture remains strictly [0, 0, 0, 0] on Unexplored & Rumored cells while packing Surveyed data.
    const bioCells: KnownCellDto[] = [
      { tier: "unexplored", cellId: 40 },
      {
        tier: "rumored",
        cellId: 41,
        observedTick: 4,
        elevationM: 220,
        confidence01: 0.3,
        provenanceLabel: "Coastal Rumor",
        biomeKey: "steppe",
        isOcean: false,
      },
      {
        tier: "surveyed",
        cellId: 42,
        observedTick: 18,
        isLiveHqSight: true,
        elevationM: 340,
        confidence01: 0.99,
        provenanceLabel: "Direct Observation (HQ)",
        biomeKey: "temperate_forest",
        isOcean: false,
        meanAnnualTempC: 16.5,
        annualPrecipMm: 1120,
        aridityIndex01: 1.05,
        soilFertility01: 0.85,
        rockStrataCode: 4,
        drainageAreaKm2: 64000,
        strahlerOrder: 3,
      },
    ];
    const validSnap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      knownCells: bioCells,
      observedWeather: [],
    };
    expect(isValidDiegeticSnapshot(validSnap)).toBe(true);

    // Runtime contract validator must reject any leaked biophysical attribute on an unexplored cell.
    const leakedSnap = {
      ...validSnap,
      knownCells: [
        { tier: "unexplored", cellId: 40, soilFertility01: 0.9 },
        bioCells[2],
      ],
    };
    expect(isValidDiegeticSnapshot(leakedSnap)).toBe(false);

    shader.uploadSnapshot(validSnap, null, null);
    const bioTex = (
      shader.material.uniforms.uBiophysicalTex.value as { image: { data: Uint8Array } }
    ).image.data;
    expect([bioTex[40 * 4], bioTex[40 * 4 + 1], bioTex[40 * 4 + 2], bioTex[40 * 4 + 3]]).toEqual([
      0, 0, 0, 0,
    ]);
    expect([bioTex[41 * 4], bioTex[41 * 4 + 1], bioTex[41 * 4 + 2], bioTex[41 * 4 + 3]]).toEqual([
      0, 0, 0, 0,
    ]);
    expect(bioTex[42 * 4]).toBeGreaterThan(0);
    expect(bioTex[42 * 4 + 3] >> 5).toBe(3);

    patchLod.dispose();
    shader.dispose();
  });

  // @req UI-MAP-008
  it("clips meandering hydrology ribbons at the fog horizon, radially de-clusters formations, and spawns 3D settlements at close zoom (UI-MAP-008)", () => {
    const topo = generateGoldbergTopology(3);
    const cellMap = new Map<number, GoldbergCellGeometry>();
    for (const c of topo.cells) cellMap.set(c.cellId, c);

    const hqCellId = 82;
    const hqNbr = topo.cells[hqCellId].neighbors[0];
    const fogSnap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      hqPresence: {
        ...CANONICAL_M3_M4.hqPresence,
        cellId: hqCellId,
      },
      knownSettlements: [
        {
          settlementId: 0,
          settlementName: "Haven Landing Perimeter",
          cellId: hqCellId,
          isHq: true,
        },
      ],
      knownCells: [
        {
          tier: "surveyed",
          cellId: hqCellId,
          observedTick: 12,
          isLiveHqSight: true,
          elevationM: 140,
          confidence01: 1.0,
          provenanceLabel: "Direct Observation (HQ)",
          biomeKey: "temperate_forest",
          isOcean: false,
          riverDownstreamCellId: hqNbr,
          riverFlowNorm01: 0.85,
          strahlerOrder: 3,
        },
        {
          tier: "unexplored",
          cellId: hqNbr,
        },
      ],
      warfare: {
        theaters: [],
        availableDoctrines: [],
        formations: [
          {
            formationId: 9,
            label: "1st Expeditionary Column",
            templateKey: "infantry_column",
            theaterId: 1,
            commanderId: 1,
            commanderName: "T. Kowalski",
            homeSettlementId: 0,
            currentCellId: hqCellId,
            currentUnitXyz: topo.cells[hqCellId].centerUnit,
            targetCellId: hqCellId,
            targetUnitXyz: topo.cells[hqCellId].centerUnit,
            personnelCount: 240,
            equipmentMassKg: 4800,
            munitionsDays: 14,
            rationsDays: 18,
            cohesion01: 0.92,
            doctrineKey: "deliberate_advance",
            noRetreatOrder: false,
            encircled: false,
            insubordinateWithdrawal: false,
            staleOrderTicks: 0,
            observedTick: 12,
            confidence01: 0.99,
            provenanceLabel: "Direct Observation (HQ)",
          },
        ],
      },
      observedWeather: [
        {
          cellId: hqCellId,
          observedTick: 12,
          precipMmHr: 4.0,
          mudIndex01: 0.45,
          tempC: 16.0,
          visibility01: 0.9,
          floodStage01: 0.6,
        },
      ],
    };

    const weatherByCell = new Map<number, ObservedWeatherDto>([
      [hqCellId, fogSnap.observedWeather[0]],
    ]);
    const mat = new THREE.MeshStandardMaterial();
    const projectPoint = (unit: readonly [number, number, number], lift: number) =>
      new THREE.Vector3(unit[0], unit[1], unit[2]).normalize().multiplyScalar(1 + lift);

    const ribbon = buildHydrologyRibbonMesh(
      fogSnap,
      topo,
      cellMap,
      weatherByCell,
      0.2,
      mat,
      projectPoint,
      false,
    );
    expect(ribbon.renderedRiverCount).toBe(1);
    expect(ribbon.floodedRiverCount).toBe(1);
    expect(ribbon.mesh).not.toBeNull();

    // Verify all river ribbon vertices terminate well before reaching the unexplored downstream center.
    const posAttr = ribbon.mesh!.geometry.getAttribute("position") as THREE.BufferAttribute;
    const dstCenter = new THREE.Vector3(...topo.cells[hqNbr].centerUnit).normalize();
    let minAngToUnexplored = Math.PI;
    for (let i = 0; i < posAttr.count; i += 1) {
      const v = new THREE.Vector3(posAttr.getX(i), posAttr.getY(i), posAttr.getZ(i)).normalize();
      const ang = Math.acos(Math.max(-1, Math.min(1, v.dot(dstCenter))));
      if (ang < minAngToUnexplored) minAngToUnexplored = ang;
    }
    const edgeSpan = greatCircleRadians(
      topo.cells[hqCellId].centerUnit,
      topo.cells[hqNbr].centerUnit,
    );
    expect(minAngToUnexplored).toBeGreaterThan(edgeSpan * 0.45);
    ribbon.mesh!.geometry.dispose();
    mat.dispose();

    // Semantic zoom classification + radial declustering + Tier-3 procedural settlement buildings.
    expect(classifySemanticZoomTier(0.85)).toBe("orbital");
    expect(classifySemanticZoomTier(0.55)).toBe("continental");
    expect(classifySemanticZoomTier(0.25)).toBe("regional");
    expect(classifySemanticZoomTier(0.08)).toBe("settlement");

    const declustered = computeDeclusteredEntityOffsets(fogSnap, cellMap, 0.12);
    const fPos = declustered.formations.get(9)!;
    expect(fPos.offsetApplied).toBe(true);
    expect(greatCircleRadians(fPos.anchorUnit, fPos.displayUnit)).toBeGreaterThan(0.01);

    const clusterGroup = new SettlementDetailClusterGroup();
    const farCount = clusterGroup.rebuild(
      fogSnap,
      topo,
      cellMap,
      declustered.formations,
      0.85,
      projectPoint,
      false,
    );
    expect(farCount).toBe(0);
    expect(clusterGroup.group.children.length).toBe(0);

    const closeCount = clusterGroup.rebuild(
      fogSnap,
      topo,
      cellMap,
      declustered.formations,
      0.08,
      projectPoint,
      false,
    );
    expect(closeCount).toBe(1);
    expect(clusterGroup.group.children.length).toBeGreaterThan(4);
    clusterGroup.dispose();
  });

  // @req UI-GLB-007
  it("projects and unprojects Equal-Area and Mercator coordinates identically and preserves pickGlobeAtNdc parity (UI-GLB-007)", async () => {
    const topo = generateGoldbergTopology(3);
    const focus = topo.cells[0].centerUnit;
    const samplePt = topo.cells[28].centerUnit;

    for (const mode of ["equal_area", "mercator"] as const) {
      const flat = projectUnitToFlat2D(samplePt, mode, focus);
      expect(flat.valid).toBe(true);
      const roundTrip = unprojectFlat2DToUnit(flat.x, flat.y, mode, focus);
      expect(roundTrip).not.toBeNull();
      expect(greatCircleRadians(samplePt, roundTrip!)).toBeLessThan(1e-4);
    }

    const dom = new JSDOM("<!doctype html><html><body><div id='app-shell'></div></body></html>");
    const prevDoc = (globalThis as unknown as { document: unknown }).document;
    const prevWin = (globalThis as unknown as { window: unknown }).window;
    const prevHtml = (globalThis as unknown as { HTMLElement: unknown }).HTMLElement;
    (globalThis as unknown as { document: unknown }).document = dom.window.document;
    (globalThis as unknown as { window: unknown }).window = dom.window;
    (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = dom.window.HTMLElement;

    const canvas = dom.window.document.createElement("canvas");
    Object.defineProperty(canvas, "clientWidth", { value: 1280 });
    Object.defineProperty(canvas, "clientHeight", { value: 720 });
    canvas.getContext = (() => null) as unknown as typeof canvas.getContext;
    const hostDiv = dom.window.document.getElementById("app-shell") as HTMLElement;
    hostDiv.appendChild(canvas);

    const store = new DiegeticStore();
    store.ingestSnapshot(M2_SNAPSHOTS[0]);
    const bus = new UiIntentBus();

    const viewport = await initializeGlobeViewport(canvas, store, bus, []);
    try {
      const targetCellId = topo.cells[0].cellId;
      viewport.camera.flyToUnitVector(topo.cells[targetCellId].centerUnit, 0.35, 0, true);
      viewport.stepRender(16);

      const hitGlobe = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(hitGlobe?.cellId).toBe(targetCellId);

      bus.emit({ kind: "SwitchGlobeProjection", projection: "equal_area" });
      viewport.stepRender(16);
      expect(viewport.status.projectionMode).toBe("equal_area");
      const hitEqualArea = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(hitEqualArea?.cellId).toBe(targetCellId);

      bus.emit({ kind: "SwitchGlobeProjection", projection: "mercator" });
      viewport.stepRender(16);
      expect(viewport.status.projectionMode).toBe("mercator");
      const hitMercator = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(hitMercator?.cellId).toBe(targetCellId);

      bus.emit({ kind: "SwitchGlobeProjection", projection: "globe" });
      viewport.stepRender(16);
      expect(viewport.status.projectionMode).toBe("globe");
    } finally {
      viewport.dispose();
      dom.window.close();
      (globalThis as unknown as { document: unknown }).document = prevDoc;
      (globalThis as unknown as { window: unknown }).window = prevWin;
      (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = prevHtml;
    }
  });
});
