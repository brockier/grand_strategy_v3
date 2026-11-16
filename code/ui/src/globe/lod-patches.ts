import * as THREE from "three";
import {
  add3,
  GoldbergTopologyData,
  greatCircleRadians,
  norm3,
  Vec3,
} from "../spine";

export type LodLevel = 0 | 1 | 2 | 3;

const SUBDIV_PER_LOD: readonly [number, number, number, number] = [1, 2, 4, 8];

interface PatchDescriptor {
  readonly sectorId: number;
  readonly centerUnit: Vec3;
  readonly angularRadiusRad: number;
  readonly triangles: readonly (readonly [number, number, number])[];
  readonly boundaryEdges: ReadonlySet<string>;
}

interface PatchRuntime {
  readonly descriptor: PatchDescriptor;
  readonly mesh: THREE.Mesh;
  readonly geomCache: Partial<Record<LodLevel, THREE.BufferGeometry>>;
  currentLod: LodLevel;
}

function edgeKey(a: number, b: number): string {
  return a < b ? `${a}:${b}` : `${b}:${a}`;
}

function buildIcosaSectorCenters(): readonly Vec3[] {
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
  ].map(norm3);

  const faces: readonly [number, number, number][] = [
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

  return faces.map(([a, b, c]) => norm3(add3(rawVerts[a], add3(rawVerts[b], rawVerts[c]))));
}

function partitionTopologyPatches(topology: GoldbergTopologyData): PatchDescriptor[] {
  const sectorCenters = buildIcosaSectorCenters();
  const buckets: [number, number, number][][] = Array.from({ length: 20 }, () => []);

  for (const [i0, i1, i2] of topology.triangles) {
    const p0 = topology.cells[i0].centerUnit;
    const p1 = topology.cells[i1].centerUnit;
    const p2 = topology.cells[i2].centerUnit;
    const centroid = norm3(add3(p0, add3(p1, p2)));
    let bestSector = 0;
    let bestDot = -2;
    for (let s = 0; s < sectorCenters.length; s += 1) {
      const sc = sectorCenters[s];
      const d = centroid[0] * sc[0] + centroid[1] * sc[1] + centroid[2] * sc[2];
      if (d > bestDot) {
        bestDot = d;
        bestSector = s;
      }
    }
    buckets[bestSector].push([i0, i1, i2]);
  }

  const patches: PatchDescriptor[] = [];
  for (let s = 0; s < buckets.length; s += 1) {
    const tris = buckets[s];
    if (tris.length === 0) continue;

    let sumX = 0;
    let sumY = 0;
    let sumZ = 0;
    const edgeCounts = new Map<string, number>();

    for (const [a, b, c] of tris) {
      const pa = topology.cells[a].centerUnit;
      const pb = topology.cells[b].centerUnit;
      const pc = topology.cells[c].centerUnit;
      sumX += pa[0] + pb[0] + pc[0];
      sumY += pa[1] + pb[1] + pc[1];
      sumZ += pa[2] + pb[2] + pc[2];

      for (const [eA, eB] of [
        [a, b],
        [b, c],
        [c, a],
      ] as const) {
        const k = edgeKey(eA, eB);
        edgeCounts.set(k, (edgeCounts.get(k) ?? 0) + 1);
      }
    }

    const centerUnit = norm3([sumX, sumY, sumZ]);
    let maxAngle = 0;
    for (const [a, b, c] of tris) {
      for (const vid of [a, b, c]) {
        const ang = greatCircleRadians(centerUnit, topology.cells[vid].centerUnit);
        if (ang > maxAngle) maxAngle = ang;
      }
    }

    const boundaryEdges = new Set<string>();
    for (const [k, count] of edgeCounts) {
      if (count === 1) {
        boundaryEdges.add(k);
      }
    }

    patches.push({
      sectorId: s,
      centerUnit,
      angularRadiusRad: maxAngle,
      triangles: tris,
      boundaryEdges,
    });
  }
  return patches;
}

function buildPatchGeometry(
  topology: GoldbergTopologyData,
  patch: PatchDescriptor,
  lod: LodLevel,
): THREE.BufferGeometry {
  const subdiv = SUBDIV_PER_LOD[lod];
  const positions: number[] = [];
  const normals: number[] = [];
  const cellIds: number[] = [];
  const cornerTriplets: number[] = [];
  const barycentrics: number[] = [];
  const triCenters: number[] = [];
  const skirtMasks: number[] = [];
  const indices: number[] = [];

  const pushVert = (
    p: Vec3,
    cid: number,
    i0: number,
    i1: number,
    i2: number,
    u: number,
    v: number,
    w: number,
    center: Vec3,
    skirtMask: number,
  ): number => {
    const idx = positions.length / 3;
    positions.push(p[0], p[1], p[2]);
    normals.push(p[0], p[1], p[2]);
    cellIds.push(cid);
    cornerTriplets.push(i0, i1, i2);
    barycentrics.push(u, v, w);
    triCenters.push(center[0], center[1], center[2]);
    skirtMasks.push(skirtMask);
    return idx;
  };

  for (const [i0, i1, i2] of patch.triangles) {
    const p0 = topology.cells[i0].centerUnit;
    const p1 = topology.cells[i1].centerUnit;
    const p2 = topology.cells[i2].centerUnit;
    const center = norm3(add3(p0, add3(p1, p2)));

    if (subdiv === 1) {
      const v0 = pushVert(p0, i0, i0, i1, i2, 1, 0, 0, center, 0);
      const v1 = pushVert(p1, i1, i0, i1, i2, 0, 1, 0, center, 0);
      const v2 = pushVert(p2, i2, i0, i1, i2, 0, 0, 1, center, 0);
      indices.push(v0, v1, v2);
      continue;
    }

    const grid: number[][] = [];
    const gridPos: Vec3[][] = [];
    const gridBary: Vec3[][] = [];
    const gridCid: number[][] = [];

    for (let r = 0; r <= subdiv; r += 1) {
      const rowIds: number[] = [];
      const rowPos: Vec3[] = [];
      const rowBary: Vec3[] = [];
      const rowCid: number[] = [];
      for (let c = 0; c <= subdiv - r; c += 1) {
        const u = (subdiv - r - c) / subdiv;
        const v = r / subdiv;
        const w = c / subdiv;
        const p = norm3([
          p0[0] * u + p1[0] * v + p2[0] * w,
          p0[1] * u + p1[1] * v + p2[1] * w,
          p0[2] * u + p1[2] * v + p2[2] * w,
        ]);
        const cid = u >= v && u >= w ? i0 : v >= w ? i1 : i2;
        const vi = pushVert(p, cid, i0, i1, i2, u, v, w, center, 0);
        rowIds.push(vi);
        rowPos.push(p);
        rowBary.push([u, v, w]);
        rowCid.push(cid);
      }
      grid.push(rowIds);
      gridPos.push(rowPos);
      gridBary.push(rowBary);
      gridCid.push(rowCid);
    }

    for (let r = 0; r < subdiv; r += 1) {
      const cur = grid[r];
      const nxt = grid[r + 1];
      for (let c = 0; c < nxt.length; c += 1) {
        indices.push(cur[c], nxt[c], cur[c + 1]);
        if (c + 1 < nxt.length) {
          indices.push(cur[c + 1], nxt[c], nxt[c + 1]);
        }
      }
    }

    // Add patch perimeter skirts so adjacent mixed-LOD sectors never expose inter-patch cracks.
    const emitBoundarySkirt = (
      edgeCoords: readonly (readonly [number, number])[],
    ) => {
      for (let k = 0; k + 1 < edgeCoords.length; k += 1) {
        const [rA, cA] = edgeCoords[k];
        const [rB, cB] = edgeCoords[k + 1];
        const topA = grid[rA][cA];
        const topB = grid[rB][cB];
        const bA = gridBary[rA][cA];
        const bB = gridBary[rB][cB];
        const skA = pushVert(gridPos[rA][cA], gridCid[rA][cA], i0, i1, i2, bA[0], bA[1], bA[2], center, 1);
        const skB = pushVert(gridPos[rB][cB], gridCid[rB][cB], i0, i1, i2, bB[0], bB[1], bB[2], center, 1);
        indices.push(topA, skA, topB, topB, skA, skB);
      }
    };

    if (patch.boundaryEdges.has(edgeKey(i0, i1))) {
      const coords: [number, number][] = [];
      for (let r = 0; r <= subdiv; r += 1) coords.push([r, 0]);
      emitBoundarySkirt(coords);
    }
    if (patch.boundaryEdges.has(edgeKey(i1, i2))) {
      const coords: [number, number][] = [];
      for (let r = subdiv; r >= 0; r -= 1) coords.push([r, subdiv - r]);
      emitBoundarySkirt(coords);
    }
    if (patch.boundaryEdges.has(edgeKey(i2, i0))) {
      const coords: [number, number][] = [];
      for (let c = subdiv; c >= 0; c -= 1) coords.push([0, c]);
      emitBoundarySkirt(coords);
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  geometry.setAttribute("aCellId", new THREE.Float32BufferAttribute(cellIds, 1));
  geometry.setAttribute("aCornerCells", new THREE.Float32BufferAttribute(cornerTriplets, 3));
  geometry.setAttribute("aBarycentric", new THREE.Float32BufferAttribute(barycentrics, 3));
  geometry.setAttribute("aTriCenter", new THREE.Float32BufferAttribute(triCenters, 3));
  geometry.setAttribute("aSkirtMask", new THREE.Float32BufferAttribute(skirtMasks, 1));
  geometry.setIndex(indices);
  geometry.computeBoundingSphere();
  return geometry;
}

/**
 * Spatial spherical sector patch manager selecting `LOD 0..3` subdivision tiers dynamically
 * around `focusUnit` while horizon-culling back-hemisphere patches (`UI-GLB-006`).
 */
export class GlobePatchLodManager {
  readonly group = new THREE.Group();
  private topology: GoldbergTopologyData;
  private material: THREE.Material;
  private patches: PatchRuntime[] = [];

  constructor(topology: GoldbergTopologyData, material: THREE.Material) {
    this.topology = topology;
    this.material = material;
    this.rebuildTopology(topology);
  }

  rebuildTopology(topology: GoldbergTopologyData): void {
    this.disposeGeometries();
    this.topology = topology;
    this.patches = partitionTopologyPatches(topology).map((descriptor) => {
      const geom0 = buildPatchGeometry(topology, descriptor, 0);
      const mesh = new THREE.Mesh(geom0, this.material);
      mesh.frustumCulled = false;
      this.group.add(mesh);
      return {
        descriptor,
        mesh,
        geomCache: { 0: geom0 },
        currentLod: 0,
      };
    });
  }

  updateForCamera(focusUnit: Vec3, altitudeNorm: number, projectionMorph = 0): void {
    const f = norm3(focusUnit);
    const flatVisible = projectionMorph >= 0.01;
    // Allow generous back-hemisphere margin for grazing limb twilight & atmospheric glow.
    const cullCosLimit = -0.35 - altitudeNorm * 0.15;

    for (const p of this.patches) {
      const dotFocus =
        p.descriptor.centerUnit[0] * f[0] +
        p.descriptor.centerUnit[1] * f[1] +
        p.descriptor.centerUnit[2] * f[2];
      const ang = Math.acos(Math.max(-1, Math.min(1, dotFocus)));
      const rimAng = Math.max(0, ang - p.descriptor.angularRadiusRad);

      if (!flatVisible && Math.cos(rimAng) < cullCosLimit) {
        p.mesh.visible = false;
        continue;
      }
      p.mesh.visible = true;

      let targetLod: LodLevel = 0;
      if (flatVisible) {
        if (altitudeNorm < 0.18 && rimAng < 0.28) targetLod = 3;
        else if (altitudeNorm < 0.36 && rimAng < 0.55) targetLod = 2;
        else if (altitudeNorm < 0.70) targetLod = 1;
        else targetLod = 0;
      } else if (altitudeNorm < 0.16) {
        if (rimAng < 0.24) targetLod = 3;
        else if (rimAng < 0.56) targetLod = 2;
        else if (rimAng < 0.95) targetLod = 1;
        else targetLod = 0;
      } else if (altitudeNorm < 0.34) {
        if (rimAng < 0.42) targetLod = 2;
        else if (rimAng < 0.92) targetLod = 1;
        else targetLod = 0;
      } else if (altitudeNorm < 0.65 && rimAng < 0.85) {
        targetLod = 1;
      }

      if (targetLod !== p.currentLod) {
        let geom = p.geomCache[targetLod];
        if (!geom) {
          geom = buildPatchGeometry(this.topology, p.descriptor, targetLod);
          p.geomCache[targetLod] = geom;
        }
        p.mesh.geometry = geom;
        p.currentLod = targetLod;
      }
    }
  }

  getLodCounts(): readonly [number, number, number, number] {
    const counts: [number, number, number, number] = [0, 0, 0, 0];
    for (const p of this.patches) {
      if (!p.mesh.visible) continue;
      counts[p.currentLod] += 1;
    }
    return counts;
  }

  getTotalVisibleTriangles(): number {
    let total = 0;
    for (const p of this.patches) {
      if (!p.mesh.visible) continue;
      const idx = p.mesh.geometry.getIndex();
      total += idx ? idx.count / 3 : 0;
    }
    return total;
  }

  private disposeGeometries(): void {
    for (const p of this.patches) {
      this.group.remove(p.mesh);
      for (const lod of [0, 1, 2, 3] as const) {
        p.geomCache[lod]?.dispose();
      }
    }
    this.patches = [];
  }

  dispose(): void {
    this.disposeGeometries();
  }
}
