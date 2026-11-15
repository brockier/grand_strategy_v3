import * as THREE from "three";
import {
  add3,
  GoldbergCellGeometry,
  GoldbergTopologyData,
  norm3,
  scale3,
  sub3,
  Vec3,
} from "../spine";

export interface GoldbergMeshBuild {
  readonly geometry: THREE.BufferGeometry;
  readonly topology: GoldbergTopologyData;
  readonly vertexCount: number;
  readonly triangleCount: number;
  readonly cellById: ReadonlyMap<number, GoldbergCellGeometry>;
}

export interface ManifoldVerificationReport {
  readonly valid: boolean;
  readonly cellCount: number;
  readonly pentagonCount: number;
  readonly hexagonCount: number;
  readonly unmatchedHalfEdges: number;
  readonly degenerateTriangles: number;
  readonly nonUnitVertices: number;
  readonly eulerCheckDual: number;
}

function sortedCornerTripleKey(
  cellPos: Vec3,
  posA: Vec3,
  posB: Vec3,
): string {
  // Quantize to 1e-6 so bit-equivalent dual Voronoi corners across shared neighbors hash identically.
  const mid = norm3(add3(cellPos, add3(posA, posB)));
  const qx = Math.round(mid[0] * 1e5);
  const qy = Math.round(mid[1] * 1e5);
  const qz = Math.round(mid[2] * 1e5);
  return `${qx}:${qy}:${qz}`;
}

/**
 * Builds an indexed closed manifold dual Goldberg spherical mesh with per-vertex
 * cell ownership, adjacent corner triplets (for smooth coastal/terrain relief),
 * and barycentric edge distance coordinates (`UI-GLB-001`, `UI-GLB-002`).
 */
export function buildGoldbergGlobeGeometry(
  topology: GoldbergTopologyData,
): GoldbergMeshBuild {
  const canonicalCornerPos = new Map<string, Vec3>();
  const cellsByPos = new Map<string, number[]>();

  // Pre-register every dual Voronoi corner canonically so adjacent cells share bit-identical border vertices.
  for (const cell of topology.cells) {
    const d = cell.cornersUnit.length;
    for (let k = 0; k < d; k += 1) {
      const cPos = cell.cornersUnit[k];
      const key = sortedCornerTripleKey(
        cell.centerUnit,
        topology.cells[cell.neighbors[k]].centerUnit,
        topology.cells[cell.neighbors[(k + 1) % d]].centerUnit,
      );
      if (!canonicalCornerPos.has(key)) {
        canonicalCornerPos.set(key, cPos);
        cellsByPos.set(key, [cell.cellId]);
      } else {
        const list = cellsByPos.get(key)!;
        if (!list.includes(cell.cellId)) list.push(cell.cellId);
      }
    }
  }

  const positions: number[] = [];
  const normals: number[] = [];
  const cellIds: number[] = [];
  const cornerTriplets: number[] = [];
  const barycentrics: number[] = [];
  const indices: number[] = [];

  for (const cell of topology.cells) {
    const d = cell.cornersUnit.length;
    const baseVertex = positions.length / 3;

    // Vertex 0 of cell fan: cell center
    const [cx, cy, cz] = cell.centerUnit;
    positions.push(cx, cy, cz);
    normals.push(cx, cy, cz);
    cellIds.push(cell.cellId);
    cornerTriplets.push(cell.cellId, cell.cellId, cell.cellId);
    barycentrics.push(1, 0, 0);

    // Vertices 1..d: dual Voronoi corners around this cell
    for (let k = 0; k < d; k += 1) {
      const nbA = cell.neighbors[k];
      const nbB = cell.neighbors[(k + 1) % d];
      const key = sortedCornerTripleKey(
        cell.centerUnit,
        topology.cells[nbA].centerUnit,
        topology.cells[nbB].centerUnit,
      );
      const pos = canonicalCornerPos.get(key) ?? cell.cornersUnit[k];
      const owners = cellsByPos.get(key) ?? [cell.cellId, nbA, nbB];
      const c0 = owners[0] ?? cell.cellId;
      const c1 = owners[1] ?? nbA;
      const c2 = owners[2] ?? nbB;

      positions.push(pos[0], pos[1], pos[2]);
      normals.push(pos[0], pos[1], pos[2]);
      cellIds.push(cell.cellId);
      cornerTriplets.push(c0, c1, c2);
      barycentrics.push(0, k % 2 === 0 ? 1 : 0, k % 2 === 0 ? 0 : 1);
    }

    // Fan triangles winding CCW outside sphere: (center, corner_k, corner_{k+1})
    for (let k = 0; k < d; k += 1) {
      const vCenter = baseVertex;
      const vCur = baseVertex + 1 + k;
      const vNext = baseVertex + 1 + ((k + 1) % d);
      indices.push(vCenter, vCur, vNext);
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  geometry.setAttribute("aCellId", new THREE.Float32BufferAttribute(cellIds, 1));
  geometry.setAttribute("aCornerCells", new THREE.Float32BufferAttribute(cornerTriplets, 3));
  geometry.setAttribute("aBarycentric", new THREE.Float32BufferAttribute(barycentrics, 3));
  geometry.setIndex(indices);
  geometry.computeBoundingSphere();

  const cellById = new Map<number, GoldbergCellGeometry>();
  for (const c of topology.cells) cellById.set(c.cellId, c);

  return {
    geometry,
    topology,
    vertexCount: positions.length / 3,
    triangleCount: indices.length / 3,
    cellById,
  };
}

/**
 * Verifies closed manifold integrity across all poles, antimeridian seams, and
 * pentagon/hexagon junctions using spatial vertex welding (`UI-GLB-001`).
 */
export function verifyGoldbergManifold(build: GoldbergMeshBuild): ManifoldVerificationReport {
  const posAttr = build.geometry.getAttribute("position") as THREE.BufferAttribute;
  const indexAttr = build.geometry.getIndex();
  if (!indexAttr) {
    return {
      valid: false,
      cellCount: build.topology.cellCount,
      pentagonCount: build.topology.pentagonCount,
      hexagonCount: build.topology.hexagonCount,
      unmatchedHalfEdges: 1,
      degenerateTriangles: 0,
      nonUnitVertices: 0,
      eulerCheckDual: 0,
    };
  }

  // Quantized spatial key for manifold vertex identity
  const vertKey = new Int32Array(posAttr.count);
  const keyMap = new Map<string, number>();
  let nextVertId = 0;
  let nonUnitVertices = 0;

  for (let i = 0; i < posAttr.count; i += 1) {
    const x = posAttr.getX(i);
    const y = posAttr.getY(i);
    const z = posAttr.getZ(i);
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
      nonUnitVertices += 1;
      vertKey[i] = -1;
      continue;
    }
    const r = Math.hypot(x, y, z);
    if (Math.abs(r - 1) > 1e-4) {
      nonUnitVertices += 1;
    }
    const q = `${Math.round(x * 1e5)}:${Math.round(y * 1e5)}:${Math.round(z * 1e5)}`;
    let uid = keyMap.get(q);
    if (uid === undefined) {
      uid = nextVertId;
      nextVertId += 1;
      keyMap.set(q, uid);
    }
    vertKey[i] = uid;
  }

  const directedEdges = new Set<string>();
  const idxArr = indexAttr.array;
  let degenerateTriangles = 0;

  const getVec = (vi: number): Vec3 => [posAttr.getX(vi), posAttr.getY(vi), posAttr.getZ(vi)];

  for (let t = 0; t < idxArr.length; t += 3) {
    const ia = idxArr[t];
    const ib = idxArr[t + 1];
    const ic = idxArr[t + 2];
    const ka = vertKey[ia];
    const kb = vertKey[ib];
    const kc = vertKey[ic];

    if (ka === kb || kb === kc || kc === ka || ka < 0 || kb < 0 || kc < 0) {
      degenerateTriangles += 1;
      continue;
    }

    const va = getVec(ia);
    const vb = getVec(ib);
    const vc = getVec(ic);
    const e1 = sub3(vb, va);
    const e2 = sub3(vc, va);
    const cx = e1[1] * e2[2] - e1[2] * e2[1];
    const cy = e1[2] * e2[0] - e1[0] * e2[2];
    const cz = e1[0] * e2[1] - e1[1] * e2[0];
    if (Math.hypot(cx, cy, cz) < 1e-9) {
      degenerateTriangles += 1;
    }

    directedEdges.add(`${ka}->${kb}`);
    directedEdges.add(`${kb}->${kc}`);
    directedEdges.add(`${kc}->${ka}`);
  }

  let unmatchedHalfEdges = 0;
  for (const edge of directedEdges) {
    const sep = edge.indexOf("->");
    const a = edge.slice(0, sep);
    const b = edge.slice(sep + 2);
    if (!directedEdges.has(`${b}->${a}`)) {
      unmatchedHalfEdges += 1;
    }
  }

  const cornerCount = build.topology.cells.reduce((acc, c) => acc + c.degree, 0) / 3;
  const dualEdges = build.topology.edges.length;
  const eulerCheckDual = cornerCount - dualEdges + build.topology.cellCount;

  return {
    valid:
      unmatchedHalfEdges === 0 &&
      degenerateTriangles === 0 &&
      nonUnitVertices === 0 &&
      build.topology.pentagonCount === 12 &&
      eulerCheckDual === 2,
    cellCount: build.topology.cellCount,
    pentagonCount: build.topology.pentagonCount,
    hexagonCount: build.topology.hexagonCount,
    unmatchedHalfEdges,
    degenerateTriangles,
    nonUnitVertices,
    eulerCheckDual,
  };
}

export function unitToLatLonDegrees(unit: Vec3): { readonly latDeg: number; readonly lonDeg: number } {
  const latDeg = (Math.asin(Math.max(-1, Math.min(1, unit[1]))) * 180) / Math.PI;
  const lonDeg = (Math.atan2(unit[0], unit[2]) * 180) / Math.PI;
  return { latDeg, lonDeg };
}

export function geodesicPointsBetween(
  a: Vec3,
  b: Vec3,
  segments: number,
  radius: number,
): THREE.Vector3[] {
  const pts: THREE.Vector3[] = [];
  const count = Math.max(2, segments);
  const dot = Math.max(-1, Math.min(1, a[0] * b[0] + a[1] * b[1] + a[2] * b[2]));
  const omega = Math.acos(dot);
  for (let i = 0; i <= count; i += 1) {
    const t = i / count;
    let v: Vec3;
    if (omega < 1e-4) {
      v = norm3(add3(scale3(a, 1 - t), scale3(b, t)));
    } else {
      const sinO = Math.sin(omega);
      v = norm3(add3(scale3(a, Math.sin((1 - t) * omega) / sinO), scale3(b, Math.sin(t * omega) / sinO)));
    }
    pts.push(new THREE.Vector3(v[0] * radius, v[1] * radius, v[2] * radius));
  }
  return pts;
}
