import * as THREE from "three";
import {
  add3,
  DiegeticSnapshot,
  GlobePalette,
  GoldbergCellGeometry,
  GoldbergTopologyData,
  greatCircleRadians,
  norm3,
  RgbTuple,
  scale3,
  sub3,
  tangentBasis,
  Vec3,
} from "../spine";
import { SurfacePointProjector } from "./hydrology-ribbons";

export type SemanticZoomTier = "orbital" | "continental" | "regional" | "settlement";

export function classifySemanticZoomTier(altitudeNorm: number): SemanticZoomTier {
  if (altitudeNorm >= 0.70) return "orbital";
  if (altitudeNorm >= 0.40) return "continental";
  if (altitudeNorm >= 0.15) return "regional";
  return "settlement";
}

export interface DeclusteredEntityPosition {
  readonly displayUnit: Vec3;
  readonly anchorUnit: Vec3;
  readonly offsetApplied: boolean;
}

function rgbToThreeColor(rgb: RgbTuple): THREE.Color {
  return new THREE.Color(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
}

/**
 * Separates co-located formations and convoys onto an orbital tangent ring around nearby
 * settlement beacons so district & military symbols never collide (`UI-GLB-006`).
 */
export function computeDeclusteredEntityOffsets(
  snapshot: DiegeticSnapshot,
  cellMap: ReadonlyMap<number, GoldbergCellGeometry>,
  altitudeNorm: number,
): {
  readonly formations: ReadonlyMap<number, DeclusteredEntityPosition>;
  readonly convoys: ReadonlyMap<number, DeclusteredEntityPosition>;
} {
  const formations = new Map<number, DeclusteredEntityPosition>();
  const convoys = new Map<number, DeclusteredEntityPosition>();

  const anchors: { readonly cellId: number; readonly unit: Vec3 }[] = [];
  for (const s of snapshot.knownSettlements ?? []) {
    const c = cellMap.get(s.cellId);
    if (c) anchors.push({ cellId: s.cellId, unit: c.centerUnit });
  }

  const ringRadiusRad = (0.015 + 0.008 * (1 - altitudeNorm)) / Math.sqrt(Math.max(1, cellMap.size / 642));
  const collideThresholdRad = Math.max(0.012, ringRadiusRad * 0.9);
  const slotCounterByAnchor = new Map<number, number>();

  for (const f of snapshot.warfare?.formations ?? []) {
    const baseUnit: Vec3 = [
      f.currentUnitXyz[0],
      f.currentUnitXyz[1],
      f.currentUnitXyz[2],
    ];
    const hitAnchor = anchors.find(
      (a) => a.cellId === f.currentCellId || greatCircleRadians(a.unit, baseUnit) < collideThresholdRad,
    );
    if (!hitAnchor) {
      formations.set(f.formationId, {
        displayUnit: norm3(baseUnit),
        anchorUnit: norm3(baseUnit),
        offsetApplied: false,
      });
      continue;
    }

    const slot = slotCounterByAnchor.get(hitAnchor.cellId) ?? 0;
    slotCounterByAnchor.set(hitAnchor.cellId, slot + 1);
    const angle = (slot * 2.0943951) + 0.55;
    const [east, north] = tangentBasis(hitAnchor.unit);
    const offsetVec = add3(
      scale3(east, Math.cos(angle) * ringRadiusRad),
      scale3(north, Math.sin(angle) * ringRadiusRad),
    );
    const displayUnit = norm3(add3(hitAnchor.unit, offsetVec));
    formations.set(f.formationId, {
      displayUnit,
      anchorUnit: hitAnchor.unit,
      offsetApplied: true,
    });
  }

  for (const convoy of snapshot.trackedConvoys) {
    const cell = cellMap.get(convoy.projectedCellId);
    const baseUnit: Vec3 | null = convoy.currentUnitXyz
      ? [convoy.currentUnitXyz[0], convoy.currentUnitXyz[1], convoy.currentUnitXyz[2]]
      : cell?.centerUnit ?? null;
    if (!baseUnit) continue;

    const hitAnchor = anchors.find(
      (a) => a.cellId === convoy.projectedCellId || greatCircleRadians(a.unit, baseUnit) < collideThresholdRad,
    );
    if (!hitAnchor) {
      convoys.set(convoy.convoyId, {
        displayUnit: norm3(baseUnit),
        anchorUnit: norm3(baseUnit),
        offsetApplied: false,
      });
      continue;
    }

    const slot = slotCounterByAnchor.get(hitAnchor.cellId) ?? 0;
    slotCounterByAnchor.set(hitAnchor.cellId, slot + 1);
    const angle = (slot * 2.0943951) + 1.6;
    const [east, north] = tangentBasis(hitAnchor.unit);
    const offsetVec = add3(
      scale3(east, Math.cos(angle) * (ringRadiusRad * 1.15)),
      scale3(north, Math.sin(angle) * (ringRadiusRad * 1.15)),
    );
    const displayUnit = norm3(add3(hitAnchor.unit, offsetVec));
    convoys.set(convoy.convoyId, {
      displayUnit,
      anchorUnit: hitAnchor.unit,
      offsetApplied: true,
    });
  }

  return { formations, convoys };
}

function orientObjectToSurface(
  obj: THREE.Object3D,
  unit: Vec3,
  eastOffset: number,
  northOffset: number,
  heightOffset: number,
  projectPoint: SurfacePointProjector,
  flatMode: boolean,
): void {
  const [east, north] = tangentBasis(unit);
  const displacedUnit = norm3(
    add3(unit, add3(scale3(east, eastOffset), scale3(north, northOffset))),
  );
  const pos = projectPoint(displacedUnit, heightOffset);
  obj.position.copy(pos);
  const normalVec = flatMode
    ? new THREE.Vector3(0, 0, 1)
    : new THREE.Vector3(displacedUnit[0], displacedUnit[1], displacedUnit[2]).normalize();
  obj.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), normalVec);
}

/**
 * Procedural Tier-2/Tier-3 3D settlement architecture, coastal harbors, agrarian plots, and
 * military tent encampments revealed as camera descends below `altitudeNorm < 0.28` (`UI-GLB-006`).
 */
export class SettlementDetailClusterGroup {
  readonly group = new THREE.Group();
  private keepMat: THREE.MeshStandardMaterial;
  private roofMat: THREE.MeshStandardMaterial;
  private fieldMat: THREE.MeshStandardMaterial;
  private harborMat: THREE.MeshStandardMaterial;
  private tentMat: THREE.MeshStandardMaterial;

  constructor(palette?: GlobePalette) {
    this.keepMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.peakSnowRgb ?? [244, 239, 228]),
      roughness: 0.72,
      metalness: 0.12,
    });
    this.roofMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      roughness: 0.62,
      metalness: 0.14,
    });
    this.fieldMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.landLowRgb ?? [115, 138, 87]),
      roughness: 0.88,
      metalness: 0.04,
    });
    this.harborMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      roughness: 0.75,
      metalness: 0.10,
    });
    this.tentMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.unexploredVellumRgb ?? [230, 216, 185]),
      roughness: 0.82,
      metalness: 0.05,
    });
  }

  setPalette(palette: GlobePalette): void {
    this.keepMat.color.copy(rgbToThreeColor(palette.peakSnowRgb));
    this.roofMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.fieldMat.color.copy(rgbToThreeColor(palette.landLowRgb));
    this.harborMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.tentMat.color.copy(rgbToThreeColor(palette.unexploredVellumRgb));
  }

  rebuild(
    snapshot: DiegeticSnapshot,
    topology: GoldbergTopologyData,
    cellMap: ReadonlyMap<number, GoldbergCellGeometry>,
    declutteredFormations: ReadonlyMap<number, DeclusteredEntityPosition>,
    altitudeNorm: number,
    projectPoint: SurfacePointProjector,
    flatMode: boolean,
  ): number {
    this.clearMeshes();
    const tier = classifySemanticZoomTier(altitudeNorm);
    if (tier === "orbital" || tier === "continental") {
      return 0;
    }

    const knownById = new Map(snapshot.knownCells.map((c) => [c.cellId, c]));
    const fullDetail = tier === "settlement";
    const scaleFactor = fullDetail ? 1.0 : 0.72;
    let builtClusters = 0;

    for (const settlement of snapshot.knownSettlements ?? []) {
      const cell = cellMap.get(settlement.cellId);
      if (!cell) continue;
      builtClusters += 1;

      const popTotal = (snapshot.populationRows ?? [])
        .filter((r) => r.settlementId === settlement.settlementId)
        .reduce((acc, r) => acc + r.reportedHeadcount, 0);
      const popScale = Math.max(0.8, Math.min(1.65, Math.cbrt(Math.max(40, popTotal) / 160))) * scaleFactor;

      // Central HQ citadel / hall
      const keep = new THREE.Mesh(
        new THREE.BoxGeometry(0.0052 * popScale, 0.0068 * popScale, 0.0052 * popScale),
        this.keepMat,
      );
      orientObjectToSurface(keep, cell.centerUnit, 0, 0, 0.011, projectPoint, flatMode);
      this.group.add(keep);

      // Stepped roof cupola on citadel
      const roof = new THREE.Mesh(
        new THREE.ConeGeometry(0.0044 * popScale, 0.0038 * popScale, 4),
        this.roofMat,
      );
      orientObjectToSurface(roof, cell.centerUnit, 0, 0, 0.011 + 0.0052 * popScale, projectPoint, flatMode);
      this.group.add(roof);

      // Flanking storehouses / workshops scaled by facility rows
      const facCount = (snapshot.facilityRows ?? []).filter(
        (f) => f.settlementId === settlement.settlementId && !f.weatherHalted,
      ).length;
      const wingCount = fullDetail ? Math.min(6, Math.max(2, facCount)) : 2;
      for (let w = 0; w < wingCount; w += 1) {
        const ang = (w / wingCount) * Math.PI * 2 + 0.35;
        const rad = 0.0085 * popScale;
        const house = new THREE.Mesh(
          new THREE.BoxGeometry(0.0032 * scaleFactor, 0.0026 * scaleFactor, 0.0024 * scaleFactor),
          w % 2 === 0 ? this.keepMat : this.roofMat,
        );
        orientObjectToSurface(
          house,
          cell.centerUnit,
          Math.cos(ang) * rad,
          Math.sin(ang) * rad,
          0.010,
          projectPoint,
          flatMode,
        );
        this.group.add(house);
      }

      if (fullDetail) {
        // Coastal harbor jetty pointing toward any surveyed ocean neighbor
        for (const nbId of cell.neighbors) {
          const nbKnown = knownById.get(nbId);
          const nbGeom = cellMap.get(nbId);
          if (nbKnown && nbKnown.tier !== "unexplored" && nbKnown.isOcean && nbGeom) {
            const pierDir = sub3(nbGeom.centerUnit, cell.centerUnit);
            const [east, north] = tangentBasis(cell.centerUnit);
            const eDot = pierDir[0] * east[0] + pierDir[1] * east[1] + pierDir[2] * east[2];
            const nDot = pierDir[0] * north[0] + pierDir[1] * north[1] + pierDir[2] * north[2];
            const len = Math.max(1e-6, Math.hypot(eDot, nDot));
            const pier = new THREE.Mesh(
              new THREE.BoxGeometry(0.0052, 0.0012, 0.0018),
              this.harborMat,
            );
            orientObjectToSurface(
              pier,
              cell.centerUnit,
              (eDot / len) * 0.012,
              (nDot / len) * 0.012,
              0.0085,
              projectPoint,
              flatMode,
            );
            this.group.add(pier);
            break;
          }
        }

        // Low-relief terraced agricultural field beds at settlement outskirts
        const plotOffsets: readonly [number, number][] = [
          [-0.013, -0.008],
          [0.013, -0.009],
          [-0.011, 0.012],
        ];
        for (const [oe, on] of plotOffsets) {
          const plot = new THREE.Mesh(
            new THREE.BoxGeometry(0.0046, 0.0008, 0.0034),
            this.fieldMat,
          );
          orientObjectToSurface(plot, cell.centerUnit, oe, on, 0.0082, projectPoint, flatMode);
          this.group.add(plot);
        }
      }
    }

    if (fullDetail) {
      for (const formation of snapshot.warfare?.formations ?? []) {
        const decluttered = declutteredFormations.get(formation.formationId);
        const campUnit: Vec3 = decluttered?.displayUnit ?? [
          formation.currentUnitXyz[0],
          formation.currentUnitXyz[1],
          formation.currentUnitXyz[2],
        ];
        const tentRows = Math.min(4, Math.max(2, Math.round(formation.personnelCount / 120)));
        for (let r = 0; r < tentRows; r += 1) {
          const tent = new THREE.Mesh(new THREE.ConeGeometry(0.0022, 0.0026, 4), this.tentMat);
          orientObjectToSurface(
            tent,
            norm3(campUnit),
            ((r % 2) - 0.5) * 0.0055,
            (Math.floor(r / 2) - 0.5) * 0.0055,
            0.011,
            projectPoint,
            flatMode,
          );
          this.group.add(tent);
        }
      }
    }

    void topology;
    return builtClusters;
  }

  private clearMeshes(): void {
    while (this.group.children.length > 0) {
      const child = this.group.children.pop();
      if ((child as THREE.Mesh | undefined)?.geometry) {
        (child as THREE.Mesh).geometry.dispose();
      }
    }
  }

  dispose(): void {
    this.clearMeshes();
    this.keepMat.dispose();
    this.roofMat.dispose();
    this.fieldMat.dispose();
    this.harborMat.dispose();
    this.tentMat.dispose();
  }
}
