import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import * as THREE from "three";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  GlobeLayerHost,
  KnownCellDto,
  MockSnapshotPlayer,
  ObservedWeatherDto,
  UiIntent,
  UiIntentBus,
  generateGoldbergTopology,
} from "../src/spine";
import { createGlobeShaderController } from "../src/globe/shaders";
import {
  buildHydrologyRibbonMesh,
  createShaderTerrainSurfaceSampler,
} from "../src/globe/hydrology-ribbons";
import { GlobeFeatureOverlayGroup } from "../src/globe/features";
import { GeologyBathymetryOverlayLayer, OverlayToolbarLayer } from "../src/overlays";
import { createEntityInspectorWindowPlugin } from "../src/views";
import { parsePlayerCommand } from "../electron/ipc-validator";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const BASE_FIXTURE = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

function setupJsdom(): JSDOM {
  const dom = new JSDOM("<!doctype html><html><body><div id='root'></div></body></html>", {
    url: "http://localhost/",
  });
  Object.defineProperty(globalThis, "window", { value: dom.window, configurable: true });
  Object.defineProperty(globalThis, "document", { value: dom.window.document, configurable: true });
  Object.defineProperty(globalThis, "HTMLElement", {
    value: dom.window.HTMLElement,
    configurable: true,
  });
  return dom;
}

describe("Sign-Preserving Coastlines, Valley-Carved Monotonic Rivers & Geology/Dam Overlays", () => {
  // @req UI-GLB-009
  it("preserves ocean/land elevation sign at sample nodes, eliminates mid-course gaps, flares estuaries, and descends monotonically", () => {
    const topo = generateGoldbergTopology(3);
    const cellMap = new Map(topo.cells.map((c) => [c.cellId, c]));

    // Build a 4-cell chain passing from upland -> low coastal plain (+6m) -> shallow coastal ocean (-12m).
    const hq = BASE_FIXTURE.hqPresence.cellId;
    const n1 = topo.cells[hq].neighbors[0];
    const n2 = topo.cells[n1].neighbors.find((id) => id !== hq && !topo.cells[hq].neighbors.includes(id)) ??
      topo.cells[n1].neighbors.find((id) => id !== hq)!;
    const oceanId =
      topo.cells[n2].neighbors.find((id) => id !== n1 && id !== hq && !topo.cells[hq].neighbors.includes(id)) ??
      topo.cells[n2].neighbors.find((id) => id !== n1)!;

    const riverCells: KnownCellDto[] = [];
    const pushed = new Set<number>();
    const addCell = (
      cellId: number,
      elevationM: number,
      isOcean: boolean,
      ds: number | null,
      strahler = 2,
      estuary = false,
    ) => {
      if (pushed.has(cellId)) return;
      pushed.add(cellId);
      riverCells.push({
        tier: "surveyed",
        cellId,
        observedTick: 10,
        isLiveHqSight: true,
        elevationM,
        confidence01: 1.0,
        provenanceLabel: "Direct Hydrology Survey",
        biomeKey: isOcean ? "shallow_sea" : "temperate_forest",
        isOcean,
        geologyProvince: isOcean ? "continental_shelf" : "alluvial_plain",
        roughnessM: isOcean ? 12 : 24,
        riverChannelDownstreamCellId: ds,
        riverDownstreamCellId: ds,
        riverFlowNorm01: isOcean ? 0 : 0.8,
        riverDischargeM3S: isOcean ? 0 : 120 + strahler * 85,
        riverVelocityMps: isOcean ? 0 : 1.4,
        hydroPotentialKw: isOcean ? 0 : 6500,
        strahlerOrder: strahler,
        isEstuary: estuary,
        riverNavigability: isOcean ? "none" : "deep_navigable",
      });
    };

    addCell(hq, 620, false, n1, 2, false);
    addCell(n1, 140, false, n2, 3, false);
    addCell(n2, 6, false, oceanId, 4, true); // Very low coastal plain (+6m) that previously triggered gaps
    addCell(oceanId, -12, true, null, 1, false); // Shallow ocean cell like #1263 in screenshot
    for (const cid of [hq, n1, n2, oceanId]) {
      for (const nb of topo.cells[cid].neighbors) {
        addCell(nb, pushed.has(nb) ? 0 : -15, true, null, 1, false);
      }
    }

    const snap: DiegeticSnapshot = {
      ...BASE_FIXTURE,
      globeTopology: { subdivision: 3, cellCount: topo.cellCount },
      knownCells: riverCells,
    };

    const sampler = createShaderTerrainSurfaceSampler(snap, topo);
    // Critical shoreline fix: shallow ocean center node (-12m) must NEVER flip into positive green land!
    const sOceanCenter = sampler.sample(topo.cells[oceanId].centerUnit, oceanId, oceanId, 0.25);
    expect(sOceanCenter.isExplored).toBe(true);
    expect(sOceanCenter.effectiveElevM).toBeLessThan(0);

    // Every land cell center must stay strictly > 0m.
    for (const cid of [hq, n1, n2]) {
      const sLand = sampler.sample(topo.cells[cid].centerUnit, cid, cid, 0.25);
      expect(sLand.isExplored).toBe(true);
      expect(sLand.effectiveElevM).toBeGreaterThan(0);
    }

    const mat = new THREE.MeshBasicMaterial();
    const projectPoint = (u: readonly [number, number, number], lift: number) =>
      new THREE.Vector3(u[0], u[1], u[2]).normalize().multiplyScalar(1 + lift);

    const ribbon = buildHydrologyRibbonMesh(
      snap,
      topo,
      cellMap,
      new Map<number, ObservedWeatherDto>(),
      0.2,
      mat,
      projectPoint,
      false,
      sampler,
    );
    expect(ribbon.renderedRiverCount).toBe(1);
    expect(ribbon.mesh).not.toBeNull();

    const posAttr = ribbon.mesh!.geometry.getAttribute("position") as THREE.BufferAttribute;
    const idxAttr = ribbon.mesh!.geometry.getIndex()!;
    // Unbroken single-strip quad index continuity from headwater through +6m plain to 0m shoreline:
    expect(posAttr.count).toBeGreaterThanOrEqual(24);
    expect(idxAttr.count).toBe(((posAttr.count / 2) - 1) * 6);

    // Centerline radius (altitude above sphere 1.0) must be strictly non-increasing along flow downstream!
    let prevMidR = Infinity;
    for (let v = 0; v + 1 < posAttr.count; v += 2) {
      const mx = 0.5 * (posAttr.getX(v) + posAttr.getX(v + 1));
      const my = 0.5 * (posAttr.getY(v) + posAttr.getY(v + 1));
      const mz = 0.5 * (posAttr.getZ(v) + posAttr.getZ(v + 1));
      const midR = Math.hypot(mx, my, mz);
      expect(midR).toBeLessThanOrEqual(prevMidR + 1e-6);
      prevMidR = midR;
    }

    // Estuary mouth near shoreline widens vs upstream headwater width!
    const headWidth = Math.hypot(
      posAttr.getX(1) - posAttr.getX(0),
      posAttr.getY(1) - posAttr.getY(0),
      posAttr.getZ(1) - posAttr.getZ(0),
    );
    const tailIdx = posAttr.count - 4;
    const mouthWidth = Math.hypot(
      posAttr.getX(tailIdx + 1) - posAttr.getX(tailIdx),
      posAttr.getY(tailIdx + 1) - posAttr.getY(tailIdx),
      posAttr.getZ(tailIdx + 1) - posAttr.getZ(tailIdx),
    );
    expect(mouthWidth).toBeGreaterThan(headWidth * 1.25);

    ribbon.mesh!.geometry.dispose();
    mat.dispose();
  });

  // @req UI-MAP-010
  it("exposes Geology & Bathymetry overlay mode, masks unexplored cells in uGeologyTex, renders pickable dams, and dispatches dam engineering commands", async () => {
    const dom = setupJsdom();
    const topo = generateGoldbergTopology(3);
    const riverCell = BASE_FIXTURE.hqPresence.cellId;
    const dsCell = topo.cells[riverCell].neighbors[0];
    const unexplCell = topo.cells.find((c) => !BASE_FIXTURE.knownCells.some((k) => k.cellId === c.cellId) && c.cellId !== dsCell)!.cellId;

    const geoCells: KnownCellDto[] = [
      {
        tier: "surveyed",
        cellId: riverCell,
        observedTick: 10,
        isLiveHqSight: true,
        elevationM: 280,
        confidence01: 1.0,
        provenanceLabel: "Direct Survey",
        biomeKey: "temperate_forest",
        isOcean: false,
        geologyProvince: "young_orogen",
        roughnessM: 185,
        soilDrainage01: 0.68,
        soilClay01: 0.28,
        riverChannelDownstreamCellId: dsCell,
        riverDownstreamCellId: dsCell,
        riverFlowNorm01: 0.72,
        riverDischargeM3S: 165,
        riverVelocityMps: 1.8,
        hydroPotentialKw: 8400,
        strahlerOrder: 3,
        riverNavigability: "rapids",
        isSpringOasis: true,
        depositEstimates: [
          {
            materialId: 1,
            materialLabel: "Magnetite Ore",
            categoryKey: "MetallicOre",
            minTonnes: 18000,
            pointEstTonnes: 24000,
            maxTonnes: 31000,
            purityEst01: 0.56,
            confidence01: 0.85,
            surveyorSkill01: 0.75,
            instrumentTier: 1,
            observedTick: 10,
            provenanceLabel: "Core Assay",
            accessibilityTier: 2,
            isOffshore: false,
            isAlluvialPlacer: true,
          },
        ],
      },
      {
        tier: "surveyed",
        cellId: dsCell,
        observedTick: 10,
        isLiveHqSight: true,
        elevationM: 18,
        confidence01: 1.0,
        provenanceLabel: "Direct Survey",
        biomeKey: "warm_desert",
        isOcean: false,
        isLake: true,
        lakeKind: "endorheic",
        lakeSalinityPerMille: 64.0,
        geologyProvince: "sedimentary_basin",
        roughnessM: 22,
      },
      { tier: "unexplored", cellId: unexplCell },
    ];

    const snap: DiegeticSnapshot = {
      ...BASE_FIXTURE,
      globeTopology: { subdivision: 3, cellCount: topo.cellCount },
      knownCells: geoCells,
    };

    // 1. Verify shader controller uGeologyTex masking & uGeologyMode toggle.
    const shaderCtrl = createGlobeShaderController();
    shaderCtrl.uploadSnapshot(snap, null, null);
    shaderCtrl.setGeologyMode?.(true);
    expect(shaderCtrl.material.uniforms.uGeologyMode.value).toBe(1);
    const geoTex = shaderCtrl.material.uniforms.uGeologyTex.value as THREE.DataTexture;
    const geoBuf = geoTex.image.data as Uint8Array;
    expect(geoBuf[riverCell * 4]).toBe(3 * 18); // young_orogen = code 3
    expect(geoBuf[riverCell * 4 + 3] & 2).toBe(2); // spring oasis bit set
    expect(geoBuf[unexplCell * 4]).toBe(0);
    expect(geoBuf[unexplCell * 4 + 1]).toBe(0);
    expect(geoBuf[unexplCell * 4 + 2]).toBe(0);
    expect(geoBuf[unexplCell * 4 + 3]).toBe(0);
    shaderCtrl.dispose();

    // 2. Verify OverlayToolbarLayer Geology button & GeologyBathymetryOverlayLayer summary.
    const root = dom.window.document.getElementById("root")!;
    const canvas = dom.window.document.createElement("canvas");
    root.appendChild(canvas);
    const bus = new UiIntentBus();
    const store = new DiegeticStore();
    store.ingestSnapshot(snap);
    const host: GlobeLayerHost = {
      getCanvasElement: () => canvas,
      overlayRootId: "root",
      getCameraAltitudeNorm: () => 0.45,
      projectLatLonToScreen: () => ({ xPx: 0, yPx: 0, visible: false }),
      getProjectionMode: () => "globe",
    };

    const toolbar = new OverlayToolbarLayer();
    const geoLayer = new GeologyBathymetryOverlayLayer();
    toolbar.mount(host, store, bus);
    geoLayer.mount(host, store, bus);
    geoLayer.onSnapshotUpdate(snap);
    expect(geoLayer.supportsMode("geology")).toBe(true);
    expect(geoLayer.getSummary()?.geologyProvinceCount).toBe(2);
    expect(geoLayer.getSummary()?.endorheicLakeCount).toBe(1);
    expect(geoLayer.getSummary()?.springOasisCount).toBe(1);

    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));
    const geoBtn = root.querySelector("button[data-overlay-mode='geology']") as HTMLButtonElement;
    expect(geoBtn).toBeTruthy();
    geoBtn.click();
    expect(emitted.some((e) => e.kind === "SwitchMapOverlayMode" && e.modeId === "geology")).toBe(true);
    expect(root.querySelector("#map-overlay-legend-summary")?.textContent).toContain("Endorheic 1");

    // 3. Verify Inspector displays Geology/Placer/Hydrology telemetry and Construct Dam -> Set Dam Rule Curve roundtrip via MockPlayer & IPC parser.
    const mockPlayer = new MockSnapshotPlayer([snap]);
    let latestSnap = snap;
    mockPlayer.subscribeSnapshots((s) => {
      latestSnap = s;
    });
    const inspector = createEntityInspectorWindowPlugin();
    const viewRoot = dom.window.document.createElement("div");
    dom.window.document.body.appendChild(viewRoot);
    let disposeInsp = inspector.mountContent(viewRoot, `river:${riverCell}`, { store, bus });

    expect(viewRoot.textContent).toContain("Young Orogen");
    expect(viewRoot.textContent).toContain("Rapids");

    const buildDamBtn = viewRoot.querySelector("button[data-action='construct-dam']") as HTMLButtonElement;
    expect(buildDamBtn).toBeTruthy();
    buildDamBtn.click();

    const buildIntent = emitted.find((i) => i.kind === "DispatchSimCommand");
    expect(buildIntent?.kind).toBe("DispatchSimCommand");
    if (buildIntent?.kind === "DispatchSimCommand") {
      const ipcValid = parsePlayerCommand(buildIntent.command);
      expect(ipcValid.ok).toBe(true);
      const ack = await mockPlayer.sendPlayerCommand(buildIntent.command);
      expect(ack.accepted).toBe(true);
    }

    store.ingestSnapshot(latestSnap);
    disposeInsp();
    disposeInsp = inspector.mountContent(viewRoot, `cell:${riverCell}`, { store, bus });

    expect(viewRoot.textContent).toContain("Dam Status");
    expect(viewRoot.textContent).toContain("Depth Tier 2");
    expect(viewRoot.textContent).toContain("Alluvial Placer");
    expect(viewRoot.textContent).toContain("Aquifer Spring Oasis");
    const floodBtn = viewRoot.querySelector("button[data-rule-curve='flood_control']") as HTMLButtonElement;
    expect(floodBtn).toBeTruthy();
    floodBtn.click();

    const switchIntent = emitted
      .filter((i) => i.kind === "DispatchSimCommand")
      .pop();
    expect(switchIntent?.kind).toBe("DispatchSimCommand");
    if (switchIntent?.kind === "DispatchSimCommand") {
      const ipcValid2 = parsePlayerCommand(switchIntent.command);
      expect(ipcValid2.ok).toBe(true);
      expect(switchIntent.command).toEqual({
        kind: "setDamRuleCurve",
        targetCell: riverCell,
        ruleCurve: "flood_control",
      });
    }

    // 4. Verify GlobeFeatureOverlayGroup renders pickable 3D masonry dam mesh + reservoir/lake basins.
    const features = new GlobeFeatureOverlayGroup();
    features.rebuild(latestSnap, topo, "geology", null, 0.2);
    const pickableDam = features.group.children.find(
      (obj) => obj.userData?.pickHit?.hitKind === "river" && obj.userData?.pickHit?.cellId === riverCell,
    );
    expect(pickableDam).toBeTruthy();
    features.dispose();
    disposeInsp();
    toolbar.dispose();
    geoLayer.dispose();
  });
});
