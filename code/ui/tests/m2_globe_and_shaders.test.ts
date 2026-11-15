import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  GlobePalette,
  generateGoldbergTopology,
  greatCircleRadians,
} from "../src/spine";
import {
  GlobeOrbitalCamera,
  buildGoldbergGlobeGeometry,
  createGlobeShaderController,
  verifyGoldbergManifold,
} from "../src/globe";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m2_snapshot.json");
const M2_SNAPSHOTS = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot[];

const INDUSTRIAL_PALETTE: GlobePalette = {
  oceanShallowRgb: [42, 73, 92],
  oceanDeepRgb: [18, 33, 43],
  landLowRgb: [78, 92, 82],
  landMidRgb: [114, 120, 106],
  landHighRgb: [141, 121, 86],
  peakSnowRgb: [214, 221, 227],
  riverWaterRgb: [69, 148, 184],
  roadOchreRgb: [228, 122, 48],
  settlementMarkerRgb: [212, 166, 68],
  contourInkRgb: [212, 166, 68],
  compassRoseRgb: [228, 122, 48],
  unexploredVellumRgb: [36, 44, 53],
  parchmentGrainTextureUrl: "",
  hatchPatternTextureUrl: "",
  terrainBiomeTextureUrl: "",
};

describe("M2 Spherical Goldberg Mesh, Pole-Free Camera, Double-Buffered Shaders & Air-Gap", () => {
  // @req UI-GLB-001
  it("constructs closed manifold GP(8,0) Goldberg globe with 12 pentagons and supports continuous polar navigation (UI-GLB-001)", () => {
    const topo = generateGoldbergTopology(3);
    const build = buildGoldbergGlobeGeometry(topo);
    const report = verifyGoldbergManifold(build);

    expect(report.valid).toBe(true);
    expect(report.cellCount).toBe(642);
    expect(report.pentagonCount).toBe(12);
    expect(report.hexagonCount).toBe(630);
    expect(report.unmatchedHalfEdges).toBe(0);
    expect(report.degenerateTriangles).toBe(0);
    expect(report.nonUnitVertices).toBe(0);
    expect(report.eulerCheckDual).toBe(2);

    // Pole-free continuous arcball orbit across North Pole (+90deg), antimeridian, and South Pole (-90deg).
    const cam = new GlobeOrbitalCamera(16 / 9, 0.5);
    let maxY = -1;
    let minY = 1;
    for (let step = 0; step < 60; step += 1) {
      cam.panByPixels(0, 60, 800);
      const focus = cam.getFocusUnit();
      expect(focus.every((v) => Number.isFinite(v))).toBe(true);
      const norm = Math.hypot(focus[0], focus[1], focus[2]);
      expect(Math.abs(norm - 1)).toBeLessThan(1e-5);
      maxY = Math.max(maxY, focus[1]);
      minY = Math.min(minY, focus[1]);
    }
    expect(maxY).toBeGreaterThan(0.95);
    expect(minY).toBeLessThan(-0.95);

    // Tilted off-axis state can cleanly realign North via resetNorth().
    cam.panByPixels(180, -90, 800);
    cam.tiltByPixels(-80);
    expect(cam.getTelemetry().pitchRad).toBeGreaterThan(0.1);
    cam.resetNorth(true);
    expect(cam.getTelemetry().pitchRad).toBe(0);
    expect(Math.abs(cam.getHeadingRadians())).toBeLessThan(1e-4);

    build.geometry.dispose();
  });

  // @req UI-GLB-002
  it("zooms logarithmically across [1.12..3.60] and blends monotonically from 3D relief to antique cartographic parchment (UI-GLB-002)", () => {
    const cam = new GlobeOrbitalCamera(16 / 9, 0.0);
    const radii: number[] = [];
    const blends: number[] = [];

    for (let i = 0; i <= 10; i += 1) {
      cam.setAltitudeNorm(i / 10);
      radii.push(cam.getAltitudeRadius());
      blends.push(cam.getCartographicBlend());
    }

    expect(radii[0]).toBeCloseTo(1.12, 3);
    expect(radii[radii.length - 1]).toBeCloseTo(3.6, 3);
    expect(blends[0]).toBe(0);
    expect(blends[blends.length - 1]).toBe(1);

    for (let i = 1; i < radii.length; i += 1) {
      expect(radii[i]).toBeGreaterThan(radii[i - 1]);
      expect(blends[i]).toBeGreaterThanOrEqual(blends[i - 1]);
      // Constant log ratio between equal altitudeNorm steps
      if (i > 1) {
        const ratioPrev = radii[i - 1] / radii[i - 2];
        const ratioCur = radii[i] / radii[i - 1];
        expect(ratioCur).toBeCloseTo(ratioPrev, 4);
      }
    }
  });

  // @req UI-GLB-003
  it("executes shortest spherical geodesic slerp fly-to along unit sphere and cancels on user input (UI-GLB-003)", () => {
    const topo = generateGoldbergTopology(3);
    const cam = new GlobeOrbitalCamera(16 / 9, 0.9);
    const targetCell = topo.cells[210];
    const startFocus = cam.getFocusUnit();
    const initialDist = greatCircleRadians(startFocus, targetCell.centerUnit);
    expect(initialDist).toBeGreaterThan(0.2);

    cam.flyToUnitVector(targetCell.centerUnit, 0.18, 500, false);
    let prevDist = initialDist;
    for (let step = 0; step < 10; step += 1) {
      cam.stepAnimation(50);
      const cur = cam.getFocusUnit();
      expect(Math.abs(Math.hypot(cur[0], cur[1], cur[2]) - 1)).toBeLessThan(1e-5);
      const dist = greatCircleRadians(cur, targetCell.centerUnit);
      expect(dist).toBeLessThanOrEqual(prevDist + 1e-6);
      prevDist = dist;
    }
    expect(prevDist).toBeLessThan(0.01);
    expect(cam.getAltitudeNorm()).toBeCloseTo(0.18, 3);

    // User drag mid-flight interrupts the active geodesic transition cleanly.
    const distantCell = topo.cells[17];
    cam.flyToUnitVector(distantCell.centerUnit, 0.5, 600, false);
    cam.stepAnimation(100);
    const midUnit = cam.getFocusUnit();
    cam.panByPixels(40, 20, 800);
    const afterPan = cam.getFocusUnit();
    cam.stepAnimation(400);
    expect(cam.getFocusUnit()).toEqual(afterPan);
    expect(greatCircleRadians(afterPan, midUnit)).toBeGreaterThan(0.001);
  });

  // @req UI-GLB-004
  it("double-buffers GPU cell DataTexture uploads within 16ms frame budget across large snapshot updates (UI-GLB-004)", () => {
    const shaderCtrl = createGlobeShaderController();
    const initialIdx = shaderCtrl.activeTextureIndex();

    const syntheticCells = Array.from({ length: 4096 }, (_, idx) =>
      idx % 3 === 0
        ? ({ tier: "unexplored", cellId: idx } as const)
        : ({
            tier: "surveyed",
            cellId: idx,
            observedTick: 12,
            isLiveHqSight: idx < 32,
            elevationM: (idx % 200) * 15 - 600,
            confidence01: 0.9,
            provenanceLabel: "Stress Benchmark",
            biomeKey: idx % 5 === 0 ? "coastal_sea" : "temperate_forest",
            isOcean: idx % 5 === 0,
            riverDownstreamCellId: null,
            riverFlowNorm01: 0,
          } as const),
    );
    const heavySnap: DiegeticSnapshot = {
      ...M2_SNAPSHOTS[0],
      knownCells: syntheticCells,
    };

    const t0 = performance.now();
    shaderCtrl.uploadSnapshot(heavySnap, 0, 1);
    const idxAfterFirst = shaderCtrl.activeTextureIndex();
    shaderCtrl.uploadSnapshot(heavySnap, 2, 4);
    const idxAfterSecond = shaderCtrl.activeTextureIndex();
    const elapsedMs = performance.now() - t0;

    expect(idxAfterFirst).not.toBe(initialIdx);
    expect(idxAfterSecond).toBe(initialIdx);
    expect(elapsedMs).toBeLessThan(16);

    shaderCtrl.dispose();
  });

  // @req UI-GLB-005
  // @req UI-SKN-004
  it("zeros out Unexplored cells in GPU buffers (epistemic air-gap) and binds external Skin palette uniforms without hardcoded colors (UI-GLB-005, UI-SKN-004)", () => {
    const store = new DiegeticStore();
    store.ingestSnapshot(M2_SNAPSHOTS[0]);
    const snap = store.getSnapshot()!;

    const shaderCtrl = createGlobeShaderController();
    shaderCtrl.uploadSnapshot(snap, null, null);

    const tex = shaderCtrl.material.uniforms.uCellDataTex.value as { image: { data: Uint8Array } };
    const data = tex.image.data;

    // Cell 0 is surveyed with live weather -> non-zero R/B/A bytes
    expect(data[0 * 4]).toBeGreaterThan(127);
    expect(data[0 * 4 + 3]).toBeGreaterThanOrEqual(170);

    // Cell 3 is stale -> non-zero R elevation but muted stale alpha tier (85)
    expect(data[3 * 4]).toBeGreaterThan(0);
    expect(data[3 * 4 + 3]).toBe(85);

    // Cells 5..8 are Unexplored -> all 4 RGBA bytes MUST be exact 0
    for (const unexploredId of [5, 6, 7, 8]) {
      const off = unexploredId * 4;
      expect([data[off], data[off + 1], data[off + 2], data[off + 3]]).toEqual([0, 0, 0, 0]);
    }

    // Swapping external Skin GlobePalette updates shader color uniforms immediately (`UI-SKN-004`)
    const beforeVellum = shaderCtrl.material.uniforms.uUnexploredVellumRgb.value.clone();
    shaderCtrl.setPalette(INDUSTRIAL_PALETTE);
    const afterVellum = shaderCtrl.material.uniforms.uUnexploredVellumRgb.value;
    expect(afterVellum.x).toBeCloseTo(36 / 255, 4);
    expect(afterVellum.y).toBeCloseTo(44 / 255, 4);
    expect(afterVellum.z).toBeCloseTo(53 / 255, 4);
    expect(afterVellum.distanceTo(beforeVellum)).toBeGreaterThan(0.3);

    shaderCtrl.dispose();
  });
});
