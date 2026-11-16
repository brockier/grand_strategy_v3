/**
 * Pure mathematical Goldberg polyhedron `GP(m, 0)` spherical topology generator
 * matching `sim-planet/src/topology.rs` bit-for-algorithm (`UI-GLB-001.1`).
 */

export type Vec3 = readonly [number, number, number];

export interface GoldbergCellGeometry {
  readonly cellId: number;
  readonly centerUnit: Vec3;
  readonly degree: number; // 5 for the 12 pentagons, 6 for all hexagons
  readonly cornersUnit: readonly Vec3[];
  readonly neighbors: readonly number[];
  readonly icosaChunkId: number;
}

export interface GoldbergEdge {
  readonly edgeId: number;
  readonly cellA: number;
  readonly cellB: number;
}

export interface GoldbergTopologyData {
  readonly subdivision: number;
  readonly frequency: number;
  readonly cellCount: number;
  readonly pentagonCount: number;
  readonly hexagonCount: number;
  readonly cells: readonly GoldbergCellGeometry[];
  readonly edges: readonly GoldbergEdge[];
  readonly edgeByPair: ReadonlyMap<string, number>;
  readonly triangles: readonly (readonly [number, number, number])[];
}

export function subdivisionToFrequency(subdivision: number): number {
  const safe = Math.max(1, Math.floor(subdivision));
  if (safe >= 8) return safe;
  return 1 << Math.min(safe, 6);
}

export function dot3(a: Vec3, b: Vec3): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

export function add3(a: Vec3, b: Vec3): Vec3 {
  return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}

export function sub3(a: Vec3, b: Vec3): Vec3 {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

export function scale3(a: Vec3, s: number): Vec3 {
  return [a[0] * s, a[1] * s, a[2] * s];
}

export function cross3(a: Vec3, b: Vec3): Vec3 {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}

export function norm3(v: Vec3): Vec3 {
  const len = Math.max(Math.hypot(v[0], v[1], v[2]), 1e-15);
  return [v[0] / len, v[1] / len, v[2] / len];
}

export function tangentBasis(p: Vec3): readonly [Vec3, Vec3] {
  const pole: Vec3 = Math.abs(p[1]) < 0.99 ? [0, 1, 0] : [1, 0, 0];
  const east = norm3(cross3(pole, p));
  const north = norm3(cross3(p, east));
  return [east, north];
}

export function greatCircleRadians(a: Vec3, b: Vec3): number {
  const d = Math.max(-1, Math.min(1, dot3(a, b)));
  return Math.acos(d);
}

export function slerpUnitVec3(a: Vec3, b: Vec3, t: number): Vec3 {
  const clampedT = Math.max(0, Math.min(1, t));
  const cosTheta = Math.max(-1, Math.min(1, dot3(a, b)));
  if (cosTheta > 0.999995) {
    return norm3(add3(scale3(a, 1 - clampedT), scale3(b, clampedT)));
  }
  if (cosTheta < -0.999995) {
    const orthoPole: Vec3 = Math.abs(a[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
    const axis = norm3(cross3(a, orthoPole));
    const angle = Math.PI * clampedT;
    return norm3(add3(scale3(a, Math.cos(angle)), scale3(axis, Math.sin(angle))));
  }
  const theta = Math.acos(cosTheta);
  const sinTheta = Math.sin(theta);
  const wA = Math.sin((1 - clampedT) * theta) / sinTheta;
  const wB = Math.sin(clampedT * theta) / sinTheta;
  return norm3(add3(scale3(a, wA), scale3(b, wB)));
}

function canonicalIcosahedron(): { readonly verts: readonly Vec3[]; readonly faces: readonly [number, number, number][] } {
  const phi = (1 + Math.sqrt(5)) * 0.5;
  const rawVerts: Vec3[] = [
    [-1, phi, 0],
    [1, phi, 0],
    [-1, -phi, 0],
    [1, -phi, 0],
    [0, -1, phi],
    [0, 1, phi],
    [0, -1, -phi],
    [0, 1, -phi],
    [phi, 0, -1],
    [phi, 0, 1],
    [-phi, 0, -1],
    [-phi, 0, 1],
  ];
  const verts = rawVerts.map(norm3);
  const rawFaces: [number, number, number][] = [
    [0, 11, 5],
    [0, 5, 1],
    [0, 1, 7],
    [0, 7, 10],
    [0, 10, 11],
    [1, 5, 9],
    [5, 11, 4],
    [11, 10, 2],
    [10, 7, 6],
    [7, 1, 8],
    [3, 9, 4],
    [3, 4, 2],
    [3, 2, 6],
    [3, 6, 8],
    [3, 8, 9],
    [4, 9, 5],
    [2, 4, 11],
    [6, 2, 10],
    [8, 6, 7],
    [9, 8, 1],
  ];

  const faces: [number, number, number][] = rawFaces.map(([a, b, c]) => {
    const centroid = add3(verts[a], add3(verts[b], verts[c]));
    const normal = cross3(sub3(verts[b], verts[a]), sub3(verts[c], verts[a]));
    return dot3(normal, centroid) < 0 ? [a, c, b] : [a, b, c];
  });
  return { verts, faces };
}

/**
 * Generates exact dual Goldberg cells matching `sim-planet`'s vertex indexing and chunk assignment.
 */
export function generateGoldbergTopology(subdivision: number): GoldbergTopologyData {
  const m = subdivisionToFrequency(subdivision);
  const { verts: icoV, faces: icoF } = canonicalIcosahedron();
  const expectedCells = 10 * m * m + 2;

  const positions: Vec3[] = [...icoV];
  const edgeVertMap = new Map<string, number>();
  const rawNeighbors: Set<number>[] = Array.from({ length: expectedCells }, () => new Set<number>());
  const chunkAssignment: number[] = Array.from({ length: expectedCells }, () => 0);
  const triangles: [number, number, number][] = [];

  const edgeVertex = (a: number, b: number, step: number): number => {
    const [lo, hi, t] = a < b ? [a, b, step] : [b, a, m - step];
    const key = `${lo}:${hi}:${t}`;
    const found = edgeVertMap.get(key);
    if (found !== undefined) return found;
    const alpha = t / m;
    const p = norm3(add3(scale3(icoV[lo], 1 - alpha), scale3(icoV[hi], alpha)));
    const id = positions.length;
    positions.push(p);
    edgeVertMap.set(key, id);
    return id;
  };

  for (let faceIdx = 0; faceIdx < icoF.length; faceIdx += 1) {
    const [vA, vB, vC] = icoF[faceIdx];
    const grid: number[][] = [];
    for (let row = 0; row <= m; row += 1) {
      const rowIds: number[] = [];
      for (let col = 0; col <= m - row; col += 1) {
        const i = m - row - col;
        const j = row;
        const k = col;
        let vid: number;
        if (i === m) {
          vid = vA;
        } else if (j === m) {
          vid = vB;
        } else if (k === m) {
          vid = vC;
        } else if (k === 0) {
          vid = edgeVertex(vA, vB, j);
        } else if (j === 0) {
          vid = edgeVertex(vA, vC, k);
        } else if (i === 0) {
          vid = edgeVertex(vB, vC, k);
        } else {
          const inv = 1 / m;
          const p = norm3(
            add3(
              scale3(icoV[vA], i * inv),
              add3(scale3(icoV[vB], j * inv), scale3(icoV[vC], k * inv)),
            ),
          );
          vid = positions.length;
          positions.push(p);
        }
        chunkAssignment[vid] = faceIdx;
        rowIds.push(vid);
      }
      grid.push(rowIds);
    }

    const linkTri = (a: number, b: number, c: number) => {
      rawNeighbors[a].add(b);
      rawNeighbors[a].add(c);
      rawNeighbors[b].add(a);
      rawNeighbors[b].add(c);
      rawNeighbors[c].add(a);
      rawNeighbors[c].add(b);
      const pa = positions[a];
      const pb = positions[b];
      const pc = positions[c];
      const orient =
        pa[0] * (pb[1] * pc[2] - pb[2] * pc[1]) +
        pa[1] * (pb[2] * pc[0] - pb[0] * pc[2]) +
        pa[2] * (pb[0] * pc[1] - pb[1] * pc[0]);
      triangles.push(orient < 0 ? [a, c, b] : [a, b, c]);
    };

    for (let r = 0; r < m; r += 1) {
      const cur = grid[r];
      const nxt = grid[r + 1];
      for (let c = 0; c < nxt.length; c += 1) {
        linkTri(cur[c], nxt[c], cur[c + 1]);
        if (c + 1 < nxt.length) {
          linkTri(cur[c + 1], nxt[c], nxt[c + 1]);
        }
      }
    }
  }

  const cells: GoldbergCellGeometry[] = [];
  const edges: GoldbergEdge[] = [];
  const edgeByPair = new Map<string, number>();
  let pentagonCount = 0;
  let hexagonCount = 0;

  for (let idx = 0; idx < positions.length; idx += 1) {
    const p = positions[idx];
    const [east, north] = tangentBasis(p);
    const sortedNbrs = Array.from(rawNeighbors[idx])
      .map((nb) => {
        const diff = sub3(positions[nb], p);
        const angle = Math.atan2(dot3(diff, north), dot3(diff, east));
        return { angle, nb };
      })
      .sort((a, b) => a.angle - b.angle)
      .map((entry) => entry.nb);

    const deg = sortedNbrs.length;
    if (deg === 5) pentagonCount += 1;
    else if (deg === 6) hexagonCount += 1;

    const cornersUnit: Vec3[] = [];
    for (let k = 0; k < deg; k += 1) {
      const nbA = sortedNbrs[k];
      const nbB = sortedNbrs[(k + 1) % deg];
      cornersUnit.push(norm3(add3(p, add3(positions[nbA], positions[nbB]))));

      const lo = Math.min(idx, nbA);
      const hi = Math.max(idx, nbA);
      const pairKey = `${lo}:${hi}`;
      if (!edgeByPair.has(pairKey)) {
        const edgeId = edges.length;
        edgeByPair.set(pairKey, edgeId);
        edges.push({ edgeId, cellA: lo, cellB: hi });
      }
    }

    cells.push({
      cellId: idx,
      centerUnit: p,
      degree: deg,
      cornersUnit,
      neighbors: sortedNbrs,
      icosaChunkId: chunkAssignment[idx] ?? 0,
    });
  }

  return {
    subdivision,
    frequency: m,
    cellCount: positions.length,
    pentagonCount,
    hexagonCount,
    cells,
    edges,
    edgeByPair,
    triangles,
  };
}

export const EQUAL_AREA_SCALE = 0.72;
export const MERCATOR_SCALE = 1.35 / Math.PI;
const MERCATOR_MAX_LAT = 1.4835298641951802; // 85 degrees in radians

function projectionTangentBasis(focusUnit: Vec3): {
  readonly east: Vec3;
  readonly north: Vec3;
  readonly center: Vec3;
} {
  const center = norm3(focusUnit);
  const upRef: Vec3 = Math.abs(center[1]) < 0.99 ? [0, 1, 0] : [0, 0, -1];
  const east = norm3(cross3(upRef, center));
  const north = norm3(cross3(center, east));
  return { east, north, center };
}

function wrapPi(angle: number): number {
  return angle - Math.PI * 2 * Math.round(angle / (Math.PI * 2));
}

/**
 * Projects a unit sphere vector onto a 2D map plane in Oblique Lambert Azimuthal Equal-Area
 * (matching ESA's ETRS89-LAEA continental perspective) or centered cylindrical Mercator (`UI-GLB-007`).
 */
export function projectUnitToFlat2D(
  unit: Vec3,
  mode: "equal_area" | "mercator",
  focusUnit: Vec3,
): { readonly x: number; readonly y: number; readonly valid: boolean } {
  const u = norm3(unit);
  if (mode === "equal_area") {
    const { east, north, center } = projectionTangentBasis(focusUnit);
    const tx = dot3(u, east);
    const ty = dot3(u, north);
    const tz = dot3(u, center);
    if (1 + tz <= 1e-5) {
      return { x: EQUAL_AREA_SCALE * 2, y: 0, valid: false };
    }
    const k = Math.sqrt(2 / (1 + tz));
    return {
      x: EQUAL_AREA_SCALE * k * tx,
      y: EQUAL_AREA_SCALE * k * ty,
      valid: true,
    };
  }

  const focusNorm = norm3(focusUnit);
  const lon0 = Math.atan2(focusNorm[0], focusNorm[2]);
  const lon = Math.atan2(u[0], u[2]);
  const dLon = wrapPi(lon - lon0);
  const rawLat = Math.asin(Math.max(-1, Math.min(1, u[1])));
  const clampedLat = Math.max(-MERCATOR_MAX_LAT, Math.min(MERCATOR_MAX_LAT, rawLat));
  const y = MERCATOR_SCALE * Math.log(Math.tan(Math.PI * 0.25 + clampedLat * 0.5));
  return {
    x: MERCATOR_SCALE * dLon,
    y,
    valid: Math.abs(rawLat) <= MERCATOR_MAX_LAT + 1e-4,
  };
}

/**
 * Inverse analytical unprojection from 2D Equal-Area or Mercator plane back onto the unit sphere (`UI-GLB-007`).
 */
export function unprojectFlat2DToUnit(
  x: number,
  y: number,
  mode: "equal_area" | "mercator",
  focusUnit: Vec3,
): Vec3 | null {
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  if (mode === "equal_area") {
    const ux = x / EQUAL_AREA_SCALE;
    const uy = y / EQUAL_AREA_SCALE;
    const rhoSq = ux * ux + uy * uy;
    if (rhoSq > 4.0001) return null;
    const tz = 1 - 0.5 * rhoSq;
    const kInv = Math.sqrt(Math.max(0, 1 - 0.25 * rhoSq));
    const tx = ux * kInv;
    const ty = uy * kInv;
    const { east, north, center } = projectionTangentBasis(focusUnit);
    return norm3(
      add3(scale3(east, tx), add3(scale3(north, ty), scale3(center, tz))),
    );
  }

  const maxX = MERCATOR_SCALE * Math.PI + 1e-4;
  const maxY = MERCATOR_SCALE * Math.log(Math.tan(Math.PI * 0.25 + MERCATOR_MAX_LAT * 0.5)) + 0.05;
  if (Math.abs(x) > maxX || Math.abs(y) > maxY) return null;
  const focusNorm = norm3(focusUnit);
  const lon0 = Math.atan2(focusNorm[0], focusNorm[2]);
  const lon = lon0 + x / MERCATOR_SCALE;
  const lat = Math.atan(Math.sinh(y / MERCATOR_SCALE));
  const cosLat = Math.cos(lat);
  return norm3([cosLat * Math.sin(lon), Math.sin(lat), cosLat * Math.cos(lon)]);
}
