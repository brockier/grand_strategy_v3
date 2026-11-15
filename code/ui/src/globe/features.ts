import * as THREE from "three";
import {
  DiegeticSnapshot,
  GlobePalette,
  GoldbergCellGeometry,
  GoldbergTopologyData,
  RgbTuple,
} from "../spine";
import { geodesicPointsBetween } from "./mesh";

function rgbToThreeColor(rgb: RgbTuple): THREE.Color {
  return new THREE.Color(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
}

/**
 * Manages 3D surveyed settlements, rivers, road splines, HQ ring, and tracked convoys
 * rendered slightly above the Goldberg spherical shell (`UI-GLB-001`, `UI-GLB-005`).
 */
export class GlobeFeatureOverlayGroup {
  readonly group = new THREE.Group();
  private riverMat: THREE.LineBasicMaterial;
  private roadMat: THREE.LineBasicMaterial;
  private settlementMat: THREE.MeshBasicMaterial;
  private hqRingMat: THREE.MeshBasicMaterial;
  private convoyMat: THREE.MeshBasicMaterial;

  constructor(palette?: GlobePalette) {
    this.riverMat = new THREE.LineBasicMaterial({ color: rgbToThreeColor(palette?.riverWaterRgb ?? [54, 108, 133]) });
    this.roadMat = new THREE.LineBasicMaterial({ color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]) });
    this.settlementMat = new THREE.MeshBasicMaterial({ color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]) });
    this.hqRingMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      side: THREE.DoubleSide,
    });
    this.convoyMat = new THREE.MeshBasicMaterial({ color: rgbToThreeColor(palette?.peakSnowRgb ?? [244, 239, 228]) });
  }

  setPalette(palette: GlobePalette): void {
    this.riverMat.color.copy(rgbToThreeColor(palette.riverWaterRgb));
    this.roadMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.settlementMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.hqRingMat.color.copy(rgbToThreeColor(palette.compassRoseRgb));
    this.convoyMat.color.copy(rgbToThreeColor(palette.peakSnowRgb));
  }

  rebuild(snapshot: DiegeticSnapshot, topology: GoldbergTopologyData): void {
    while (this.group.children.length > 0) {
      const child = this.group.children.pop();
      if (!child) continue;
      if ((child as THREE.Mesh).geometry) {
        (child as THREE.Mesh).geometry.dispose();
      }
    }

    const cellMap = new Map<number, GoldbergCellGeometry>();
    for (const c of topology.cells) cellMap.set(c.cellId, c);

    // 1. Surveyed rivers only (`UI-GLB-005`: never render rivers for Unexplored or Stale cells)
    const riverSegs: THREE.Vector3[] = [];
    for (const cell of snapshot.knownCells) {
      if (cell.tier !== "surveyed") continue;
      if (cell.riverDownstreamCellId === null || cell.riverDownstreamCellId === undefined) continue;
      const src = cellMap.get(cell.cellId);
      const dst = cellMap.get(cell.riverDownstreamCellId);
      if (!src || !dst) continue;
      const arc = geodesicPointsBetween(src.centerUnit, dst.centerUnit, 6, 1.006);
      for (let i = 0; i < arc.length - 1; i += 1) {
        riverSegs.push(arc[i], arc[i + 1]);
      }
    }
    if (riverSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(riverSegs);
      this.group.add(new THREE.LineSegments(geom, this.riverMat));
    }

    // 2. Surveyed road edges
    const roadSegs: THREE.Vector3[] = [];
    for (const road of snapshot.surveyedRoads ?? []) {
      const src = cellMap.get(road.fromCellId);
      const dst = cellMap.get(road.toCellId);
      if (!src || !dst) continue;
      const arc = geodesicPointsBetween(src.centerUnit, dst.centerUnit, 8, 1.009);
      for (let i = 0; i < arc.length - 1; i += 1) {
        roadSegs.push(arc[i], arc[i + 1]);
      }
    }
    if (roadSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(roadSegs);
      this.group.add(new THREE.LineSegments(geom, this.roadMat));
    }

    // 3. Surveyed settlements
    for (const settlement of snapshot.knownSettlements ?? []) {
      const cell = cellMap.get(settlement.cellId);
      if (!cell) continue;
      const markerGeom = new THREE.SphereGeometry(0.016, 10, 8);
      const mesh = new THREE.Mesh(markerGeom, this.settlementMat);
      mesh.position.set(
        cell.centerUnit[0] * 1.015,
        cell.centerUnit[1] * 1.015,
        cell.centerUnit[2] * 1.015,
      );
      this.group.add(mesh);
    }

    // 4. Live HQ beacon ring
    const hqCell = cellMap.get(snapshot.hqPresence.cellId);
    if (hqCell) {
      const ringGeom = new THREE.RingGeometry(0.022, 0.032, 24);
      const ring = new THREE.Mesh(ringGeom, this.hqRingMat);
      const n = new THREE.Vector3(...hqCell.centerUnit).normalize();
      ring.position.copy(n.clone().multiplyScalar(1.018));
      ring.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
      this.group.add(ring);
    }

    // 5. Tracked convoys (only within snapshot visibility horizon)
    for (const convoy of snapshot.trackedConvoys) {
      const cell = cellMap.get(convoy.projectedCellId);
      if (!cell) continue;
      const markerGeom = new THREE.BoxGeometry(0.016, 0.016, 0.016);
      const marker = new THREE.Mesh(markerGeom, this.convoyMat);
      marker.position.set(
        cell.centerUnit[0] * 1.022,
        cell.centerUnit[1] * 1.022,
        cell.centerUnit[2] * 1.022,
      );
      this.group.add(marker);
    }
  }

  dispose(): void {
    while (this.group.children.length > 0) {
      const child = this.group.children.pop();
      if ((child as THREE.Mesh | undefined)?.geometry) {
        (child as THREE.Mesh).geometry.dispose();
      }
    }
    this.riverMat.dispose();
    this.roadMat.dispose();
    this.settlementMat.dispose();
    this.hqRingMat.dispose();
    this.convoyMat.dispose();
  }
}
