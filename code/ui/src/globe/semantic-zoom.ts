import * as THREE from "three";
import {
  add3,
  DiegeticSnapshot,
  GlobePalette,
  GlobeRayHit,
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
import {
  bisectShorelineUnit,
  createShaderTerrainSurfaceSampler,
  ShaderTerrainSurfaceSampler,
  SurfacePointProjector,
} from "./hydrology-ribbons";

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

const UP_Y = new THREE.Vector3(0, 1, 0);

function rgbToThreeColor(rgb: RgbTuple): THREE.Color {
  return new THREE.Color(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
}

function blendRgb(a: RgbTuple, b: RgbTuple, t: number): THREE.Color {
  const k = Math.max(0, Math.min(1, t));
  return new THREE.Color(
    (a[0] + (b[0] - a[0]) * k) / 255,
    (a[1] + (b[1] - a[1]) * k) / 255,
    (a[2] + (b[2] - a[2]) * k) / 255,
  );
}

function scaleRgb(a: RgbTuple, rMul: number, gMul: number, bMul: number): THREE.Color {
  return new THREE.Color(
    Math.min(1, Math.max(0, (a[0] / 255) * rMul)),
    Math.min(1, Math.max(0, (a[1] / 255) * gMul)),
    Math.min(1, Math.max(0, (a[2] / 255) * bMul)),
  );
}

function hashCellSlot(seedA: number, seedB: number): number {
  let h = (seedA * 374761393 + seedB * 668265263) | 0;
  h = ((h ^ (h >> 13)) * 1274126177) | 0;
  return ((h ^ (h >> 16)) >>> 0) / 4294967295;
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

function computeSurfacePose(
  unit: Vec3,
  eastOffset: number,
  northOffset: number,
  heightOffset: number,
  projectPoint: SurfacePointProjector,
  flatMode: boolean,
  yawRad = 0,
): {
  readonly displacedUnit: Vec3;
  readonly position: THREE.Vector3;
  readonly quaternion: THREE.Quaternion;
} {
  const [east, north] = tangentBasis(unit);
  const displacedUnit = norm3(
    add3(unit, add3(scale3(east, eastOffset), scale3(north, northOffset))),
  );
  const position = projectPoint(displacedUnit, heightOffset);
  const normalVec = flatMode
    ? new THREE.Vector3(0, 0, 1)
    : new THREE.Vector3(displacedUnit[0], displacedUnit[1], displacedUnit[2]).normalize();
  const quaternion = new THREE.Quaternion().setFromUnitVectors(UP_Y, normalVec);
  if (Math.abs(yawRad) > 1e-4) {
    quaternion.multiply(new THREE.Quaternion().setFromAxisAngle(UP_Y, yawRad));
  }
  return { displacedUnit, position, quaternion };
}

/**
 * Procedural Tier-2/Tier-3 3D settlement architecture, coastal harbors, agrarian plots,
 * military tent encampments, and instanced woodland crowns grounded on 3D relief (`UI-GLB-006`).
 */
export class SettlementDetailClusterGroup {
  readonly group = new THREE.Group();
  private keepMat: THREE.MeshStandardMaterial;
  private timberWallMat: THREE.MeshStandardMaterial;
  private roofMat: THREE.MeshStandardMaterial;
  private foundationMat: THREE.MeshStandardMaterial;
  private fieldMat: THREE.MeshStandardMaterial;
  private harborMat: THREE.MeshStandardMaterial;
  private metalMat: THREE.MeshStandardMaterial;
  private tentMat: THREE.MeshStandardMaterial;
  private foliageConiferMat: THREE.MeshStandardMaterial;
  private foliageBroadleafMat: THREE.MeshStandardMaterial;

  private readonly unitBoxGeom = new THREE.BoxGeometry(1, 1, 1);
  private readonly unitRoofPyramidGeom = new THREE.ConeGeometry(Math.SQRT1_2, 1, 4).rotateY(Math.PI / 4);
  private readonly unitCylGeom = new THREE.CylinderGeometry(0.5, 0.5, 1, 8);
  private readonly unitConiferGeom = new THREE.ConeGeometry(0.5, 1, 6);
  private readonly unitBroadleafGeom = new THREE.IcosahedronGeometry(0.5, 1);
  private readonly unitTentGeom = new THREE.ConeGeometry(Math.SQRT1_2, 1, 4).rotateY(Math.PI / 4);

  private forestMeshes: THREE.InstancedMesh[] = [];

  constructor(palette?: GlobePalette) {
    const p: GlobePalette = palette ?? {
      oceanShallowRgb: [135, 163, 173],
      oceanDeepRgb: [61, 89, 102],
      landLowRgb: [115, 138, 87],
      landMidRgb: [176, 162, 116],
      landHighRgb: [140, 114, 88],
      peakSnowRgb: [244, 239, 228],
      riverWaterRgb: [54, 108, 133],
      roadOchreRgb: [166, 89, 40],
      settlementMarkerRgb: [139, 46, 34],
      contourInkRgb: [59, 44, 29],
      compassRoseRgb: [110, 85, 57],
      unexploredVellumRgb: [230, 216, 185],
      parchmentGrainTextureUrl: "",
      hatchPatternTextureUrl: "",
      terrainBiomeTextureUrl: "",
    };

    this.keepMat = new THREE.MeshStandardMaterial({
      color: blendRgb(p.peakSnowRgb, p.landMidRgb, 0.20),
      roughness: 0.72,
      metalness: 0.12,
    });
    this.timberWallMat = new THREE.MeshStandardMaterial({
      color: blendRgb(p.unexploredVellumRgb, p.roadOchreRgb, 0.30),
      roughness: 0.78,
      metalness: 0.06,
    });
    this.roofMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(p.settlementMarkerRgb),
      roughness: 0.62,
      metalness: 0.14,
    });
    this.foundationMat = new THREE.MeshStandardMaterial({
      color: blendRgb(p.landHighRgb, p.contourInkRgb, 0.45),
      roughness: 0.86,
      metalness: 0.08,
    });
    this.fieldMat = new THREE.MeshStandardMaterial({
      color: blendRgb(p.landLowRgb, p.landMidRgb, 0.28),
      roughness: 0.88,
      metalness: 0.04,
    });
    this.harborMat = new THREE.MeshStandardMaterial({
      color: blendRgb(p.roadOchreRgb, p.contourInkRgb, 0.30),
      roughness: 0.75,
      metalness: 0.10,
    });
    this.metalMat = new THREE.MeshStandardMaterial({
      color: blendRgb(p.contourInkRgb, p.landHighRgb, 0.38),
      roughness: 0.56,
      metalness: 0.34,
    });
    this.tentMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(p.unexploredVellumRgb),
      roughness: 0.82,
      metalness: 0.05,
    });
    this.foliageConiferMat = new THREE.MeshStandardMaterial({
      color: scaleRgb(p.landLowRgb, 0.48, 0.72, 0.56),
      roughness: 0.86,
      metalness: 0.02,
    });
    this.foliageBroadleafMat = new THREE.MeshStandardMaterial({
      color: scaleRgb(p.landLowRgb, 0.68, 0.94, 0.58),
      roughness: 0.84,
      metalness: 0.02,
    });
  }

  setPalette(palette: GlobePalette): void {
    this.keepMat.color.copy(blendRgb(palette.peakSnowRgb, palette.landMidRgb, 0.20));
    this.timberWallMat.color.copy(blendRgb(palette.unexploredVellumRgb, palette.roadOchreRgb, 0.30));
    this.roofMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.foundationMat.color.copy(blendRgb(palette.landHighRgb, palette.contourInkRgb, 0.45));
    this.fieldMat.color.copy(blendRgb(palette.landLowRgb, palette.landMidRgb, 0.28));
    this.harborMat.color.copy(blendRgb(palette.roadOchreRgb, palette.contourInkRgb, 0.30));
    this.metalMat.color.copy(blendRgb(palette.contourInkRgb, palette.landHighRgb, 0.38));
    this.tentMat.color.copy(rgbToThreeColor(palette.unexploredVellumRgb));
    this.foliageConiferMat.color.copy(scaleRgb(palette.landLowRgb, 0.48, 0.72, 0.56));
    this.foliageBroadleafMat.color.copy(scaleRgb(palette.landLowRgb, 0.68, 0.94, 0.58));
  }

  updateForCameraFrame(altitudeNorm: number): void {
    const show = altitudeNorm < 0.40;
    this.group.visible = show;
    const crownVisScale = show
      ? Math.max(0.55, Math.min(1.0, (0.40 - altitudeNorm) / 0.24))
      : 0.0;
    for (const fm of this.forestMeshes) {
      fm.visible = crownVisScale > 0.05;
    }
  }

  rebuild(
    snapshot: DiegeticSnapshot,
    topology: GoldbergTopologyData,
    cellMap: ReadonlyMap<number, GoldbergCellGeometry>,
    declutteredFormations: ReadonlyMap<number, DeclusteredEntityPosition>,
    altitudeNorm: number,
    projectPoint: SurfacePointProjector,
    flatMode: boolean,
    surfaceSampler?: ShaderTerrainSurfaceSampler,
    focusUnit?: Vec3,
  ): number {
    this.clearMeshes();
    const tier = classifySemanticZoomTier(altitudeNorm);
    if (tier === "orbital" || tier === "continental") {
      return 0;
    }

    const sampler = surfaceSampler ?? createShaderTerrainSurfaceSampler(snapshot, topology);
    const knownById = new Map(snapshot.knownCells.map((c) => [c.cellId, c]));
    const fullDetail = tier === "settlement";
    const scaleFactor = fullDetail ? 1.0 : 0.72;
    const flatReliefScale = flatMode ? 0.65 : 1.0;
    let builtClusters = 0;

    const placeMesh = (
      geometry: THREE.BufferGeometry,
      material: THREE.Material,
      anchorUnit: Vec3,
      hintCellId: number,
      eastOff: number,
      northOff: number,
      sx: number,
      sy: number,
      sz: number,
      localCenterY: number,
      yawRad: number,
      skipOnOcean = true,
      useZeroSeaLevel = false,
      pickHit?: GlobeRayHit,
    ): THREE.Mesh | null => {
      const [east, north] = tangentBasis(anchorUnit);
      const displacedUnit = norm3(
        add3(anchorUnit, add3(scale3(east, eastOff), scale3(north, northOff))),
      );
      let terrainH = 0;
      if (!useZeroSeaLevel) {
        const surf = sampler.sample(displacedUnit, hintCellId, hintCellId, altitudeNorm);
        if (!surf.isExplored) return null;
        if (skipOnOcean && surf.effectiveElevM < 1.2) return null;
        terrainH = surf.vertexReliefHeight * flatReliefScale;
      }
      const mesh = new THREE.Mesh(geometry, material);
      const pose = computeSurfacePose(
        anchorUnit,
        eastOff,
        northOff,
        terrainH + localCenterY,
        projectPoint,
        flatMode,
        yawRad,
      );
      mesh.position.copy(pose.position);
      mesh.quaternion.copy(pose.quaternion);
      mesh.scale.set(sx, sy, sz);
      if (pickHit) {
        mesh.userData.pickHit = pickHit;
      }
      this.group.add(mesh);
      return mesh;
    };

    const addBuilding = (
      anchorUnit: Vec3,
      hintCellId: number,
      eastOff: number,
      northOff: number,
      w: number,
      h: number,
      d: number,
      roofH: number,
      yawRad: number,
      wallMat: THREE.Material,
      pickHit?: GlobeRayHit,
    ) => {
      const plinthH = 0.0022 * scaleFactor;
      const deckY = 0.0004 * scaleFactor;
      const plinthCenterY = deckY - plinthH * 0.5;
      const wallBase = deckY;
      const plinth = placeMesh(
        this.unitBoxGeom,
        this.foundationMat,
        anchorUnit,
        hintCellId,
        eastOff,
        northOff,
        w * 1.24,
        plinthH,
        d * 1.24,
        plinthCenterY,
        yawRad,
        true,
        false,
        pickHit,
      );
      if (!plinth) return;
      placeMesh(
        this.unitBoxGeom,
        wallMat,
        anchorUnit,
        hintCellId,
        eastOff,
        northOff,
        w,
        h,
        d,
        wallBase + h * 0.5,
        yawRad,
        true,
        false,
        pickHit,
      );
      if (roofH > 0) {
        placeMesh(
          this.unitRoofPyramidGeom,
          this.roofMat,
          anchorUnit,
          hintCellId,
          eastOff,
          northOff,
          w * 1.15,
          roofH,
          d * 1.15,
          wallBase + h + roofH * 0.5,
          yawRad,
          true,
          false,
          pickHit,
        );
      }
    };

    const settledCellIds = new Map<number, Vec3>();

    for (const settlement of snapshot.knownSettlements ?? []) {
      const cell = cellMap.get(settlement.cellId);
      if (!cell) continue;
      settledCellIds.set(cell.cellId, cell.centerUnit);
      builtClusters += 1;

      const popTotal = (snapshot.populationRows ?? [])
        .filter((r) => r.settlementId === settlement.settlementId)
        .reduce((acc, r) => acc + r.reportedHeadcount, 0);
      const popScale =
        Math.max(0.8, Math.min(1.65, Math.cbrt(Math.max(40, popTotal) / 160))) * scaleFactor;

      const settlementPick: GlobeRayHit = {
        hitKind: "settlement",
        cellId: cell.cellId,
        settlementId: settlement.settlementId,
        label: settlement.isHq ? `${settlement.settlementName} (HQ)` : settlement.settlementName,
      };

      // 1. Grounded Central Civic Hall / HQ Citadel
      addBuilding(
        cell.centerUnit,
        cell.cellId,
        0,
        0,
        0.0046 * popScale,
        0.0048 * popScale,
        0.0040 * popScale,
        0.0028 * popScale,
        0.2,
        this.keepMat,
        settlementPick,
      );
      if (settlement.isHq) {
        addBuilding(
          cell.centerUnit,
          cell.cellId,
          0.0022 * popScale,
          0.0014 * popScale,
          0.0022 * popScale,
          0.0074 * popScale,
          0.0022 * popScale,
          0.0032 * popScale,
          0.2,
          this.keepMat,
          settlementPick,
        );
        if (fullDetail) {
          for (let sIdx = 0; sIdx < 6; sIdx += 1) {
            const ang = (sIdx / 6) * Math.PI * 2 + 0.15;
            const wallRad = 0.0062 * popScale;
            placeMesh(
              this.unitBoxGeom,
              this.foundationMat,
              cell.centerUnit,
              cell.cellId,
              Math.cos(ang) * wallRad,
              Math.sin(ang) * wallRad,
              0.0032 * popScale,
              0.0024 * popScale,
              0.0009 * popScale,
              0.0008 * popScale,
              -ang + Math.PI * 0.5,
              true,
              false,
              settlementPick,
            );
          }
        }
      }

      // 2. Facility Workshops (smelter chimneys, waterwheel mills, storehouses)
      const activeFacs = (snapshot.facilityRows ?? [])
        .filter((f) => f.settlementId === settlement.settlementId && !f.weatherHalted)
        .slice(0, fullDetail ? 6 : 2);
      const facCount = activeFacs.length;
      for (let fIdx = 0; fIdx < facCount; fIdx += 1) {
        const fac = activeFacs[fIdx];
        const ang = (fIdx / Math.max(1, facCount)) * Math.PI * 2 + 0.45;
        const rad = 0.0076 * popScale;
        const fx = Math.cos(ang) * rad;
        const fn = Math.sin(ang) * rad;
        const keyLower = `${fac.processKey} ${fac.processName}`.toLowerCase();
        const isSmelter = /smelt|blast|hearth|forge|foundry|kiln/.test(keyLower);
        const isMill = /waterwheel|mill|pinion|lathe|bore|saw/.test(keyLower);
        const facPick: GlobeRayHit = {
          hitKind: "facility",
          cellId: cell.cellId,
          facilityId: fac.facilityId,
          settlementId: settlement.settlementId,
          label: `${fac.processName} (${settlement.settlementName})`,
        };

        addBuilding(
          cell.centerUnit,
          cell.cellId,
          fx,
          fn,
          0.0028 * scaleFactor,
          0.0024 * scaleFactor,
          0.0020 * scaleFactor,
          0.0014 * scaleFactor,
          -ang,
          this.timberWallMat,
          facPick,
        );
        if (isSmelter && fullDetail) {
          placeMesh(
            this.unitCylGeom,
            this.metalMat,
            cell.centerUnit,
            cell.cellId,
            fx + Math.cos(ang + 0.5) * 0.0016,
            fn + Math.sin(ang + 0.5) * 0.0016,
            0.0007 * scaleFactor,
            0.0052 * scaleFactor,
            0.0007 * scaleFactor,
            0.0024 * scaleFactor,
            0,
            true,
            false,
            facPick,
          );
        } else if (isMill && fullDetail) {
          const wheel = placeMesh(
            this.unitCylGeom,
            this.harborMat,
            cell.centerUnit,
            cell.cellId,
            fx - Math.sin(ang) * 0.0014,
            fn + Math.cos(ang) * 0.0014,
            0.0014 * scaleFactor,
            0.0006 * scaleFactor,
            0.0014 * scaleFactor,
            0.0011 * scaleFactor,
            -ang,
            true,
            false,
            facPick,
          );
          if (wheel) {
            wheel.quaternion.multiply(
              new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), Math.PI * 0.5),
            );
          }
        }
      }

      // 3. Street-aligned residential townhouses scaled by population
      const houseCount = fullDetail ? Math.min(10, Math.max(3, Math.round(popTotal / 45) + 2)) : 3;
      for (let hIdx = 0; hIdx < houseCount; hIdx += 1) {
        const lane = hIdx % 4;
        const ring = Math.floor(hIdx / 4);
        const ang = (lane / 4) * Math.PI * 2 + 0.85 + ring * 0.35;
        const rad = (0.0068 + ring * 0.0038 + hashCellSlot(settlement.settlementId, hIdx) * 0.0015) * popScale;
        const hx = Math.cos(ang) * rad;
        const hn = Math.sin(ang) * rad;
        const hW = (0.0021 + hashCellSlot(hIdx, 7) * 0.0007) * scaleFactor;
        const hD = (0.0017 + hashCellSlot(hIdx, 13) * 0.0006) * scaleFactor;
        const hH = (0.0017 + hashCellSlot(hIdx, 23) * 0.0007) * scaleFactor;
        addBuilding(
          cell.centerUnit,
          cell.cellId,
          hx,
          hn,
          hW,
          hH,
          hD,
          0.0011 * scaleFactor,
          -ang,
          hIdx % 2 === 0 ? this.keepMat : this.timberWallMat,
          settlementPick,
        );
      }

      if (fullDetail) {
        // 4. Shoreline-bisected stone quay and timber harbor pier at land/sea edge
        for (const nbId of cell.neighbors) {
          const nbKnown = knownById.get(nbId);
          const nbGeom = cellMap.get(nbId);
          if (nbKnown && nbKnown.tier !== "unexplored" && (nbKnown.isOcean || nbKnown.elevationM < 0) && nbGeom) {
            const shoreUnit = bisectShorelineUnit(
              sampler,
              cell.centerUnit,
              nbGeom.centerUnit,
              cell.cellId,
              nbId,
              altitudeNorm,
              1.5,
            );
            const [sEast, sNorth] = tangentBasis(shoreUnit);
            const waterDir = sub3(nbGeom.centerUnit, shoreUnit);
            const eDot = waterDir[0] * sEast[0] + waterDir[1] * sEast[1] + waterDir[2] * sEast[2];
            const nDot = waterDir[0] * sNorth[0] + waterDir[1] * sNorth[1] + waterDir[2] * sNorth[2];
            const len = Math.max(1e-6, Math.hypot(eDot, nDot));
            const dirE = eDot / len;
            const dirN = nDot / len;
            const pierYaw = Math.atan2(dirE, dirN);

            // Stone quay bulkhead right on dry land edge
            placeMesh(
              this.unitBoxGeom,
              this.foundationMat,
              shoreUnit,
              cell.cellId,
              -dirE * 0.0010,
              -dirN * 0.0010,
              0.0036,
              0.0016,
              0.0018,
              0.0004,
              pierYaw,
              true,
            );
            // Timber jetty stretching out onto ocean level `0`
            placeMesh(
              this.unitBoxGeom,
              this.harborMat,
              shoreUnit,
              cell.cellId,
              dirE * 0.0022,
              dirN * 0.0022,
              0.0016,
              0.0009,
              0.0046,
              0.0008,
              pierYaw,
              false,
              true,
            );
            break;
          }
        }

        // 5. Hedged terraced agricultural crop plots on surrounding dry slopes
        const [cellEast, cellNorth] = tangentBasis(cell.centerUnit);
        const plotAngles = [0.4, 2.0, 3.6, 5.1];
        for (let pIdx = 0; pIdx < plotAngles.length; pIdx += 1) {
          const ang = plotAngles[pIdx];
          const oe = Math.cos(ang) * 0.0135 * popScale;
          const on = Math.sin(ang) * 0.0135 * popScale;
          const surfPlot = sampler.sample(
            norm3(add3(cell.centerUnit, add3(scale3(cellEast, oe), scale3(cellNorth, on)))),
            cell.cellId,
            cell.cellId,
            altitudeNorm,
          );
          if (!surfPlot.isExplored || surfPlot.effectiveElevM < 4.0) continue;

          placeMesh(
            this.unitBoxGeom,
            this.foundationMat,
            cell.centerUnit,
            cell.cellId,
            oe,
            on,
            0.0046,
            0.0012,
            0.0036,
            -0.0003,
            -ang,
          );
          placeMesh(
            this.unitBoxGeom,
            this.fieldMat,
            cell.centerUnit,
            cell.cellId,
            oe,
            on,
            0.0042,
            0.0006,
            0.0032,
            0.0004,
            -ang,
          );
          placeMesh(
            this.unitBoxGeom,
            this.foliageBroadleafMat,
            cell.centerUnit,
            cell.cellId,
            oe + Math.cos(ang + Math.PI * 0.5) * 0.0017,
            on + Math.sin(ang + Math.PI * 0.5) * 0.0017,
            0.0043,
            0.0008,
            0.0004,
            0.0007,
            -ang,
          );
        }
      }
    }

    // 5b. Resource / quarry / timber-mill outposts anchored at non-settlement endpoints of access roads
    const renderedOutpostCells = new Set<number>();
    for (const road of snapshot.surveyedRoads ?? []) {
      if (road.roadPurpose !== "resource_or_port_access") continue;
      const endpointCellId = !settledCellIds.has(road.toCellId)
        ? road.toCellId
        : !settledCellIds.has(road.fromCellId)
          ? road.fromCellId
          : null;
      if (endpointCellId === null || renderedOutpostCells.has(endpointCellId)) continue;
      const epCell = cellMap.get(endpointCellId);
      if (!epCell) continue;
      renderedOutpostCells.add(endpointCellId);
      builtClusters += 1;

      const outpostPick: GlobeRayHit = {
        hitKind: "facility",
        cellId: endpointCellId,
        facilityId: 900000 + road.edgeId,
        label: road.purposeLabel || `Outpost (Edge ${road.edgeId})`,
      };
      const isQuarry = (road.purposeLabel ?? "").toLowerCase().includes("quarry");
      addBuilding(
        epCell.centerUnit,
        endpointCellId,
        0,
        0,
        0.0034 * scaleFactor,
        0.0028 * scaleFactor,
        0.0026 * scaleFactor,
        isQuarry ? 0 : 0.0016 * scaleFactor,
        0.4,
        isQuarry ? this.foundationMat : this.timberWallMat,
        outpostPick,
      );
      if (isQuarry) {
        placeMesh(
          this.unitCylGeom,
          this.metalMat,
          epCell.centerUnit,
          endpointCellId,
          0.0022 * scaleFactor,
          0.0012 * scaleFactor,
          0.0009 * scaleFactor,
          0.0046 * scaleFactor,
          0.0009 * scaleFactor,
          0.0023 * scaleFactor,
          0,
          true,
          false,
          outpostPick,
        );
      }
    }

    // 6. Grounded Military Tent Encampments & Earthwork Parapet Berms (`fullDetail`)
    if (fullDetail) {
      for (const formation of snapshot.warfare?.formations ?? []) {
        const decluttered = declutteredFormations.get(formation.formationId);
        const campUnit: Vec3 = norm3(
          decluttered?.displayUnit ?? [
            formation.currentUnitXyz[0],
            formation.currentUnitXyz[1],
            formation.currentUnitXyz[2],
          ],
        );
        const formPick: GlobeRayHit = {
          hitKind: "formation",
          cellId: formation.currentCellId,
          formationId: formation.formationId,
          label: `${formation.label} (${formation.commanderName})`,
        };
        // Command pavilion tent
        placeMesh(
          this.unitTentGeom,
          this.roofMat,
          campUnit,
          formation.currentCellId,
          0,
          0,
          0.0026,
          0.0026,
          0.0026,
          0.0011,
          0.3,
          true,
          false,
          formPick,
        );
        const tentRows = Math.min(6, Math.max(3, Math.round(formation.personnelCount / 90)));
        for (let r = 0; r < tentRows; r += 1) {
          placeMesh(
            this.unitTentGeom,
            this.tentMat,
            campUnit,
            formation.currentCellId,
            ((r % 3) - 1.0) * 0.0034,
            (Math.floor(r / 3) - 0.5) * 0.0036 + 0.0026,
            0.0019,
            0.0020,
            0.0019,
            0.0008,
            0.15,
            true,
            false,
            formPick,
          );
        }
        // Defensive earthwork parapet berms around encampment perimeter
        for (let pSide = 0; pSide < 4; pSide += 1) {
          const ang = (pSide / 4) * Math.PI * 2;
          placeMesh(
            this.unitBoxGeom,
            this.foundationMat,
            campUnit,
            formation.currentCellId,
            Math.cos(ang) * 0.0064,
            Math.sin(ang) * 0.0064,
            0.0044,
            0.0009,
            0.0008,
            0.0002,
            -ang + Math.PI * 0.5,
            true,
            false,
            formPick,
          );
        }
      }
    }

    // 7. Instanced 3D Conifer & Broadleaf Woodland Crowns (`THREE.InstancedMesh`)
    const hqCell = cellMap.get(snapshot.hqPresence.cellId);
    const firstSettledUnit = settledCellIds.values().next().value as Vec3 | undefined;
    const focusCenter = norm3(focusUnit ?? hqCell?.centerUnit ?? firstSettledUnit ?? [0, 0, 1]);
    const forestFocusRadiusRad = fullDetail ? 0.42 : 0.34;
    const coniferMatrices: THREE.Matrix4[] = [];
    const broadleafMatrices: THREE.Matrix4[] = [];
    const dummyMat = new THREE.Matrix4();
    const dummyPos = new THREE.Vector3();
    const dummyQuat = new THREE.Quaternion();
    const dummyScale = new THREE.Vector3();
    const maxTotalTrees = 1800;

    for (const cell of snapshot.knownCells) {
      if (coniferMatrices.length + broadleafMatrices.length >= maxTotalTrees) break;
      if (cell.tier === "unexplored" || cell.tier === "rumored") continue;
      if (cell.isOcean || cell.elevationM < 4 || cell.elevationM > 2100) continue;
      const biomeLower = (cell.biomeKey ?? "").toLowerCase();
      if (/desert|arid|erg|glacier|ice|snow|barren/.test(biomeLower)) continue;
      const geom = cellMap.get(cell.cellId);
      if (!geom) continue;
      if (greatCircleRadians(geom.centerUnit, focusCenter) > forestFocusRadiusRad) continue;

      const isConiferCell =
        /taiga|boreal|pine|spruce|alpine|subalpine|cold/.test(biomeLower) ||
        cell.elevationM > 780 ||
        (cell.meanAnnualTempC ?? 14) < 6.0;
      const aridity = cell.aridityIndex01 ?? 0.9;
      const densityFactor = aridity < 0.35 ? 0.35 : biomeLower.includes("forest") || biomeLower.includes("taiga") ? 1.0 : 0.6;
      const countPerCell = Math.max(3, Math.round((fullDetail ? 10 : 6) * densityFactor));
      const [east, north] = tangentBasis(geom.centerUnit);
      const settledInThisCell = settledCellIds.has(cell.cellId);

      for (let tIdx = 0; tIdx < countPerCell; tIdx += 1) {
        if (coniferMatrices.length + broadleafMatrices.length >= maxTotalTrees) break;
        const ang = hashCellSlot(cell.cellId, tIdx * 3 + 1) * Math.PI * 2;
        const minR = settledInThisCell ? 0.011 : 0.002;
        const maxR = 0.022;
        const dist = minR + hashCellSlot(cell.cellId, tIdx * 3 + 2) * (maxR - minR);
        const oe = Math.cos(ang) * dist;
        const on = Math.sin(ang) * dist;
        const treeUnit = norm3(
          add3(geom.centerUnit, add3(scale3(east, oe), scale3(north, on))),
        );
        const surf = sampler.sample(treeUnit, cell.cellId, cell.cellId, altitudeNorm);
        if (!surf.isExplored || surf.effectiveElevM < 4.5 || surf.effectiveElevM > 2000) continue;

        const sizeVar = 0.75 + hashCellSlot(cell.cellId, tIdx * 3 + 5) * 0.55;
        const h = (isConiferCell ? 0.0024 : 0.0019) * sizeVar * scaleFactor;
        const w = (isConiferCell ? 0.0012 : 0.0015) * sizeVar * scaleFactor;
        const groundH = surf.vertexReliefHeight * flatReliefScale;

        dummyPos.copy(projectPoint(treeUnit, groundH + h * 0.42));
        const nVec = flatMode
          ? new THREE.Vector3(0, 0, 1)
          : new THREE.Vector3(treeUnit[0], treeUnit[1], treeUnit[2]).normalize();
        dummyQuat.setFromUnitVectors(UP_Y, nVec);
        dummyScale.set(w, h, w);
        dummyMat.compose(dummyPos, dummyQuat, dummyScale);

        if (isConiferCell || surf.effectiveElevM > 780) {
          coniferMatrices.push(dummyMat.clone());
        } else {
          broadleafMatrices.push(dummyMat.clone());
        }
      }
    }

    if (coniferMatrices.length > 0) {
      const im = new THREE.InstancedMesh(
        this.unitConiferGeom,
        this.foliageConiferMat,
        coniferMatrices.length,
      );
      im.frustumCulled = false;
      for (let i = 0; i < coniferMatrices.length; i += 1) {
        im.setMatrixAt(i, coniferMatrices[i]);
      }
      im.instanceMatrix.needsUpdate = true;
      this.forestMeshes.push(im);
      this.group.add(im);
    }

    if (broadleafMatrices.length > 0) {
      const im = new THREE.InstancedMesh(
        this.unitBroadleafGeom,
        this.foliageBroadleafMat,
        broadleafMatrices.length,
      );
      im.frustumCulled = false;
      for (let i = 0; i < broadleafMatrices.length; i += 1) {
        im.setMatrixAt(i, broadleafMatrices[i]);
      }
      im.instanceMatrix.needsUpdate = true;
      this.forestMeshes.push(im);
      this.group.add(im);
    }

    void topology;
    return builtClusters;
  }

  private clearMeshes(): void {
    this.forestMeshes = [];
    while (this.group.children.length > 0) {
      const child = this.group.children.pop();
      if (child instanceof THREE.InstancedMesh) {
        child.dispose();
      }
    }
  }

  dispose(): void {
    this.clearMeshes();
    this.unitBoxGeom.dispose();
    this.unitRoofPyramidGeom.dispose();
    this.unitCylGeom.dispose();
    this.unitConiferGeom.dispose();
    this.unitBroadleafGeom.dispose();
    this.unitTentGeom.dispose();
    this.keepMat.dispose();
    this.timberWallMat.dispose();
    this.roofMat.dispose();
    this.foundationMat.dispose();
    this.fieldMat.dispose();
    this.harborMat.dispose();
    this.metalMat.dispose();
    this.tentMat.dispose();
    this.foliageConiferMat.dispose();
    this.foliageBroadleafMat.dispose();
  }
}
