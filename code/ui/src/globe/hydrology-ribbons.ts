import * as THREE from "three";
import {
  add3,
  DiegeticSnapshot,
  GoldbergCellGeometry,
  GoldbergTopologyData,
  KnownCellDto,
  norm3,
  ObservedWeatherDto,
  scale3,
  slerpUnitVec3,
  sub3,
  tangentBasis,
  Vec3,
} from "../spine";

export type SurfacePointProjector = (unit: Vec3, lift: number) => THREE.Vector3;

function deterministicSignedNoise(a: number, b: number, salt: number): number {
  let h = (a * 374761393 + b * 668265263 + salt * 2147483647) | 0;
  h = ((h ^ (h >> 13)) * 1274126177) | 0;
  const norm = ((h ^ (h >> 16)) >>> 0) / 4294967295;
  return norm * 2 - 1;
}

function buildMeanderingSpine(
  srcUnit: Vec3,
  dstUnit: Vec3,
  srcId: number,
  dstId: number,
  truncatedAtFog: boolean,
  segments: number,
): { readonly points: readonly Vec3[]; readonly widthsNorm: readonly number[] } {
  const dir = sub3(dstUnit, srcUnit);
  const segLen = Math.hypot(dir[0], dir[1], dir[2]);
  const [east, north] = tangentBasis(srcUnit);
  const chordDir = segLen > 1e-6 ? scale3(dir, 1 / segLen) : east;
  // Lateral normal perpendicular to both surface normal and river chord direction.
  const lateralRaw: Vec3 = [
    srcUnit[1] * chordDir[2] - srcUnit[2] * chordDir[1],
    srcUnit[2] * chordDir[0] - srcUnit[0] * chordDir[2],
    srcUnit[0] * chordDir[1] - srcUnit[1] * chordDir[0],
  ];
  const latLen = Math.hypot(lateralRaw[0], lateralRaw[1], lateralRaw[2]);
  const lateral: Vec3 = latLen > 1e-6 ? scale3(lateralRaw, 1 / latLen) : north;

  const maxT = truncatedAtFog ? 0.42 : 1.0;
  const amp = Math.min(0.022, segLen * 0.24);
  const phaseA = deterministicSignedNoise(srcId, dstId, 11) * amp;
  const phaseB = deterministicSignedNoise(srcId, dstId, 29) * amp * 0.85;

  const ctrlA = norm3(add3(slerpUnitVec3(srcUnit, dstUnit, 0.30 * maxT), scale3(lateral, phaseA)));
  const ctrlB = norm3(add3(slerpUnitVec3(srcUnit, dstUnit, 0.68 * maxT), scale3(lateral, -phaseB)));
  const endPt = truncatedAtFog ? slerpUnitVec3(srcUnit, dstUnit, maxT) : dstUnit;

  const curvePts: Vec3[] = [srcUnit, ctrlA, ctrlB, endPt];
  const points: Vec3[] = [];
  const widthsNorm: number[] = [];

  for (let i = 0; i <= segments; i += 1) {
    const t = i / segments;
    const u = 1 - t;
    // Cubic Bezier evaluation projected smoothly back onto the unit sphere.
    const b0 = u * u * u;
    const b1 = 3 * u * u * t;
    const b2 = 3 * u * t * t;
    const b3 = t * t * t;
    const pt = norm3([
      curvePts[0][0] * b0 + curvePts[1][0] * b1 + curvePts[2][0] * b2 + curvePts[3][0] * b3,
      curvePts[0][1] * b0 + curvePts[1][1] * b1 + curvePts[2][1] * b2 + curvePts[3][1] * b3,
      curvePts[0][2] * b0 + curvePts[1][2] * b1 + curvePts[2][2] * b2 + curvePts[3][2] * b3,
    ]);
    points.push(pt);
    widthsNorm.push(truncatedAtFog ? Math.max(0.05, 1 - t) : 1.0);
  }
  return { points, widthsNorm };
}

function appendRibbonStrip(
  positions: number[],
  normals: number[],
  uvs: number[],
  indices: number[],
  projectedPts: readonly THREE.Vector3[],
  halfWidths: readonly number[],
  upNormals: readonly THREE.Vector3[],
): void {
  if (projectedPts.length < 2) return;
  const baseVertex = positions.length / 3;
  let emittedVerts = 0;

  for (let i = 0; i < projectedPts.length; i += 1) {
    const cur = projectedPts[i];
    const prev = projectedPts[Math.max(0, i - 1)];
    const next = projectedPts[Math.min(projectedPts.length - 1, i + 1)];

    // Break strip cleanly across 2D Mercator wrap discontinuities.
    if (i > 0 && cur.distanceTo(projectedPts[i - 1]) > 0.72) {
      return;
    }

    const tangent = next.clone().sub(prev);
    if (tangent.lengthSq() < 1e-12) tangent.set(1, 0, 0);
    tangent.normalize();
    const up = upNormals[i];
    const side = new THREE.Vector3().crossVectors(up, tangent);
    if (side.lengthSq() < 1e-12) side.set(0, 1, 0);
    side.normalize();

    const hw = halfWidths[i];
    const left = cur.clone().addScaledVector(side, hw);
    const right = cur.clone().addScaledVector(side, -hw);

    positions.push(left.x, left.y, left.z, right.x, right.y, right.z);
    normals.push(up.x, up.y, up.z, up.x, up.y, up.z);
    const uCoord = i / Math.max(1, projectedPts.length - 1);
    uvs.push(uCoord, 0, uCoord, 1);
    emittedVerts += 2;
  }

  for (let i = 0; i + 2 < emittedVerts; i += 2) {
    const a = baseVertex + i;
    const b = baseVertex + i + 1;
    const c = baseVertex + i + 2;
    const d = baseVertex + i + 3;
    indices.push(a, b, c, c, b, d);
  }
}

export interface RiverRibbonBuildResult {
  readonly mesh: THREE.Mesh | null;
  readonly renderedRiverCount: number;
  readonly floodedRiverCount: number;
}

/**
 * Constructs single tapered, meandering triangle-strip river ribbons strictly inside surveyed space,
 * clipping & tapering cleanly before unexplored cells (`UI-GLB-006`, `UI-MAP-003`, `UI-MAP-004`).
 */
export function buildHydrologyRibbonMesh(
  snapshot: DiegeticSnapshot,
  topology: GoldbergTopologyData,
  cellMap: ReadonlyMap<number, GoldbergCellGeometry>,
  weatherByCell: ReadonlyMap<number, ObservedWeatherDto>,
  altitudeNorm: number,
  material: THREE.Material,
  projectPoint: SurfacePointProjector,
  flatMode: boolean,
): RiverRibbonBuildResult {
  const knownById = new Map<number, KnownCellDto>();
  for (const c of snapshot.knownCells) {
    knownById.set(c.cellId, c);
  }

  const positions: number[] = [];
  const normals: number[] = [];
  const uvs: number[] = [];
  const indices: number[] = [];

  let renderedRiverCount = 0;
  let floodedRiverCount = 0;
  const densityScale = Math.sqrt(Math.max(1, topology.cellCount) / 642);
  const zoomWidthScale = Math.max(0.28, 0.55 + 0.45 * altitudeNorm) / Math.max(1, densityScale * 0.68);
  const lift = flatMode ? 0.008 : 0.007;

  for (const cell of snapshot.knownCells) {
    if (cell.tier !== "surveyed") continue;
    if (cell.isOcean || cell.elevationM < 0) continue;
    if (cell.riverDownstreamCellId === null || cell.riverDownstreamCellId === undefined) continue;

    const src = cellMap.get(cell.cellId);
    const dstGeom = cellMap.get(cell.riverDownstreamCellId);
    if (!src || !dstGeom) continue;

    const dstKnown = knownById.get(cell.riverDownstreamCellId);
    const truncatedAtFog = !dstKnown || dstKnown.tier === "unexplored" || dstKnown.tier === "rumored";
    const dstIsOcean = Boolean(dstKnown && dstKnown.tier !== "unexplored" && (dstKnown.isOcean || dstKnown.elevationM < 0));

    const obsW = weatherByCell.get(cell.cellId);
    const floodStage = obsW?.floodStage01 ?? 0;
    if (floodStage > 0.25) {
      floodedRiverCount += 1;
    }

    const strahler = cell.strahlerOrder ?? 1;
    const flowNorm = cell.riverFlowNorm01 ?? 0.35;
    if (altitudeNorm > 0.72 && strahler <= 1 && flowNorm < 0.25 && floodStage <= 0.25) {
      continue;
    }

    renderedRiverCount += 1;
    const segCount = altitudeNorm < 0.28 ? 10 : 7;
    const { points, widthsNorm } = buildMeanderingSpine(
      src.centerUnit,
      dstGeom.centerUnit,
      cell.cellId,
      cell.riverDownstreamCellId,
      truncatedAtFog,
      segCount,
    );

    const baseHalfW =
      (0.0012 + Math.min(4, strahler) * 0.00055 + flowNorm * 0.00085) *
      (1 + floodStage * 0.85) *
      zoomWidthScale;

    const projectedPts: THREE.Vector3[] = [];
    const halfWidths: number[] = [];
    const upNormals: THREE.Vector3[] = [];

    for (let i = 0; i < points.length; i += 1) {
      const t = i / (points.length - 1);
      const estuaryFlare = dstIsOcean ? 1 + Math.pow(t, 2.2) * 0.75 : 1.0;
      const hw = baseHalfW * widthsNorm[i] * estuaryFlare;
      projectedPts.push(projectPoint(points[i], lift));
      halfWidths.push(hw);
      upNormals.push(
        flatMode
          ? new THREE.Vector3(0, 0, 1)
          : new THREE.Vector3(points[i][0], points[i][1], points[i][2]).normalize(),
      );
    }

    appendRibbonStrip(positions, normals, uvs, indices, projectedPts, halfWidths, upNormals);
  }

  if (indices.length === 0) {
    return { mesh: null, renderedRiverCount, floodedRiverCount };
  }

  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geom.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  geom.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
  geom.setIndex(indices);
  geom.computeBoundingSphere();

  const mesh = new THREE.Mesh(geom, material);
  mesh.frustumCulled = false;
  return { mesh, renderedRiverCount, floodedRiverCount };
}

/**
 * Constructs smooth curved road triangle-strip ribbons for intact road corridors (`UI-GLB-006`).
 */
export function buildRoadRibbonMesh(
  snapshot: DiegeticSnapshot,
  topology: GoldbergTopologyData,
  cellMap: ReadonlyMap<number, GoldbergCellGeometry>,
  altitudeNorm: number,
  material: THREE.Material,
  projectPoint: SurfacePointProjector,
  flatMode: boolean,
): THREE.Mesh | null {
  const knownById = new Map<number, KnownCellDto>();
  for (const c of snapshot.knownCells) {
    knownById.set(c.cellId, c);
  }

  const positions: number[] = [];
  const normals: number[] = [];
  const uvs: number[] = [];
  const indices: number[] = [];

  const densityScale = Math.sqrt(Math.max(1, topology.cellCount) / 642);
  const hw = (0.0014 * Math.max(0.32, 0.55 + 0.45 * altitudeNorm)) / Math.max(1, densityScale * 0.68);
  const lift = flatMode ? 0.009 : 0.008;

  for (const road of snapshot.surveyedRoads ?? []) {
    if (road.washedOut) continue;
    const src = cellMap.get(road.fromCellId);
    const dst = cellMap.get(road.toCellId);
    if (!src || !dst) continue;
    const srcKnown = knownById.get(road.fromCellId);
    const dstKnown = knownById.get(road.toCellId);
    if (
      !srcKnown ||
      srcKnown.tier === "unexplored" ||
      !dstKnown ||
      dstKnown.tier === "unexplored"
    ) {
      continue;
    }

    const { points } = buildMeanderingSpine(
      src.centerUnit,
      dst.centerUnit,
      road.fromCellId,
      road.toCellId,
      false,
      6,
    );
    const projectedPts: THREE.Vector3[] = [];
    const halfWidths: number[] = [];
    const upNormals: THREE.Vector3[] = [];
    for (const pt of points) {
      projectedPts.push(projectPoint(pt, lift));
      halfWidths.push(hw * (road.infraKey.includes("metalled") || road.infraKey.includes("rail") ? 1.35 : 1.0));
      upNormals.push(
        flatMode ? new THREE.Vector3(0, 0, 1) : new THREE.Vector3(pt[0], pt[1], pt[2]).normalize(),
      );
    }
    appendRibbonStrip(positions, normals, uvs, indices, projectedPts, halfWidths, upNormals);
  }

  if (indices.length === 0) return null;

  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geom.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  geom.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
  geom.setIndex(indices);
  geom.computeBoundingSphere();

  const mesh = new THREE.Mesh(geom, material);
  mesh.frustumCulled = false;
  return mesh;
}
