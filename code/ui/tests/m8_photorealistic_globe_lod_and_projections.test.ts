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
  PluginRegistry,
  UiIntent,
  UiIntentBus,
  generateGoldbergTopology,
  greatCircleRadians,
  isValidDiegeticSnapshot,
  norm3,
  projectUnitToFlat2D,
  slerpUnitVec3,
  unprojectFlat2DToUnit,
} from "../src/spine";
import {
  GlobeFeatureOverlayGroup,
  GlobeOrbitalCamera,
  GlobePatchLodManager,
  SettlementDetailClusterGroup,
  classifySemanticZoomTier,
  computeDeclusteredEntityOffsets,
  createGlobeShaderController,
  initializeGlobeViewport,
} from "../src/globe";
import {
  buildHydrologyRibbonMesh,
  createShaderTerrainSurfaceSampler,
} from "../src/globe/hydrology-ribbons";
import { createDispatchInboxStubPlugin } from "../src/dispatches";
import {
  createControlsReferenceWindowPlugin,
  createNotificationMatrixWindowPlugin,
  createWarfareCommandWindowPlugin,
  mountCommandHud,
} from "../src/hud";
import {
  createEntityInspectorWindowPlugin,
  createLedgerStubPlugin,
} from "../src/views";
import { DesktopWindowManager } from "../src/windows";

const M2_FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m2_snapshot.json");
const M2_SNAPSHOTS = JSON.parse(fs.readFileSync(M2_FIXTURE_PATH, "utf8")) as DiegeticSnapshot[];

const M3_M4_FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const CANONICAL_M3_M4 = JSON.parse(fs.readFileSync(M3_M4_FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

describe("Photorealistic Planet Presentation, Multi-Tier Semantic Zoom LOD, De-Clustered Ribbons & Projections", () => {
  // @req UI-GLB-006
  it("subdivides 5,762-cell planet patches across LOD 0..4 around focus and keeps unexplored biophysical slots zeroed (UI-GLB-006)", () => {
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
    expect(orbitalCounts[4]).toBe(0);

    patchLod.updateForCamera(focus, 0.05, 0);
    const closeCounts = patchLod.getLodCounts();
    const closeTris = patchLod.getTotalVisibleTriangles();
    expect(closeCounts[4]).toBeGreaterThan(0);
    expect(closeCounts[3]).toBeGreaterThan(0);
    expect(closeTris).toBeGreaterThan(orbitalTris * 4);

    // 2D flat mode shows all 20 sectors simultaneously.
    patchLod.updateForCamera(focus, 0.85, 1);
    const flatCounts = patchLod.getLodCounts();
    expect(flatCounts[0] + flatCounts[1] + flatCounts[2] + flatCounts[3] + flatCounts[4]).toBe(20);

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

    // Verify coastal rivers draining into an ocean cell terminate at the land/sea shoreline with zero ocean vertices.
    const coastSrcId = hqCellId;
    const coastOceanId = hqNbr;
    const coastKnownCells: KnownCellDto[] = [];
    const seenCoastIds = new Set<number>();
    const pushCoastCell = (id: number, elevM: number, isOcean: boolean, isSrc: boolean) => {
      if (seenCoastIds.has(id)) return;
      seenCoastIds.add(id);
      coastKnownCells.push({
        tier: "surveyed",
        cellId: id,
        observedTick: 12,
        isLiveHqSight: isSrc,
        elevationM: elevM,
        confidence01: 1.0,
        provenanceLabel: "Coastal Survey",
        biomeKey: isOcean ? "ocean" : "temperate_forest",
        isOcean,
        riverDownstreamCellId: isSrc ? coastOceanId : null,
        riverFlowNorm01: isSrc ? 0.85 : 0.0,
        strahlerOrder: isSrc ? 3 : 1,
      });
    };
    pushCoastCell(coastSrcId, 55, false, true);
    for (const nbr of topo.cells[coastSrcId].neighbors) {
      if (nbr !== coastOceanId) pushCoastCell(nbr, 95, false, false);
    }
    pushCoastCell(coastOceanId, -220, true, false);
    for (const nbr of topo.cells[coastOceanId].neighbors) {
      pushCoastCell(nbr, -260, true, false);
    }
    const coastSnap: DiegeticSnapshot = {
      ...fogSnap,
      knownCells: coastKnownCells,
    };
    const coastRibbon = buildHydrologyRibbonMesh(
      coastSnap,
      topo,
      cellMap,
      weatherByCell,
      0.2,
      mat,
      projectPoint,
      false,
    );
    expect(coastRibbon.renderedRiverCount).toBe(1);
    expect(coastRibbon.mesh).not.toBeNull();
    const sampler = createShaderTerrainSurfaceSampler(coastSnap, topo);
    const cPosAttr = coastRibbon.mesh!.geometry.getAttribute("position") as THREE.BufferAttribute;
    const oceanCenter = new THREE.Vector3(...topo.cells[coastOceanId].centerUnit).normalize();
    let minEffElev = Infinity;
    let minAngToOcean = Math.PI;
    for (let i = 0; i < cPosAttr.count; i += 1) {
      const unit: readonly [number, number, number] = [
        cPosAttr.getX(i),
        cPosAttr.getY(i),
        cPosAttr.getZ(i),
      ];
      const len = Math.hypot(unit[0], unit[1], unit[2]);
      const normU: readonly [number, number, number] = [unit[0] / len, unit[1] / len, unit[2] / len];
      const sample = sampler.sample(normU, coastSrcId, coastOceanId, 0.2);
      expect(sample.isExplored).toBe(true);
      expect(sample.effectiveElevM).toBeGreaterThanOrEqual(0);
      if (sample.effectiveElevM < minEffElev) minEffElev = sample.effectiveElevM;
      const v = new THREE.Vector3(normU[0], normU[1], normU[2]);
      const ang = Math.acos(Math.max(-1, Math.min(1, v.dot(oceanCenter))));
      if (ang < minAngToOcean) minAngToOcean = ang;
    }
    expect(minEffElev).toBeLessThan(20);
    expect(minAngToOcean).toBeGreaterThan(edgeSpan * 0.25);
    coastRibbon.mesh!.geometry.dispose();
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

  // @req UI-GLB-002
  // @req UI-GLB-006
  it("performs smooth inertial cursor-zoom, transitions pitch from 0 to 45 deg, elevates all explored land, and spawns instanced 3D forests (UI-GLB-002, UI-GLB-006)", async () => {
    const topo = generateGoldbergTopology(3);
    const cellMap = new Map<number, GoldbergCellGeometry>();
    for (const c of topo.cells) cellMap.set(c.cellId, c);

    // 1. Inertial cursor-directed scroll zoom & 0..45 deg auto-pitch + manual offset
    const cam = new GlobeOrbitalCamera(16 / 9, 0.72);
    const cursorTarget = topo.cells[18].centerUnit;
    const initialDist = greatCircleRadians(cam.getFocusUnit(), cursorTarget);
    cam.zoomSmoothByWheelDelta(-0.24, cursorTarget);
    expect(cam.getAltitudeNorm()).toBeCloseTo(0.72, 6);

    let prevAlt = cam.getAltitudeNorm();
    for (let f = 0; f < 40; f += 1) {
      cam.stepAnimation(16);
      const curAlt = cam.getAltitudeNorm();
      expect(curAlt).toBeLessThanOrEqual(prevAlt + 1e-6);
      expect(curAlt).toBeGreaterThanOrEqual(0.48 - 1e-4);
      prevAlt = curAlt;
    }
    expect(cam.getAltitudeNorm()).toBeCloseTo(0.48, 2);
    expect(greatCircleRadians(cam.getFocusUnit(), cursorTarget)).toBeLessThan(initialDist);

    cam.setAltitudeNorm(0.60);
    expect(cam.getTelemetry().pitchRad).toBe(0);
    cam.setAltitudeNorm(0.01);
    expect(cam.getTelemetry().pitchRad).toBeCloseTo(Math.PI / 4, 3);
    cam.tiltByPixels(-50);
    expect(cam.getTelemetry().pitchRad).toBeGreaterThan(Math.PI / 4 + 0.05);
    cam.resetNorth(true);
    expect(cam.getTelemetry().pitchRad).toBeCloseTo(Math.PI / 4, 3);

    // 2. Full-spectrum 3D terrain elevation across coastal lowlands, midlands & alpine peaks (0 on ocean/unexplored)
    const testCenterId = 10;
    const oceanId = 200;
    const coastId = 20;
    const hillId = 35;
    const peakId = 50;
    const unexplId = 300;
    const elevCells: KnownCellDto[] = [];
    const registerCluster = (cid: number, elevM: number, isOcean: boolean, biome: string) => {
      const ids = [cid, ...topo.cells[cid].neighbors];
      for (const id of ids) {
        if (elevCells.some((c) => c.cellId === id)) continue;
        elevCells.push({
          tier: "surveyed",
          cellId: id,
          observedTick: 10,
          isLiveHqSight: true,
          elevationM: elevM,
          confidence01: 1,
          provenanceLabel: "Survey",
          biomeKey: biome,
          isOcean,
          meanAnnualTempC: isOcean ? 12 : 14 - Math.max(0, elevM) * 0.005,
          aridityIndex01: 1.0,
          soilFertility01: 0.7,
        });
      }
    };
    registerCluster(testCenterId, 160, false, "temperate_forest");
    registerCluster(oceanId, -300, true, "ocean");
    registerCluster(coastId, 40, false, "temperate_forest");
    registerCluster(hillId, 220, false, "temperate_forest");
    registerCluster(peakId, 2400, false, "alpine_rock");
    elevCells.push({ tier: "unexplored", cellId: unexplId });

    const reliefSnap: DiegeticSnapshot = {
      ...CANONICAL_M3_M4,
      hqPresence: { ...CANONICAL_M3_M4.hqPresence, cellId: testCenterId },
      knownSettlements: [
        {
          settlementId: 0,
          settlementName: "Haven",
          cellId: testCenterId,
          isHq: true,
        },
      ],
      knownCells: elevCells,
      observedWeather: [],
    };
    const sampler = createShaderTerrainSurfaceSampler(reliefSnap, topo);
    const sUnexpl = sampler.sample(topo.cells[unexplId].centerUnit, unexplId, unexplId, 0.08);
    const sOcean = sampler.sample(topo.cells[oceanId].centerUnit, oceanId, oceanId, 0.08);
    const sCoast = sampler.sample(topo.cells[coastId].centerUnit, coastId, coastId, 0.08);
    const sHill = sampler.sample(topo.cells[hillId].centerUnit, hillId, hillId, 0.08);
    const sPeak = sampler.sample(topo.cells[peakId].centerUnit, peakId, peakId, 0.08);

    expect(sUnexpl.isExplored).toBe(false);
    expect(sUnexpl.vertexReliefHeight).toBe(0);
    expect(sOcean.vertexReliefHeight).toBe(0);
    expect(sCoast.vertexReliefHeight).toBeGreaterThan(0.002);
    expect(sHill.vertexReliefHeight).toBeGreaterThan(sCoast.vertexReliefHeight);
    expect(sPeak.vertexReliefHeight).toBeGreaterThan(sHill.vertexReliefHeight);

    // 3. Instanced 3D woodland crowns & grounded structures on explored dry land
    const clusterGroup = new SettlementDetailClusterGroup();
    const projectPoint = (unit: readonly [number, number, number], lift: number) =>
      new THREE.Vector3(unit[0], unit[1], unit[2]).normalize().multiplyScalar(1 + lift);
    const declustered = computeDeclusteredEntityOffsets(reliefSnap, cellMap, 0.08);
    const built = clusterGroup.rebuild(
      reliefSnap,
      topo,
      cellMap,
      declustered.formations,
      0.08,
      projectPoint,
      false,
      sampler,
      topo.cells[testCenterId].centerUnit,
    );
    expect(built).toBe(1);
    const instancedForests = clusterGroup.group.children.filter(
      (c): c is THREE.InstancedMesh => c instanceof THREE.InstancedMesh,
    );
    expect(instancedForests.length).toBeGreaterThan(0);
    const totalTreeInstances = instancedForests.reduce((sum, im) => sum + im.count, 0);
    expect(totalTreeInstances).toBeGreaterThan(0);

    const tmpMat = new THREE.Matrix4();
    const tmpPos = new THREE.Vector3();
    for (const im of instancedForests) {
      for (let i = 0; i < im.count; i += 1) {
        im.getMatrixAt(i, tmpMat);
        tmpPos.setFromMatrixPosition(tmpMat).normalize();
        const ts = sampler.sample([tmpPos.x, tmpPos.y, tmpPos.z], testCenterId, testCenterId, 0.08);
        expect(ts.isExplored).toBe(true);
        expect(ts.effectiveElevM).toBeGreaterThanOrEqual(0);
      }
    }
    clusterGroup.dispose();

    // 4. 3D ray-marched oblique terrain picking accuracy at ~45 deg tilt
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
    dom.window.document.getElementById("app-shell")!.appendChild(canvas);

    const store = new DiegeticStore();
    store.ingestSnapshot(M2_SNAPSHOTS[0]);
    const bus = new UiIntentBus();
    const viewport = await initializeGlobeViewport(canvas, store, bus, []);
    try {
      const targetCellId = topo.cells[0].cellId;
      viewport.camera.flyToUnitVector(topo.cells[targetCellId].centerUnit, 0.06, 0, true);
      viewport.stepRender(16);
      expect(viewport.camera.getTelemetry().pitchRad).toBeGreaterThan(0.65);
      const hit = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(hit?.cellId).toBe(targetCellId);
    } finally {
      viewport.dispose();
      dom.window.close();
      (globalThis as unknown as { document: unknown }).document = prevDoc;
      (globalThis as unknown as { window: unknown }).window = prevWin;
      (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = prevHtml;
    }
  });

  // @req UI-GLB-008
  it("supports heading-relative WASDQE camera controls, pointer yaw rotation, non-colliding window shortcuts, and Controls reference (UI-GLB-008)", () => {
    const cam = new GlobeOrbitalCamera(16 / 9, 0.25);
    cam.flyToUnitVector([0, 0, 1], 0.25, 0, true);
    const startHeading = cam.getTelemetry().headingRad;
    expect(Math.abs(startHeading)).toBeLessThan(0.05);

    cam.rotateYawByRadians(0.45);
    expect(cam.getTelemetry().headingRad).toBeLessThan(-0.35);
    cam.rotateYawByRadians(-0.90);
    expect(cam.getTelemetry().headingRad).toBeGreaterThan(0.35);
    cam.resetNorth(true);
    expect(Math.abs(cam.getTelemetry().headingRad)).toBeLessThan(0.05);

    // Heading-relative WASDQE motion
    cam.stepKeyboardNavigation(1, 0, 0, 100);
    expect(cam.getFocusUnit()[1]).toBeGreaterThan(0.02);
    cam.stepKeyboardNavigation(-1, 0, 0, 100);
    cam.stepKeyboardNavigation(0, 1, 0, 100);
    expect(cam.getFocusUnit()[0]).toBeGreaterThan(0.02);
    cam.flyToUnitVector([0, 0, 1], 0.25, 0, true);
    cam.stepKeyboardNavigation(0, 0, 1, 100);
    expect(cam.getTelemetry().headingRad).toBeLessThan(-0.10);

    const dom = new JSDOM(
      `<!DOCTYPE html><html><body><div id="app-shell"></div><div id="desktop-win-host"></div></body></html>`,
      { url: "http://localhost/" },
    );
    const prevDoc = (globalThis as unknown as { document: unknown }).document;
    const prevWin = (globalThis as unknown as { window: unknown }).window;
    const prevHtml = (globalThis as unknown as { HTMLElement: unknown }).HTMLElement;
    (globalThis as unknown as { document: unknown }).document = dom.window.document;
    (globalThis as unknown as { window: unknown }).window = dom.window;
    (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = dom.window.HTMLElement;

    try {
      const store = new DiegeticStore();
      store.ingestSnapshot(CANONICAL_M3_M4);
      const bus = new UiIntentBus();
      const registry = new PluginRegistry();
      registry.registerWindowPlugin(createLedgerStubPlugin());
      registry.registerWindowPlugin(createDispatchInboxStubPlugin());
      registry.registerWindowPlugin(createWarfareCommandWindowPlugin());
      registry.registerWindowPlugin(createNotificationMatrixWindowPlugin());
      registry.registerWindowPlugin(createEntityInspectorWindowPlugin());
      registry.registerWindowPlugin(createControlsReferenceWindowPlugin());

      const winHost = dom.window.document.getElementById("desktop-win-host")!;
      const appShell = dom.window.document.getElementById("app-shell")!;
      const hudDispose = mountCommandHud(appShell, store, bus);
      const manager = new DesktopWindowManager(winHost, registry, { store, bus });

      // Pressing W, A, S, D, Q, E must never trigger or toggle floating windows.
      for (const key of ["w", "a", "s", "d", "q", "e"]) {
        dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key, bubbles: true }));
      }
      expect(manager.listFrames()).toHaveLength(0);

      // Clicking HUD Controls (?) button opens the Controls & Keybindings window.
      const controlsBtn = appShell.querySelector<HTMLButtonElement>('[data-open-plugin="controls_reference"]');
      expect(controlsBtn).not.toBeNull();
      controlsBtn!.click();
      expect(manager.listFrames().some((f) => f.tabs[0].pluginId === "controls_reference")).toBe(true);
      expect(winHost.textContent).toContain("W / S");
      expect(winHost.textContent).toContain("Q / E");

      // Pressing Escape closes the unpinned Controls window cleanly; F1 re-opens it.
      dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      expect(manager.listFrames()).toHaveLength(0);
      dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "F1", bubbles: true }));
      expect(manager.listFrames().some((f) => f.tabs[0].pluginId === "controls_reference")).toBe(true);

      manager.dispose();
      hudDispose();
    } finally {
      dom.window.close();
      (globalThis as unknown as { document: unknown }).document = prevDoc;
      (globalThis as unknown as { window: unknown }).window = prevWin;
      (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = prevHtml;
    }
  });

  // @req UI-MAP-009
  it("resolves settlements, facilities, stone-grey purpose roads, contiguous flow rivers, and clean selection clearing (UI-MAP-009)", async () => {
    const topo = generateGoldbergTopology(3);
    const cellMap = new Map<number, GoldbergCellGeometry>();
    for (const c of topo.cells) cellMap.set(c.cellId, c);

    const hqId = 0;
    const roadOutpostId = topo.cells[hqId].neighbors[0];
    const riverMidId = topo.cells[hqId].neighbors[1];
    const seaId =
      topo.cells[riverMidId].neighbors.find((n) => n !== hqId && n !== roadOutpostId) ??
      topo.cells[riverMidId].neighbors[0];
    const farId = topo.cells.find((c) => c.cellId !== hqId && c.cellId !== roadOutpostId && c.cellId !== riverMidId && c.cellId !== seaId)!.cellId;

    const knownCells: KnownCellDto[] = [
      {
        tier: "surveyed",
        cellId: hqId,
        observedTick: 12,
        isLiveHqSight: true,
        elevationM: 120,
        confidence01: 1.0,
        provenanceLabel: "Direct Observation",
        biomeKey: "temperate_forest",
        isOcean: false,
        riverDownstreamCellId: riverMidId,
        riverChannelDownstreamCellId: riverMidId,
        riverDischargeM3S: 140,
        riverVelocityMps: 1.4,
        hydroPotentialKw: 8400,
        strahlerOrder: 2,
      },
      {
        tier: "surveyed",
        cellId: roadOutpostId,
        observedTick: 12,
        isLiveHqSight: true,
        elevationM: 280,
        confidence01: 1.0,
        provenanceLabel: "Direct Observation",
        biomeKey: "temperate_grassland",
        isOcean: false,
        riverDownstreamCellId: hqId,
        riverChannelDownstreamCellId: null,
        riverDischargeM3S: 12,
        strahlerOrder: 1,
      },
      {
        tier: "stale",
        cellId: riverMidId,
        observedTick: 4,
        lastLiveTick: 4,
        elevationM: 55,
        confidence01: 0.8,
        provenanceLabel: "Stale Survey",
        biomeKey: "temperate_forest",
        isOcean: false,
        riverDownstreamCellId: seaId,
        riverChannelDownstreamCellId: seaId,
        riverDischargeM3S: 260,
        riverVelocityMps: 1.8,
        hydroPotentialKw: 12400,
        strahlerOrder: 3,
      },
      {
        tier: "surveyed",
        cellId: seaId,
        observedTick: 12,
        isLiveHqSight: false,
        elevationM: -180,
        confidence01: 1.0,
        provenanceLabel: "Coastal Sounding",
        biomeKey: "shallow_sea",
        isOcean: true,
        strahlerOrder: 0,
      },
      {
        tier: "unexplored",
        cellId: farId,
      },
    ];
    for (const nbr of [...topo.cells[hqId].neighbors, ...topo.cells[riverMidId].neighbors]) {
      if (!knownCells.some((k) => k.cellId === nbr)) {
        knownCells.push({
          tier: "surveyed",
          cellId: nbr,
          observedTick: 12,
          isLiveHqSight: true,
          elevationM: 110,
          confidence01: 1.0,
          provenanceLabel: "Survey",
          biomeKey: "temperate_grassland",
          isOcean: false,
          strahlerOrder: 1,
        });
      }
    }

    const base = CANONICAL_M3_M4;
    const testSnap: DiegeticSnapshot = {
      ...base,
      globeTopology: { subdivision: 3, cellCount: topo.cellCount },
      hqPresence: { ...base.hqPresence, cellId: hqId, settlementName: "Haven Landing" },
      knownSettlements: [{ settlementId: 0, settlementName: "Haven Landing", cellId: hqId, isHq: true }],
      knownCells,
      surveyedRoads: [
        {
          edgeId: 77,
          fromCellId: hqId,
          toCellId: roadOutpostId,
          infraKey: "foot_trail",
          roadPurpose: "resource_or_port_access",
          purposeLabel: "Haven -> Quarry Outpost",
          cumulativePassages: 5,
          cumulativeTonnesPassed: 8.2,
        },
      ],
      facilityRows: [
        {
          facilityId: 101,
          settlementId: 0,
          settlementName: "Haven Landing",
          processKey: "copper_smelter",
          processName: "Copper Smelter Hearth",
          assignedWorkers: 12,
          toolCondition01: 0.9,
          weatherHalted: false,
          shaftWater01: 0,
          observedTick: 12,
          provenanceLabel: "Direct Observation",
        },
      ],
      warfare: base.warfare
        ? {
            ...base.warfare,
            formations: base.warfare.formations.slice(0, 1).map((f) => ({
              ...f,
              currentCellId: hqId,
              currentUnitXyz: topo.cells[hqId].centerUnit,
              targetCellId: hqId,
              targetUnitXyz: topo.cells[hqId].centerUnit,
            })),
          }
        : undefined,
    };

    // Contiguous river ribbon renders single trunk across surveyed -> stale -> sea with chevron mesh.
    const mat = new THREE.MeshStandardMaterial();
    const chevMat = new THREE.MeshStandardMaterial();
    const projectPoint = (unit: readonly [number, number, number], lift: number) =>
      new THREE.Vector3(unit[0], unit[1], unit[2]).normalize().multiplyScalar(1 + lift);
    const ribbon = buildHydrologyRibbonMesh(
      testSnap,
      topo,
      cellMap,
      new Map<number, ObservedWeatherDto>(),
      0.2,
      mat,
      projectPoint,
      false,
      undefined,
      chevMat,
    );
    expect(ribbon.renderedRiverCount).toBe(1);
    expect(ribbon.chevronMesh).not.toBeNull();
    ribbon.mesh?.geometry.dispose();
    ribbon.chevronMesh?.geometry.dispose();

    const overlay = new GlobeFeatureOverlayGroup({
      oceanShallowRgb: [135, 163, 173],
      oceanDeepRgb: [61, 89, 102],
      landLowRgb: [115, 138, 87],
      landMidRgb: [176, 162, 116],
      landHighRgb: [140, 114, 88],
      peakSnowRgb: [244, 239, 228],
      riverWaterRgb: [54, 108, 133],
      roadOchreRgb: [166, 89, 40],
      roadStoneRgb: [122, 125, 130],
      settlementMarkerRgb: [139, 46, 34],
      contourInkRgb: [59, 44, 29],
      compassRoseRgb: [110, 85, 57],
      unexploredVellumRgb: [230, 216, 185],
      parchmentGrainTextureUrl: "",
      hatchPatternTextureUrl: "",
      terrainBiomeTextureUrl: "",
    });
    expect(overlay).toBeDefined();
    overlay.dispose();

    const dom = new JSDOM(
      `<!DOCTYPE html><html><body><div id="app-shell"></div><div id="win-host"></div></body></html>`,
      { url: "http://localhost/" },
    );
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
    dom.window.document.getElementById("app-shell")!.appendChild(canvas);

    const store = new DiegeticStore();
    store.ingestSnapshot(testSnap);
    const bus = new UiIntentBus();
    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));

    const viewport = await initializeGlobeViewport(canvas, store, bus, []);
    const shaderCtrl = createGlobeShaderController();
    try {
      // Fragment shader must never fill discrete aCellId polygons when selectedUnitXyz is null (star-polygon regression).
      expect(shaderCtrl.material.fragmentShader).not.toContain("step(abs(cid - uSelectedCellId)");
      expect(shaderCtrl.material.fragmentShader).not.toContain("step(abs(cid - uHoveredCellId)");

      // Aim directly at settlement center even though formation is co-located on cell 0 -> must resolve settlement/building, never hijacked by formation!
      viewport.camera.flyToUnitVector(topo.cells[hqId].centerUnit, 0.12, 0, true);
      viewport.stepRender(16);
      const centerHit = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(centerHit?.hitKind).not.toBe("formation");
      expect(["settlement", "facility"]).toContain(centerHit?.hitKind);

      // Sloping cell continuous elevation sampling: picking from high cell (280m) toward lower coastal/valley neighbor differs from static cell node elevation.
      const slopeOutpostSnap: DiegeticSnapshot = {
        ...testSnap,
        knownCells: testSnap.knownCells.map((c) =>
          c.cellId === roadOutpostId && c.tier === "surveyed"
            ? { ...c, elevationM: 566, rockStrataCode: 1 }
            : c.cellId === hqId && c.tier === "surveyed"
              ? { ...c, elevationM: 20 }
              : c,
        ),
        knownSettlements: [],
        surveyedRoads: [],
        trackedConvoys: [],
        warfare: undefined,
      };
      store.ingestSnapshot(slopeOutpostSnap);
      viewport.updateFromSnapshot(slopeOutpostSnap);

      const farNbr = norm3(slerpUnitVec3(topo.cells[roadOutpostId].centerUnit, topo.cells[hqId].centerUnit, 0.42));
      viewport.camera.flyToUnitVector(topo.cells[roadOutpostId].centerUnit, 0.12, 0, true);
      viewport.stepRender(16);
      const crestHit = viewport.layerHost.pickGlobeAtNdc(0, 0);

      viewport.camera.flyToUnitVector(farNbr, 0.12, 0, true);
      viewport.stepRender(16);
      const slopeHit = viewport.layerHost.pickGlobeAtNdc(0, 0);

      expect(crestHit?.hitKind).toBe("cell");
      expect(slopeHit?.hitKind).toBe("cell");
      if (crestHit?.hitKind === "cell" && slopeHit?.hitKind === "cell") {
        expect(crestHit.localElevationM).toBeGreaterThan(slopeHit.localElevationM! + 50);
      }

      store.ingestSnapshot(testSnap);
      viewport.updateFromSnapshot(testSnap);

      // Aim directly at road midpoint -> resolves "road" with purpose label!
      const roadMid = norm3(slerpUnitVec3(topo.cells[hqId].centerUnit, topo.cells[roadOutpostId].centerUnit, 0.5));
      viewport.camera.flyToUnitVector(roadMid, 0.12, 0, true);
      viewport.stepRender(16);
      const roadHit = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(roadHit?.hitKind).toBe("road");
      if (roadHit?.hitKind === "road") {
        expect(roadHit.edgeId).toBe(77);
        expect(roadHit.purposeLabel).toBe("Haven -> Quarry Outpost");
      }

      // Aim directly at river reach midpoint -> resolves "river" with volumetric m3/s metadata!
      const riverMid = norm3(slerpUnitVec3(topo.cells[hqId].centerUnit, topo.cells[riverMidId].centerUnit, 0.5));
      viewport.camera.flyToUnitVector(riverMid, 0.12, 0, true);
      viewport.stepRender(16);
      const riverHit = viewport.layerHost.pickGlobeAtNdc(0, 0);
      expect(riverHit?.hitKind).toBe("river");
      if (riverHit?.hitKind === "river") {
        expect(riverHit.dischargeM3S).toBe(140);
        expect(riverHit.velocityMps).toBeCloseTo(1.4, 2);
      }

      // Pressing Escape clears active map selection cleanly.
      bus.emit({ kind: "SelectMapFeature", hit: roadHit! });
      dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      const lastSel = [...emitted].reverse().find((e) => e.kind === "SelectMapFeature");
      expect(lastSel?.kind === "SelectMapFeature" ? lastSel.hit : undefined).toBeNull();

      // Dedicated Entity Inspector views render road, river, cell slope, and rich resource-outpost facility dossiers.
      const reg = new PluginRegistry();
      reg.registerWindowPlugin(createEntityInspectorWindowPlugin());
      const winHost = dom.window.document.getElementById("win-host")!;
      const wm = new DesktopWindowManager(winHost, reg, { store, bus });
      bus.emit({ kind: "FocusEntityInspector", entityKind: "road", entityId: 77 });
      expect(winHost.textContent).toContain("Haven -> Quarry Outpost");
      bus.emit({ kind: "FocusEntityInspector", entityKind: "river", entityId: hqId });
      expect(winHost.textContent).toContain("140.0 m3/s");
      bus.emit({ kind: "FocusEntityInspector", entityKind: "cell", entityId: roadOutpostId, localElevationM: 215 });
      expect(winHost.textContent).toContain("Local ground: 215m");
      expect(winHost.textContent).toContain("Cell center node: 280m");
      bus.emit({ kind: "FocusEntityInspector", entityKind: "facility", entityId: 900077 });
      expect(winHost.textContent).toContain("Quarry Outpost");
      expect(winHost.textContent).toContain("Open-Cut Stone & Mineral Quarry");
      expect(winHost.textContent).toContain("Stationed Personnel");
      expect(winHost.textContent).toContain("Haven -> Quarry Outpost");
      wm.dispose();
    } finally {
      shaderCtrl.dispose();
      viewport.dispose();
      dom.window.close();
      (globalThis as unknown as { document: unknown }).document = prevDoc;
      (globalThis as unknown as { window: unknown }).window = prevWin;
      (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = prevHtml;
    }
  });
});
