import * as THREE from "three";
import {
  add3,
  cross3,
  DiegeticSnapshot,
  dot3,
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

const SHORELINE_STOP_ELEV_M = 2.0;

function fround(x: number): number {
  return Math.fround(x);
}

function fract(x: number): number {
  return x - Math.floor(x);
}

function clamp(v: number, lo: number, hi: number): number {
  return v < lo ? lo : v > hi ? hi : v;
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  const t = clamp((x - edge0) / (edge1 - edge0), 0, 1);
  return t * t * (3 - 2 * t);
}

function encodeShaderElevByte(elevM: number, isOcean: boolean): number {
  if (isOcean || elevM < 0) {
    const t = clamp(1 + elevM / 5000, 0, 1);
    return clamp(Math.round(1 + t * 126), 1, 127);
  }
  const t = clamp(elevM / 5000, 0, 1);
  return clamp(Math.round(128 + t * 127), 128, 255);
}

function decodeShaderElevMeters(elevNorm: number): number {
  if (elevNorm < 0.5) {
    return (elevNorm / 0.5 - 1.0) * 5000.0;
  }
  return ((elevNorm - 0.5) / 0.5) * 5000.0;
}

function hash31(px: number, py: number, pz: number): number {
  let x = fract(fround(px * 443.897));
  let y = fract(fround(py * 441.423));
  let z = fract(fround(pz * 437.195));
  const d = fround(x * fround(y + 19.19) + y * fround(z + 19.19) + z * fround(x + 19.19));
  x = fround(x + d);
  y = fround(y + d);
  z = fround(z + d);
  return fract(fround(fround(x + y) * z));
}

function vnoise3(px: number, py: number, pz: number): number {
  const ix = Math.floor(px);
  const iy = Math.floor(py);
  const iz = Math.floor(pz);
  const ux = px - ix;
  const uy = py - iy;
  const uz = pz - iz;
  const fx = ux * ux * (3.0 - 2.0 * ux);
  const fy = uy * uy * (3.0 - 2.0 * uy);
  const fz = uz * uz * (3.0 - 2.0 * uz);

  const n000 = hash31(ix, iy, iz);
  const n100 = hash31(ix + 1, iy, iz);
  const n010 = hash31(ix, iy + 1, iz);
  const n110 = hash31(ix + 1, iy + 1, iz);
  const n001 = hash31(ix, iy, iz + 1);
  const n101 = hash31(ix + 1, iy, iz + 1);
  const n011 = hash31(ix, iy + 1, iz + 1);
  const n111 = hash31(ix + 1, iy + 1, iz + 1);

  const nx00 = n000 + (n100 - n000) * fx;
  const nx10 = n010 + (n110 - n010) * fx;
  const nx01 = n001 + (n101 - n001) * fx;
  const nx11 = n011 + (n111 - n011) * fx;

  const nxy0 = nx00 + (nx10 - nx00) * fy;
  const nxy1 = nx01 + (nx11 - nx01) * fy;
  return nxy0 + (nxy1 - nxy0) * fz;
}

function fbm4(px: number, py: number, pz: number): number {
  let sum = 0.0;
  let amp = 0.5;
  let x = px;
  let y = py;
  let z = pz;
  for (let i = 0; i < 4; i += 1) {
    sum += amp * vnoise3(x, y, z);
    x = x * 2.07 + 13.7;
    y = y * 2.07 - 7.3;
    z = z * 2.07 + 21.1;
    amp *= 0.5;
  }
  return sum;
}

function ridgedFbm3(px: number, py: number, pz: number): number {
  let sum = 0.0;
  let amp = 0.55;
  let norm = 0.0;
  let x = px;
  let y = py;
  let z = pz;
  for (let i = 0; i < 3; i += 1) {
    const r = 1.0 - Math.abs(vnoise3(x, y, z) * 2.0 - 1.0);
    sum += r * r * amp;
    norm += amp;
    x = x * 2.12 + 7.1;
    y = y * 2.12 + 3.9;
    z = z * 2.12 - 11.3;
    amp *= 0.52;
  }
  return sum / Math.max(norm, 0.001);
}

export interface ShaderTerrainSample {
  readonly isExplored: boolean;
  readonly effectiveElevM: number;
  readonly vertexReliefHeight: number;
}

export interface ShaderTerrainSurfaceSampler {
  sample(
    unit: Vec3,
    hintCellA: number,
    hintCellB: number,
    altitudeNorm: number,
  ): ShaderTerrainSample;
  nearestCellId(unit: Vec3, startCellHint?: number): number;
}

/**
 * Evaluates the globe shader's exact interpolated coastline elevation and 3D land relief field on the CPU
 * so river ribbons terminate cleanly at the land/sea shoreline and 3D buildings/trees stay grounded (`UI-GLB-006`, `UI-MAP-008`).
 */
export function createShaderTerrainSurfaceSampler(
  snapshot: DiegeticSnapshot,
  topology: GoldbergTopologyData,
): ShaderTerrainSurfaceSampler {
  const nCells = topology.cellCount;
  const knownMask = new Uint8Array(nCells);
  const bioValidMask = new Uint8Array(nCells);
  const elevNorm = new Float32Array(nCells);
  const isLand = new Uint8Array(nCells);
  const strahlerOrder = new Float32Array(nCells);

  for (const cell of snapshot.knownCells) {
    if (cell.cellId < 0 || cell.cellId >= nCells || cell.tier === "unexplored") continue;
    knownMask[cell.cellId] = 1;
    const elevByte = encodeShaderElevByte(cell.elevationM, Boolean(cell.isOcean || cell.elevationM < 0));
    const norm = elevByte / 255;
    elevNorm[cell.cellId] = norm;
    isLand[cell.cellId] = norm >= 0.499 ? 1 : 0;
    if (cell.tier === "stale" || cell.tier === "surveyed") {
      bioValidMask[cell.cellId] = 1;
      strahlerOrder[cell.cellId] = Math.max(
        0,
        Math.min(7, cell.strahlerOrder ?? ((cell.riverFlowNorm01 ?? 0) > 0.45 ? 2 : 1)),
      );
    }
  }

  const trianglesByCell: number[][] = Array.from({ length: nCells }, () => []);
  for (let t = 0; t < topology.triangles.length; t += 1) {
    const [i0, i1, i2] = topology.triangles[t];
    trianglesByCell[i0]?.push(t);
    trianglesByCell[i1]?.push(t);
    trianglesByCell[i2]?.push(t);
  }

  const nearestCellId = (unit: Vec3, startCellHint = 0): number => {
    const u = norm3(unit);
    let cur = startCellHint >= 0 && startCellHint < nCells ? startCellHint : 0;
    const startCenter = topology.cells[cur].centerUnit;
    let bestDot =
      startCenter[0] * u[0] + startCenter[1] * u[1] + startCenter[2] * u[2];
    for (let iter = 0; iter < nCells; iter += 1) {
      let improved = false;
      const nbrs = topology.cells[cur].neighbors;
      for (let i = 0; i < nbrs.length; i += 1) {
        const nid = nbrs[i];
        const nc = topology.cells[nid].centerUnit;
        const d = nc[0] * u[0] + nc[1] * u[1] + nc[2] * u[2];
        if (d > bestDot + 1e-12) {
          bestDot = d;
          cur = nid;
          improved = true;
        }
      }
      if (!improved) break;
    }
    return cur;
  };

  const evalTriBary = (
    unitS: Vec3,
    tIdx: number,
  ): { readonly minB: number; readonly b0: number; readonly b1: number; readonly b2: number } => {
    const [i0, i1, i2] = topology.triangles[tIdx];
    const p0 = topology.cells[i0].centerUnit;
    const p1 = topology.cells[i1].centerUnit;
    const p2 = topology.cells[i2].centerUnit;
    const d0 = dot3(unitS, cross3(p1, p2));
    const d1 = dot3(unitS, cross3(p2, p0));
    const d2 = dot3(unitS, cross3(p0, p1));
    const sum = d0 + d1 + d2;
    if (sum <= 1e-12) {
      return { minB: -2, b0: 1, b1: 0, b2: 0 };
    }
    const b0 = d0 / sum;
    const b1 = d1 / sum;
    const b2 = d2 / sum;
    return { minB: Math.min(b0, b1, b2), b0, b1, b2 };
  };

  const findEnclosingTri = (
    unitS: Vec3,
    hintA: number,
    hintB: number,
  ): { readonly triIdx: number; readonly b0: number; readonly b1: number; readonly b2: number } => {
    const visited = new Set<number>();
    let bestTri = 0;
    let bestMin = -2;
    let bestB0 = 1;
    let bestB1 = 0;
    let bestB2 = 0;

    const considerCell = (cid: number) => {
      const list = trianglesByCell[cid];
      if (!list) return;
      for (const tIdx of list) {
        if (visited.has(tIdx)) continue;
        visited.add(tIdx);
        const res = evalTriBary(unitS, tIdx);
        if (res.minB > bestMin) {
          bestMin = res.minB;
          bestTri = tIdx;
          bestB0 = res.b0;
          bestB1 = res.b1;
          bestB2 = res.b2;
        }
      }
    };

    considerCell(hintA);
    considerCell(hintB);

    if (bestMin < -1e-4) {
      const greedyCid = nearestCellId(unitS, hintA >= 0 && hintA < nCells ? hintA : 0);
      considerCell(greedyCid);
      if (bestMin < -1e-4) {
        for (const nbr of topology.cells[greedyCid]?.neighbors ?? []) {
          considerCell(nbr);
        }
      }
    }

    if (bestMin < -1e-4) {
      for (let tIdx = 0; tIdx < topology.triangles.length; tIdx += 1) {
        if (visited.has(tIdx)) continue;
        const res = evalTriBary(unitS, tIdx);
        if (res.minB > bestMin) {
          bestMin = res.minB;
          bestTri = tIdx;
          bestB0 = res.b0;
          bestB1 = res.b1;
          bestB2 = res.b2;
        }
      }
    }

    return { triIdx: bestTri, b0: clamp(bestB0, 0, 1), b1: clamp(bestB1, 0, 1), b2: clamp(bestB2, 0, 1) };
  };

  return {
    nearestCellId,
    sample(unit: Vec3, hintCellA: number, hintCellB: number, altitudeNorm: number): ShaderTerrainSample {
      void altitudeNorm;
      const unitS = norm3(unit);
      const { triIdx, b0, b1, b2 } = findEnclosingTri(unitS, hintCellA, hintCellB);
      const [i0, i1, i2] = topology.triangles[triIdx];

      const sb0Raw = smoothstep(0, 1, b0);
      const sb1Raw = smoothstep(0, 1, b1);
      const sb2Raw = smoothstep(0, 1, b2);
      const sbSum = Math.max(sb0Raw + sb1Raw + sb2Raw, 0.0001);
      const sb0 = sb0Raw / sbSum;
      const sb1 = sb1Raw / sbSum;
      const sb2 = sb2Raw / sbSum;

      const k0 = knownMask[i0] * sb0;
      const k1 = knownMask[i1] * sb1;
      const k2 = knownMask[i2] * sb2;
      const kSum = k0 + k1 + k2;
      if (kSum < 0.35) {
        return { isExplored: false, effectiveElevM: -5000, vertexReliefHeight: 0 };
      }

      const w0 = k0 / kSum;
      const w1 = k1 / kSum;
      const w2 = k2 / kSum;
      const eNorm = elevNorm[i0] * w0 + elevNorm[i1] * w1 + elevNorm[i2] * w2;
      const baseElevM = decodeShaderElevMeters(eNorm);

      const landFrac = isLand[i0] * w0 + isLand[i1] * w1 + isLand[i2] * w2;
      const coastTransition = smoothstep(0.05, 0.32, landFrac) * smoothstep(0.05, 0.32, 1.0 - landFrac);
      const coastBand = (1.0 - smoothstep(40.0, 520.0, Math.abs(baseElevM))) * coastTransition;

      const b0Mask = bioValidMask[i0] * w0;
      const b1Mask = bioValidMask[i1] * w1;
      const b2Mask = bioValidMask[i2] * w2;
      const bSum = b0Mask + b1Mask + b2Mask;
      let strahler = 1.0;
      if (bSum >= 0.01) {
        strahler =
          strahlerOrder[i0] * (b0Mask / bSum) +
          strahlerOrder[i1] * (b1Mask / bSum) +
          strahlerOrder[i2] * (b2Mask / bSum);
      }

      const ux = unitS[0];
      const uy = unitS[1];
      const uz = unitS[2];
      const wx = fbm4(ux * 9.0 + 1.2, uy * 9.0 + 4.7, uz * 9.0 - 2.3) - 0.5;
      const wy = fbm4(ux * 9.0 - 3.8, uy * 9.0 + 0.6, uz * 9.0 + 5.1) - 0.5;
      const wz = fbm4(ux * 9.0 + 2.9, uy * 9.0 - 4.1, uz * 9.0 + 1.4) - 0.5;

      const absLat = Math.abs(Math.asin(clamp(uy, -1.0, 1.0)));
      const coastMacro = (fbm4(ux * 16.0 + wx * 1.45, uy * 16.0 + wy * 1.45, uz * 16.0 + wz * 1.45) - 0.48) * 320.0;
      const coastDetail = (fbm4(ux * 44.0 + wx * 2.1, uy * 44.0 + wy * 2.1, uz * 44.0 + wz * 2.1) - 0.5) * 145.0;
      const fjordMask = smoothstep(0.68, 1.15, absLat);
      const fjordCuts = (ridgedFbm3(ux * 34.0 + wx * 1.8, uy * 34.0 + wy * 1.8, uz * 34.0 + wz * 1.8) - 0.46) * 290.0 * fjordMask;
      const microRidgeM =
        (ridgedFbm3(ux * 28.0 + wx * 0.8, uy * 28.0 + wy * 0.8, uz * 28.0 + wz * 0.8) - 0.35) *
        smoothstep(80.0, 1400.0, Math.max(0.0, baseElevM)) *
        420.0;

      const effectiveElevM = baseElevM + (coastMacro + coastDetail - fjordCuts) * coastBand + microRidgeM;
      const posElev = Math.max(0.0, effectiveElevM);
      const landMask = smoothstep(0.0, 6.0, posElev);
      const coastalBluff = smoothstep(0.0, 65.0, posElev) * 0.0052;
      const hypsoBase = Math.pow(clamp(posElev / 4200.0, 0.0, 1.0), 0.65) * 0.060;
      const rollingHills =
        (fbm4(ux * 34.0 + 3.1, uy * 34.0 - 1.7, uz * 34.0 + 5.3) - 0.42) *
        0.0054 *
        smoothstep(2.0, 35.0, posElev);
      const valleyCut =
        0.0036 *
        smoothstep(1.25, 3.8, strahler) *
        smoothstep(8.0, 85.0, posElev) *
        (1.0 - smoothstep(450.0, 1800.0, posElev));
      const alpineCrags =
        ridgedFbm3(ux * 26.0 - 2.4, uy * 26.0 + 5.1, uz * 26.0 + 1.7) *
        0.019 *
        smoothstep(180.0, 1700.0, posElev);
      const rawLandRelief =
        Math.max(
          0.0003 * landMask,
          coastalBluff + hypsoBase + rollingHills + alpineCrags - valleyCut,
        ) * landMask;
      const vertexReliefHeight = rawLandRelief * smoothstep(0.42, 0.82, kSum);

      return { isExplored: true, effectiveElevM, vertexReliefHeight };
    },
  };
}

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

export function bisectShorelineUnit(
  sampler: ShaderTerrainSurfaceSampler,
  landPt: Vec3,
  otherPt: Vec3,
  hintA: number,
  hintB: number,
  altitudeNorm: number,
  thresholdElevM = SHORELINE_STOP_ELEV_M,
): Vec3 {
  let lo = landPt;
  let hi = otherPt;
  for (let iter = 0; iter < 10; iter += 1) {
    const mid = slerpUnitVec3(lo, hi, 0.5);
    const s = sampler.sample(mid, hintA, hintB, altitudeNorm);
    if (s.isExplored && s.effectiveElevM >= thresholdElevM) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return lo;
}

function safeDryHalfWidth(
  sampler: ShaderTerrainSurfaceSampler,
  curUnit: Vec3,
  prevUnit: Vec3,
  nextUnit: Vec3,
  targetHw: number,
  hintA: number,
  hintB: number,
  altitudeNorm: number,
): number {
  const dir = sub3(nextUnit, prevUnit);
  const sideRaw = cross3(curUnit, dir);
  const sideLen = Math.hypot(sideRaw[0], sideRaw[1], sideRaw[2]);
  if (sideLen < 1e-9) return targetHw;
  const side = scale3(sideRaw, 1 / sideLen);
  let hw = targetHw;
  for (let step = 0; step < 5; step += 1) {
    const leftU = norm3(add3(curUnit, scale3(side, hw)));
    const rightU = norm3(sub3(curUnit, scale3(side, hw)));
    const sL = sampler.sample(leftU, hintA, hintB, altitudeNorm);
    const sR = sampler.sample(rightU, hintA, hintB, altitudeNorm);
    if (sL.isExplored && sL.effectiveElevM >= 0 && sR.isExplored && sR.effectiveElevM >= 0) {
      return hw;
    }
    hw *= 0.5;
  }
  return hw;
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
 * Constructs single tapered, meandering triangle-strip river ribbons strictly inside surveyed land,
 * clipping cleanly at unexplored horizons and land/sea shorelines (`UI-GLB-006`, `UI-MAP-003`, `UI-MAP-004`, `UI-MAP-008`).
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
  existingSampler?: ShaderTerrainSurfaceSampler,
): RiverRibbonBuildResult {
  const knownById = new Map<number, KnownCellDto>();
  for (const c of snapshot.knownCells) {
    knownById.set(c.cellId, c);
  }

  const surfaceSampler = existingSampler ?? createShaderTerrainSurfaceSampler(snapshot, topology);

  const positions: number[] = [];
  const normals: number[] = [];
  const uvs: number[] = [];
  const indices: number[] = [];

  let renderedRiverCount = 0;
  let floodedRiverCount = 0;
  const densityScale = Math.sqrt(Math.max(1, topology.cellCount) / 642);
  const zoomWidthScale = Math.max(0.28, 0.55 + 0.45 * altitudeNorm) / Math.max(1, densityScale * 0.68);
  const flatScale = flatMode ? 0.65 : 1.0;

  for (const cell of snapshot.knownCells) {
    if (cell.tier !== "surveyed") continue;
    if (cell.isOcean || cell.elevationM < 0) continue;
    if (cell.riverDownstreamCellId === null || cell.riverDownstreamCellId === undefined) continue;

    const src = cellMap.get(cell.cellId);
    const dstGeom = cellMap.get(cell.riverDownstreamCellId);
    if (!src || !dstGeom) continue;

    const dstId = cell.riverDownstreamCellId;
    const dstKnown = knownById.get(dstId);
    const dstIsOcean = Boolean(dstKnown && dstKnown.tier !== "unexplored" && (dstKnown.isOcean || dstKnown.elevationM < 0));
    const truncatedAtFog = !dstKnown || dstKnown.tier === "unexplored" || (dstKnown.tier === "rumored" && !dstIsOcean);

    const obsW = weatherByCell.get(cell.cellId);
    const floodStage = obsW?.floodStage01 ?? 0;

    const strahler = cell.strahlerOrder ?? 1;
    const flowNorm = cell.riverFlowNorm01 ?? 0.35;
    if (altitudeNorm > 0.72 && strahler <= 1 && flowNorm < 0.25 && floodStage <= 0.25) {
      continue;
    }

    const segCount = dstIsOcean ? 16 : altitudeNorm < 0.28 ? 12 : 8;
    const { points, widthsNorm } = buildMeanderingSpine(
      src.centerUnit,
      dstGeom.centerUnit,
      cell.cellId,
      dstId,
      truncatedAtFog,
      segCount,
    );

    const baseHalfW =
      (0.0012 + Math.min(4, strahler) * 0.00055 + flowNorm * 0.00085) *
      (1 + floodStage * 0.85) *
      zoomWidthScale;

    let spanUnits: Vec3[] = [];
    let spanHalfWidths: number[] = [];
    let prevUnit: Vec3 | null = null;
    let prevSurf = surfaceSampler.sample(points[0], cell.cellId, dstId, altitudeNorm);
    let emittedAnySpan = false;

    const flushSpan = () => {
      if (spanUnits.length < 2) {
        spanUnits = [];
        spanHalfWidths = [];
        return;
      }
      const projectedPts: THREE.Vector3[] = [];
      const guardedHalfWidths: number[] = [];
      const upNormals: THREE.Vector3[] = [];
      for (let k = 0; k < spanUnits.length; k += 1) {
        const u = spanUnits[k];
        const prevU = spanUnits[Math.max(0, k - 1)];
        const nextU = spanUnits[Math.min(spanUnits.length - 1, k + 1)];
        const s = surfaceSampler.sample(u, cell.cellId, dstId, altitudeNorm);
        const lift = (flatMode ? s.vertexReliefHeight * 0.65 : s.vertexReliefHeight) + (flatMode ? 0.0015 : 0.0008);
        const guardedHw = safeDryHalfWidth(
          surfaceSampler,
          u,
          prevU,
          nextU,
          spanHalfWidths[k] / flatScale,
          cell.cellId,
          dstId,
          altitudeNorm,
        ) * flatScale;
        projectedPts.push(projectPoint(u, lift));
        guardedHalfWidths.push(guardedHw);
        upNormals.push(
          flatMode
            ? new THREE.Vector3(0, 0, 1)
            : new THREE.Vector3(u[0], u[1], u[2]).normalize(),
        );
      }
      appendRibbonStrip(positions, normals, uvs, indices, projectedPts, guardedHalfWidths, upNormals);
      emittedAnySpan = true;
      spanUnits = [];
      spanHalfWidths = [];
    };

    const pushDryPoint = (u: Vec3, wNorm: number, elevM: number, isMouthCap: boolean) => {
      const inlandSwell =
        dstIsOcean && !isMouthCap
          ? 1.0 + 0.12 * smoothstep(35.0, 6.0, elevM) * smoothstep(1.0, 8.0, elevM)
          : 1.0;
      const capScale = isMouthCap ? 0.35 : 1.0;
      spanUnits.push(u);
      spanHalfWidths.push(baseHalfW * wNorm * inlandSwell * capScale * flatScale);
    };

    if (prevSurf.isExplored && prevSurf.effectiveElevM >= SHORELINE_STOP_ELEV_M) {
      pushDryPoint(points[0], widthsNorm[0], prevSurf.effectiveElevM, false);
    }
    prevUnit = points[0];

    for (let i = 1; i < points.length; i += 1) {
      const curUnit = points[i];
      const curSurf = surfaceSampler.sample(curUnit, cell.cellId, dstId, altitudeNorm);
      const prevDry = prevSurf.isExplored && prevSurf.effectiveElevM >= SHORELINE_STOP_ELEV_M;
      const curDry = curSurf.isExplored && curSurf.effectiveElevM >= SHORELINE_STOP_ELEV_M;

      if (prevDry && curDry) {
        pushDryPoint(curUnit, widthsNorm[i], curSurf.effectiveElevM, false);
      } else if (prevDry && !curDry) {
        if (curSurf.isExplored && prevUnit) {
          const shorePt = bisectShorelineUnit(
            surfaceSampler,
            prevUnit,
            curUnit,
            cell.cellId,
            dstId,
            altitudeNorm,
          );
          const shoreSurf = surfaceSampler.sample(shorePt, cell.cellId, dstId, altitudeNorm);
          pushDryPoint(shorePt, widthsNorm[i] * 0.65, shoreSurf.effectiveElevM, true);
        }
        flushSpan();
        if (dstIsOcean) {
          break;
        }
      } else if (!prevDry && curDry) {
        if (prevSurf.isExplored && prevUnit) {
          const entryPt = bisectShorelineUnit(
            surfaceSampler,
            curUnit,
            prevUnit,
            cell.cellId,
            dstId,
            altitudeNorm,
          );
          const entrySurf = surfaceSampler.sample(entryPt, cell.cellId, dstId, altitudeNorm);
          pushDryPoint(entryPt, widthsNorm[i], entrySurf.effectiveElevM, false);
        }
        pushDryPoint(curUnit, widthsNorm[i], curSurf.effectiveElevM, false);
      }

      prevUnit = curUnit;
      prevSurf = curSurf;
    }

    flushSpan();

    if (emittedAnySpan) {
      renderedRiverCount += 1;
      if (floodStage > 0.25) {
        floodedRiverCount += 1;
      }
    }
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
  existingSampler?: ShaderTerrainSurfaceSampler,
): THREE.Mesh | null {
  const knownById = new Map<number, KnownCellDto>();
  for (const c of snapshot.knownCells) {
    knownById.set(c.cellId, c);
  }

  const surfaceSampler = existingSampler ?? createShaderTerrainSurfaceSampler(snapshot, topology);

  const positions: number[] = [];
  const normals: number[] = [];
  const uvs: number[] = [];
  const indices: number[] = [];

  const densityScale = Math.sqrt(Math.max(1, topology.cellCount) / 642);
  const hw = (0.0014 * Math.max(0.32, 0.55 + 0.45 * altitudeNorm)) / Math.max(1, densityScale * 0.68);
  const flatScale = flatMode ? 0.65 : 1.0;

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
    const roadMul = road.infraKey.includes("metalled") || road.infraKey.includes("rail") ? 1.35 : 1.0;
    for (const pt of points) {
      const s = surfaceSampler.sample(pt, road.fromCellId, road.toCellId, altitudeNorm);
      const lift = (flatMode ? s.vertexReliefHeight * 0.65 : s.vertexReliefHeight) + (flatMode ? 0.002 : 0.0012);
      projectedPts.push(projectPoint(pt, lift));
      halfWidths.push(hw * roadMul * flatScale);
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
