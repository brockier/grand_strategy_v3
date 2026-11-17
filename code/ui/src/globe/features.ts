import * as THREE from "three";
import {
  DiegeticSnapshot,
  GlobePalette,
  GoldbergCellGeometry,
  GoldbergTopologyData,
  norm3,
  RgbTuple,
  slerpUnitVec3,
  Vec3,
} from "../spine";
import {
  buildHydrologyRibbonMesh,
  buildRoadRibbonMesh,
  SurfacePointProjector,
} from "./hydrology-ribbons";
import {
  classifySemanticZoomTier,
  computeDeclusteredEntityOffsets,
  SemanticZoomTier,
  SettlementDetailClusterGroup,
} from "./semantic-zoom";

function rgbToThreeColor(rgb: RgbTuple): THREE.Color {
  return new THREE.Color(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
}

const defaultSphereProjector: SurfacePointProjector = (unit: Vec3, lift: number) =>
  new THREE.Vector3(unit[0], unit[1], unit[2]).normalize().multiplyScalar(1 + lift);

export interface MapLabelAnchor {
  readonly id: string;
  readonly kind: "settlement" | "formation";
  readonly entityId: number;
  readonly text: string;
  readonly subtitle: string;
  readonly unit: Vec3;
  readonly isPrimary: boolean;
}

export interface FeatureOverlayRenderSummary {
  readonly activeModeId: string;
  readonly previewDestinationCellId: number | null;
  readonly convoyCount: number;
  readonly confirmedConvoyCount: number;
  readonly projectedConvoyCount: number;
  readonly overdueConvoyCount: number;
  readonly lostConvoyCount: number;
  readonly windVectorCount: number;
  readonly stormCellCount: number;
  readonly floodedRiverCount: number;
  readonly renderedRiverCount?: number;
  readonly settlementClusterCount?: number;
  readonly semanticZoomTier?: SemanticZoomTier;
  readonly commContourCount: number;
  readonly tradeRibbonCount: number;
  readonly congestedCorridorCount: number;
  readonly theaterVectorCount: number;
  readonly formationCount: number;
  readonly noRetreatCount: number;
}

function appendProjectedArcSegments(
  target: THREE.Vector3[],
  a: Vec3,
  b: Vec3,
  segments: number,
  lift: number,
  projectPoint: SurfacePointProjector,
): void {
  const count = Math.max(2, segments);
  const pts: THREE.Vector3[] = [];
  for (let i = 0; i <= count; i += 1) {
    const t = i / count;
    pts.push(projectPoint(slerpUnitVec3(a, b, t), lift));
  }
  for (let i = 0; i + 1 < pts.length; i += 1) {
    if (pts[i].distanceTo(pts[i + 1]) <= 0.72) {
      target.push(pts[i], pts[i + 1]);
    }
  }
}

/**
 * Manages lit 3D surveyed settlements, Tier-3 settlement clusters, de-clustered formations & convoys,
 * fog-clipped meandering river ribbons, road ribbons, HQ beacons, communication contours, relocation preview arcs,
 * and observed weather glyphs (`UI-GLB-006`, `UI-MAP-001`..`UI-MAP-004`).
 */
export class GlobeFeatureOverlayGroup {
  readonly group = new THREE.Group();
  private readonly detailClusters: SettlementDetailClusterGroup;
  private riverMat: THREE.MeshStandardMaterial;
  private roadMat: THREE.MeshStandardMaterial;
  private dashedRouteMat: THREE.LineDashedMaterial;
  private previewRouteMat: THREE.LineDashedMaterial;
  private windVectorMat: THREE.LineBasicMaterial;
  private leaderTickMat: THREE.LineBasicMaterial;
  private settlementMat: THREE.MeshStandardMaterial;
  private hqRingMat: THREE.MeshStandardMaterial;
  private contourRingMat: THREE.MeshStandardMaterial;
  private convoyConfirmedMat: THREE.MeshStandardMaterial;
  private convoyProjectedMat: THREE.MeshStandardMaterial;
  private convoyOverdueMat: THREE.MeshStandardMaterial;
  private convoyLostMat: THREE.LineBasicMaterial;
  private floodRingMat: THREE.MeshStandardMaterial;
  private snowMarkerMat: THREE.MeshStandardMaterial;
  private tradeRibbonMat: THREE.LineBasicMaterial;
  private congestionMat: THREE.LineBasicMaterial;
  private theaterVectorMat: THREE.LineDashedMaterial;
  private phaseLineMat: THREE.LineBasicMaterial;
  private formationFreshMat: THREE.MeshStandardMaterial;
  private formationStaleMat: THREE.MeshStandardMaterial;
  private noRetreatRingMat: THREE.MeshStandardMaterial;

  private labelAnchors: MapLabelAnchor[] = [];
  private lastSnapshot: DiegeticSnapshot | null = null;
  private lastTopology: GoldbergTopologyData | null = null;
  private lastModeId = "terrain";
  private lastPreviewDest: number | null = null;
  private lastAltitudeNorm = 0.62;
  private lastSemanticTier: SemanticZoomTier = "continental";
  private lastFlatMode = false;

  private lastSummary: FeatureOverlayRenderSummary = {
    activeModeId: "terrain",
    previewDestinationCellId: null,
    convoyCount: 0,
    confirmedConvoyCount: 0,
    projectedConvoyCount: 0,
    overdueConvoyCount: 0,
    lostConvoyCount: 0,
    windVectorCount: 0,
    stormCellCount: 0,
    floodedRiverCount: 0,
    renderedRiverCount: 0,
    settlementClusterCount: 0,
    semanticZoomTier: "continental",
    commContourCount: 0,
    tradeRibbonCount: 0,
    congestedCorridorCount: 0,
    theaterVectorCount: 0,
    formationCount: 0,
    noRetreatCount: 0,
  };

  constructor(palette?: GlobePalette) {
    this.detailClusters = new SettlementDetailClusterGroup(palette);
    this.group.add(this.detailClusters.group);

    this.riverMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.riverWaterRgb ?? [54, 108, 133]),
      roughness: 0.28,
      metalness: 0.14,
      side: THREE.DoubleSide,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -2,
    });
    this.roadMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      roughness: 0.86,
      metalness: 0.04,
      side: THREE.DoubleSide,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -2,
    });
    this.dashedRouteMat = new THREE.LineDashedMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      dashSize: 0.025,
      gapSize: 0.015,
      transparent: true,
      opacity: 0.72,
    });
    this.previewRouteMat = new THREE.LineDashedMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      dashSize: 0.03,
      gapSize: 0.012,
      transparent: true,
      opacity: 0.92,
    });
    this.windVectorMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.contourInkRgb ?? [59, 44, 29]),
      transparent: true,
      opacity: 0.75,
    });
    this.leaderTickMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      transparent: true,
      opacity: 0.85,
    });
    this.settlementMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      roughness: 0.55,
      metalness: 0.18,
    });
    this.hqRingMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      roughness: 0.42,
      metalness: 0.35,
      side: THREE.DoubleSide,
    });
    this.contourRingMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      roughness: 0.65,
      metalness: 0.10,
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.45,
    });
    this.convoyConfirmedMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.peakSnowRgb ?? [244, 239, 228]),
      roughness: 0.48,
      metalness: 0.22,
    });
    this.convoyProjectedMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      roughness: 0.62,
      metalness: 0.12,
      transparent: true,
      opacity: 0.64,
    });
    this.convoyOverdueMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      roughness: 0.7,
      metalness: 0.1,
      wireframe: true,
    });
    this.convoyLostMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      transparent: true,
      opacity: 0.85,
    });
    this.floodRingMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.riverWaterRgb ?? [54, 108, 133]),
      roughness: 0.35,
      metalness: 0.15,
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.68,
    });
    this.snowMarkerMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.peakSnowRgb ?? [244, 239, 228]),
      roughness: 0.75,
      metalness: 0.05,
      transparent: true,
      opacity: 0.78,
    });
    this.tradeRibbonMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.landMidRgb ?? [176, 162, 116]),
      transparent: true,
      opacity: 0.88,
    });
    this.congestionMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      transparent: true,
      opacity: 0.94,
    });
    this.theaterVectorMat = new THREE.LineDashedMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      dashSize: 0.032,
      gapSize: 0.014,
      transparent: true,
      opacity: 0.9,
    });
    this.phaseLineMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.contourInkRgb ?? [59, 44, 29]),
      transparent: true,
      opacity: 0.85,
    });
    this.formationFreshMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      roughness: 0.45,
      metalness: 0.30,
    });
    this.formationStaleMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      roughness: 0.68,
      metalness: 0.15,
      transparent: true,
      opacity: 0.65,
    });
    this.noRetreatRingMat = new THREE.MeshStandardMaterial({
      color: rgbToThreeColor(palette?.contourInkRgb ?? [59, 44, 29]),
      roughness: 0.55,
      metalness: 0.20,
      side: THREE.DoubleSide,
    });
  }

  setPalette(palette: GlobePalette): void {
    this.detailClusters.setPalette(palette);
    this.riverMat.color.copy(rgbToThreeColor(palette.riverWaterRgb));
    this.roadMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.dashedRouteMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.previewRouteMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.windVectorMat.color.copy(rgbToThreeColor(palette.contourInkRgb));
    this.leaderTickMat.color.copy(rgbToThreeColor(palette.compassRoseRgb));
    this.settlementMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.hqRingMat.color.copy(rgbToThreeColor(palette.compassRoseRgb));
    this.contourRingMat.color.copy(rgbToThreeColor(palette.compassRoseRgb));
    this.convoyConfirmedMat.color.copy(rgbToThreeColor(palette.peakSnowRgb));
    this.convoyProjectedMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.convoyOverdueMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.convoyLostMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.floodRingMat.color.copy(rgbToThreeColor(palette.riverWaterRgb));
    this.snowMarkerMat.color.copy(rgbToThreeColor(palette.peakSnowRgb));
    this.tradeRibbonMat.color.copy(rgbToThreeColor(palette.landMidRgb));
    this.congestionMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.theaterVectorMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.phaseLineMat.color.copy(rgbToThreeColor(palette.contourInkRgb));
    this.formationFreshMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.formationStaleMat.color.copy(rgbToThreeColor(palette.compassRoseRgb));
    this.noRetreatRingMat.color.copy(rgbToThreeColor(palette.contourInkRgb));
  }

  getSummary(): FeatureOverlayRenderSummary {
    return this.lastSummary;
  }

  getMapLabelAnchors(): readonly MapLabelAnchor[] {
    return this.labelAnchors;
  }

  refreshForCamera(
    altitudeNorm: number,
    projectPoint: SurfacePointProjector,
    flatMode: boolean,
  ): void {
    const nextTier = classifySemanticZoomTier(altitudeNorm);
    const zoomStepChanged =
      nextTier !== this.lastSemanticTier ||
      Math.abs(altitudeNorm - this.lastAltitudeNorm) > 0.08 ||
      flatMode !== this.lastFlatMode;
    if (!zoomStepChanged || !this.lastSnapshot || !this.lastTopology) {
      return;
    }
    this.rebuild(
      this.lastSnapshot,
      this.lastTopology,
      this.lastModeId,
      this.lastPreviewDest,
      altitudeNorm,
      projectPoint,
      flatMode,
    );
  }

  private clearFeatureMeshes(): void {
    for (let i = this.group.children.length - 1; i >= 0; i -= 1) {
      const child = this.group.children[i];
      if (child === this.detailClusters.group) continue;
      this.group.remove(child);
      if ((child as THREE.Mesh).geometry) {
        (child as THREE.Mesh).geometry.dispose();
      }
    }
  }

  rebuild(
    snapshot: DiegeticSnapshot,
    topology: GoldbergTopologyData,
    activeModeId = "terrain",
    previewDestinationCellId: number | null = null,
    altitudeNorm = 0.62,
    projectPoint: SurfacePointProjector = defaultSphereProjector,
    flatMode = false,
  ): void {
    this.clearFeatureMeshes();
    this.lastSnapshot = snapshot;
    this.lastTopology = topology;
    this.lastModeId = activeModeId;
    this.lastPreviewDest = previewDestinationCellId;
    this.lastAltitudeNorm = altitudeNorm;
    this.lastSemanticTier = classifySemanticZoomTier(altitudeNorm);
    this.lastFlatMode = flatMode;
    this.labelAnchors = [];

    const cellMap = new Map<number, GoldbergCellGeometry>();
    for (const c of topology.cells) cellMap.set(c.cellId, c);

    const weatherByCell = new Map<number, (typeof snapshot.observedWeather)[number]>();
    for (const w of snapshot.observedWeather) {
      weatherByCell.set(w.cellId, w);
    }

    const densityScale = Math.sqrt(Math.max(1, topology.cellCount) / 642);
    const markerScale = Math.max(0.28, (0.55 + 0.45 * altitudeNorm) / Math.max(1, densityScale * 0.55));
    const surfaceNormalFor = (u: Vec3): THREE.Vector3 =>
      flatMode ? new THREE.Vector3(0, 0, 1) : new THREE.Vector3(u[0], u[1], u[2]).normalize();
    const yUp = new THREE.Vector3(0, 1, 0);
    const zUp = new THREE.Vector3(0, 0, 1);

    // 1. Fog-clipped tapering meandering river ribbons (`UI-GLB-006`, `UI-MAP-003`, `UI-MAP-004`).
    const riverBuild = buildHydrologyRibbonMesh(
      snapshot,
      topology,
      cellMap,
      weatherByCell,
      altitudeNorm,
      this.riverMat,
      projectPoint,
      flatMode,
    );
    if (riverBuild.mesh) {
      this.group.add(riverBuild.mesh);
    }

    // 2. Curved intact road ribbons & dashed washed-out segments.
    const roadMesh = buildRoadRibbonMesh(
      snapshot,
      topology,
      cellMap,
      altitudeNorm,
      this.roadMat,
      projectPoint,
      flatMode,
    );
    if (roadMesh) {
      this.group.add(roadMesh);
    }
    const washedRoadSegs: THREE.Vector3[] = [];
    for (const road of snapshot.surveyedRoads ?? []) {
      if (!road.washedOut) continue;
      const src = cellMap.get(road.fromCellId);
      const dst = cellMap.get(road.toCellId);
      if (!src || !dst) continue;
      appendProjectedArcSegments(washedRoadSegs, src.centerUnit, dst.centerUnit, 8, 0.009, projectPoint);
    }
    if (washedRoadSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(washedRoadSegs);
      const line = new THREE.LineSegments(geom, this.dashedRouteMat);
      line.computeLineDistances();
      this.group.add(line);
    }

    // Compute radial declustering offsets before placing settlements, formations & convoys.
    const declustered = computeDeclusteredEntityOffsets(snapshot, cellMap, altitudeNorm);
    const leaderSegs: THREE.Vector3[] = [];

    // 3. Lit stepped civic settlement monuments + map label anchors.
    for (const settlement of snapshot.knownSettlements ?? []) {
      const cell = cellMap.get(settlement.cellId);
      if (!cell) continue;
      const up = surfaceNormalFor(cell.centerUnit);
      const quat = new THREE.Quaternion().setFromUnitVectors(yUp, up);

      const plinth = new THREE.Mesh(
        new THREE.CylinderGeometry(0.011 * markerScale, 0.014 * markerScale, 0.008 * markerScale, 8),
        this.settlementMat,
      );
      plinth.position.copy(projectPoint(cell.centerUnit, 0.013));
      plinth.quaternion.copy(quat);
      this.group.add(plinth);

      if (settlement.isHq) {
        const spire = new THREE.Mesh(
          new THREE.ConeGeometry(0.007 * markerScale, 0.011 * markerScale, 6),
          this.hqRingMat,
        );
        spire.position.copy(projectPoint(cell.centerUnit, 0.013 + 0.008 * markerScale));
        spire.quaternion.copy(quat);
        this.group.add(spire);
      }

      const popTotal = (snapshot.populationRows ?? [])
        .filter((r) => r.settlementId === settlement.settlementId)
        .reduce((acc, r) => acc + r.reportedHeadcount, 0);
      this.labelAnchors.push({
        id: `settlement:${settlement.settlementId}`,
        kind: "settlement",
        entityId: settlement.settlementId,
        text: settlement.isHq ? `${settlement.settlementName} (HQ)` : settlement.settlementName,
        subtitle: popTotal > 0 ? `Pop ${popTotal}` : "Settlement",
        unit: cell.centerUnit,
        isPrimary: settlement.isHq,
      });
    }

    // 4. Live HQ beacon ring, communication latency contours (`UI-MAP-001`), and relocation preview arc (`UI-HUD-005`).
    const hqCell = cellMap.get(snapshot.hqPresence.cellId);
    if (hqCell) {
      const ringGeom = new THREE.RingGeometry(0.020 * markerScale, 0.028 * markerScale, 28);
      const ring = new THREE.Mesh(ringGeom, this.hqRingMat);
      ring.position.copy(projectPoint(hqCell.centerUnit, 0.016));
      ring.quaternion.setFromUnitVectors(zUp, surfaceNormalFor(hqCell.centerUnit));
      this.group.add(ring);
    }

    let commContourCount = 0;
    const contours = snapshot.hqPresence.communicationContours ?? [];
    if (activeModeId === "epistemic" || activeModeId === "logistics") {
      for (const sample of contours) {
        const cell = cellMap.get(sample.cellId);
        if (!cell) continue;
        commContourCount += 1;
        const inner = (0.012 + Math.min(0.025, sample.estimatedCourierHours * 0.0005)) * markerScale;
        const ringGeom = new THREE.RingGeometry(inner, inner + 0.005 * markerScale, 16);
        const ring = new THREE.Mesh(ringGeom, this.contourRingMat);
        ring.position.copy(projectPoint(cell.centerUnit, 0.014));
        ring.quaternion.setFromUnitVectors(zUp, surfaceNormalFor(cell.centerUnit));
        this.group.add(ring);
      }
    }

    const activePreviewTarget =
      previewDestinationCellId ??
      (snapshot.hqPresence.inTransit ? (snapshot.hqPresence.destinationCellId ?? null) : null);
    if (hqCell && activePreviewTarget !== null) {
      const dstCell = cellMap.get(activePreviewTarget);
      if (dstCell) {
        const previewSegs: THREE.Vector3[] = [];
        appendProjectedArcSegments(previewSegs, hqCell.centerUnit, dstCell.centerUnit, 14, 0.024, projectPoint);
        if (previewSegs.length > 0) {
          const arcGeom = new THREE.BufferGeometry().setFromPoints(previewSegs);
          const arcLine = new THREE.LineSegments(arcGeom, this.previewRouteMat);
          arcLine.computeLineDistances();
          this.group.add(arcLine);
        }

        const targetRingGeom = new THREE.RingGeometry(0.018 * markerScale, 0.026 * markerScale, 20);
        const targetRing = new THREE.Mesh(targetRingGeom, this.hqRingMat);
        targetRing.position.copy(projectPoint(dstCell.centerUnit, 0.018));
        targetRing.quaternion.setFromUnitVectors(zUp, surfaceNormalFor(dstCell.centerUnit));
        this.group.add(targetRing);
      }
    }

    // 5. Dead-reckoning vs live tracked convoys (with radial declustering when docked near a settlement).
    let confirmedConvoyCount = 0;
    let projectedConvoyCount = 0;
    let overdueConvoyCount = 0;
    let lostConvoyCount = 0;
    const dashedRouteSegs: THREE.Vector3[] = [];
    const lostCrossSegs: THREE.Vector3[] = [];

    for (const convoy of snapshot.trackedConvoys) {
      const cell = cellMap.get(convoy.projectedCellId);
      const rawUnit: Vec3 | null = convoy.currentUnitXyz
        ? [convoy.currentUnitXyz[0], convoy.currentUnitXyz[1], convoy.currentUnitXyz[2]]
        : cell?.centerUnit ?? null;
      if (!rawUnit) continue;

      const dec = declustered.convoys.get(convoy.convoyId);
      const convoyUnit = dec?.displayUnit ?? rawUnit;
      if (dec?.offsetApplied) {
        appendProjectedArcSegments(leaderSegs, dec.anchorUnit, convoyUnit, 2, 0.015, projectPoint);
      }

      const statusLower = convoy.status.toLowerCase();
      const pos = projectPoint(convoyUnit, 0.019);
      const up = surfaceNormalFor(convoyUnit);
      const quat = new THREE.Quaternion().setFromUnitVectors(yUp, up);

      if (statusLower.includes("lost") || statusLower.includes("missing")) {
        lostConvoyCount += 1;
        const pCenter = pos;
        const nVec = flatMode ? new THREE.Vector3(0, 0, 1) : new THREE.Vector3(...convoyUnit).normalize();
        const refAxis = Math.abs(nVec.y) < 0.9 ? yUp : new THREE.Vector3(1, 0, 0);
        const east = new THREE.Vector3().crossVectors(refAxis, nVec).normalize().multiplyScalar(0.012 * markerScale);
        const north = new THREE.Vector3().crossVectors(nVec, east).normalize().multiplyScalar(0.012 * markerScale);
        lostCrossSegs.push(pCenter.clone().sub(east), pCenter.clone().add(east));
        lostCrossSegs.push(pCenter.clone().sub(north), pCenter.clone().add(north));
      } else if (statusLower.includes("overdue")) {
        overdueConvoyCount += 1;
        const marker = new THREE.Mesh(
          new THREE.BoxGeometry(0.016 * markerScale, 0.012 * markerScale, 0.010 * markerScale),
          this.convoyOverdueMat,
        );
        marker.position.copy(pos);
        marker.quaternion.copy(quat);
        this.group.add(marker);
      } else if (statusLower.includes("confirmed") || statusLower.includes("live")) {
        confirmedConvoyCount += 1;
        const marker = new THREE.Mesh(
          new THREE.BoxGeometry(0.015 * markerScale, 0.011 * markerScale, 0.009 * markerScale),
          this.convoyConfirmedMat,
        );
        marker.position.copy(pos);
        marker.quaternion.copy(quat);
        this.group.add(marker);
      } else {
        projectedConvoyCount += 1;
        const marker = new THREE.Mesh(
          new THREE.OctahedronGeometry(0.011 * markerScale, 0),
          this.convoyProjectedMat,
        );
        marker.position.copy(pos);
        marker.quaternion.copy(quat);
        this.group.add(marker);
      }

      const waypoints = convoy.routeWaypointsXyz;
      if (waypoints && waypoints.length >= 2) {
        for (let i = 0; i + 1 < waypoints.length; i += 1) {
          const wpA: Vec3 = [waypoints[i][0], waypoints[i][1], waypoints[i][2]];
          const wpB: Vec3 = [waypoints[i + 1][0], waypoints[i + 1][1], waypoints[i + 1][2]];
          appendProjectedArcSegments(dashedRouteSegs, wpA, wpB, 4, 0.015, projectPoint);
        }
      } else {
        const route = convoy.routeCells ?? [];
        for (let i = 0; i + 1 < route.length; i += 1) {
          const src = cellMap.get(route[i]);
          const dst = cellMap.get(route[i + 1]);
          if (!src || !dst) continue;
          appendProjectedArcSegments(dashedRouteSegs, src.centerUnit, dst.centerUnit, 6, 0.015, projectPoint);
        }
      }
    }
    if (dashedRouteSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(dashedRouteSegs);
      const lines = new THREE.LineSegments(geom, this.dashedRouteMat);
      lines.computeLineDistances();
      this.group.add(lines);
    }
    if (lostCrossSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(lostCrossSegs);
      this.group.add(new THREE.LineSegments(geom, this.convoyLostMat));
    }

    // 6. Observed weather wind chevrons, flood rings, and snow markers (`UI-MAP-004`).
    let windVectorCount = 0;
    let stormCellCount = 0;
    const windSegs: THREE.Vector3[] = [];
    const showWeatherDetails = activeModeId === "weather";

    for (const w of snapshot.observedWeather) {
      const cell = cellMap.get(w.cellId);
      if (!cell) continue;
      const u = w.windUMps ?? 0;
      const v = w.windVMps ?? 0;
      const windSpeed = Math.hypot(u, v);
      const flood = w.floodStage01 ?? 0;
      const snow = w.snowDepthM ?? 0;

      if (windSpeed > 0.4) {
        windVectorCount += 1;
        if (showWeatherDetails) {
          const n = new THREE.Vector3(...cell.centerUnit).normalize();
          const pole = Math.abs(n.y) < 0.9 ? yUp : new THREE.Vector3(1, 0, 0);
          const east = new THREE.Vector3().crossVectors(pole, n).normalize();
          const north = new THREE.Vector3().crossVectors(n, east).normalize();
          const dirVec = east.multiplyScalar(u / windSpeed).add(north.multiplyScalar(v / windSpeed)).normalize();
          const len = Math.min(0.032, 0.012 + windSpeed * 0.0012) * markerScale;
          const tipUnit = norm3([
            cell.centerUnit[0] + dirVec.x * len,
            cell.centerUnit[1] + dirVec.y * len,
            cell.centerUnit[2] + dirVec.z * len,
          ]);
          const p0 = projectPoint(cell.centerUnit, 0.019);
          const p1 = projectPoint(tipUnit, 0.019);
          if (p0.distanceTo(p1) <= 0.5) {
            windSegs.push(p0, p1);
          }
        }
      }

      if (w.precipMmHr > 1.5 || flood > 0.25) {
        stormCellCount += 1;
        if (showWeatherDetails) {
          const floodGeom = new THREE.RingGeometry(0.013 * markerScale, 0.020 * markerScale, 16);
          const floodRing = new THREE.Mesh(floodGeom, this.floodRingMat);
          floodRing.position.copy(projectPoint(cell.centerUnit, 0.015));
          floodRing.quaternion.setFromUnitVectors(zUp, surfaceNormalFor(cell.centerUnit));
          this.group.add(floodRing);
        }
      }

      if (snow > 0.05 && showWeatherDetails) {
        const snowGeom = new THREE.SphereGeometry(0.008 * markerScale, 8, 6);
        const snowMesh = new THREE.Mesh(snowGeom, this.snowMarkerMat);
        snowMesh.position.copy(projectPoint(cell.centerUnit, 0.016));
        this.group.add(snowMesh);
      }
    }
    if (windSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(windSegs);
      this.group.add(new THREE.LineSegments(geom, this.windVectorMat));
    }

    // 7. M5 Trade ribbons & corridor congestion bottlenecks (`UI-MAP-006.1`, `UI-MAP-006.2`).
    let tradeRibbonCount = 0;
    let congestedCorridorCount = 0;
    if (activeModeId === "trade") {
      const tradeSegs: THREE.Vector3[] = [];
      for (const convoy of snapshot.trackedConvoys) {
        const origCell =
          convoy.originCellId !== undefined ? cellMap.get(convoy.originCellId) : undefined;
        const destCell =
          convoy.destinationCellId !== undefined ? cellMap.get(convoy.destinationCellId) : undefined;
        const srcUnit: Vec3 | null = convoy.originUnitXyz
          ? [convoy.originUnitXyz[0], convoy.originUnitXyz[1], convoy.originUnitXyz[2]]
          : origCell?.centerUnit ?? null;
        const dstUnit: Vec3 | null = convoy.destinationUnitXyz
          ? [convoy.destinationUnitXyz[0], convoy.destinationUnitXyz[1], convoy.destinationUnitXyz[2]]
          : destCell?.centerUnit ?? null;
        if (srcUnit && dstUnit) {
          tradeRibbonCount += 1;
          appendProjectedArcSegments(tradeSegs, srcUnit, dstUnit, 12, 0.019, projectPoint);
        }
      }
      const settlementCellById = new Map<number, GoldbergCellGeometry>();
      for (const st of snapshot.knownSettlements ?? []) {
        const sc = cellMap.get(st.cellId);
        if (sc) settlementCellById.set(st.settlementId, sc);
      }
      const markets = snapshot.nonStateEconomy?.informalMarkets ?? [];
      for (let i = 0; i < markets.length; i += 1) {
        for (let j = i + 1; j < markets.length; j += 1) {
          const a = markets[i];
          const b = markets[j];
          if (a.commodityKey !== b.commodityKey || a.settlementId === b.settlementId) continue;
          const cellA = settlementCellById.get(a.settlementId);
          const cellB = settlementCellById.get(b.settlementId);
          if (!cellA || !cellB) continue;
          tradeRibbonCount += 1;
          appendProjectedArcSegments(tradeSegs, cellA.centerUnit, cellB.centerUnit, 10, 0.019, projectPoint);
        }
      }
      if (tradeSegs.length > 0) {
        const geom = new THREE.BufferGeometry().setFromPoints(tradeSegs);
        this.group.add(new THREE.LineSegments(geom, this.tradeRibbonMat));
      }

      const congestedSegs: THREE.Vector3[] = [];
      for (const edge of snapshot.nonStateEconomy?.congestedEdges ?? []) {
        if (edge.congestionMultiplier <= 1.05) continue;
        const src = cellMap.get(edge.fromCellId);
        const dst = cellMap.get(edge.toCellId);
        if (!src || !dst) continue;
        congestedCorridorCount += 1;
        appendProjectedArcSegments(congestedSegs, src.centerUnit, dst.centerUnit, 8, 0.021, projectPoint);
      }
      if (congestedSegs.length > 0) {
        const geom = new THREE.BufferGeometry().setFromPoints(congestedSegs);
        this.group.add(new THREE.LineSegments(geom, this.congestionMat));
      }
    }

    // 8. M7 Operational warfare formations (radially declustered from settlement beacons) + phase lines.
    let theaterVectorCount = 0;
    let formationCount = 0;
    let noRetreatCount = 0;
    if (activeModeId === "warfare" || activeModeId === "terrain" || activeModeId === "parchment") {
      const currentTick = snapshot.clockAndNotifications.tick;
      const phaseSegs: THREE.Vector3[] = [];
      for (const formation of snapshot.warfare?.formations ?? []) {
        formationCount += 1;
        const dec = declustered.formations.get(formation.formationId);
        const fUnit: Vec3 = dec?.displayUnit ?? [
          formation.currentUnitXyz[0],
          formation.currentUnitXyz[1],
          formation.currentUnitXyz[2],
        ];
        if (dec?.offsetApplied) {
          appendProjectedArcSegments(leaderSegs, dec.anchorUnit, fUnit, 2, 0.015, projectPoint);
        }

        const ageTicks = Math.max(0, currentTick - formation.observedTick);
        const isFresh = ageTicks <= 6;
        const up = surfaceNormalFor(fUnit);
        const quat = new THREE.Quaternion().setFromUnitVectors(yUp, up);
        const fMat = isFresh ? this.formationFreshMat : this.formationStaleMat;

        const baseBlock = new THREE.Mesh(
          new THREE.BoxGeometry(0.014 * markerScale, 0.005 * markerScale, 0.009 * markerScale),
          fMat,
        );
        baseBlock.position.copy(projectPoint(fUnit, 0.019));
        baseBlock.quaternion.copy(quat);
        this.group.add(baseBlock);

        const banner = new THREE.Mesh(
          new THREE.ConeGeometry(0.006 * markerScale, 0.011 * markerScale, 4),
          fMat,
        );
        banner.position.copy(projectPoint(fUnit, 0.019 + 0.007 * markerScale));
        banner.quaternion.copy(quat);
        this.group.add(banner);

        this.labelAnchors.push({
          id: `formation:${formation.formationId}`,
          kind: "formation",
          entityId: formation.formationId,
          text: formation.label,
          subtitle: `${formation.personnelCount} personnel`,
          unit: fUnit,
          isPrimary: false,
        });

        if (activeModeId === "warfare") {
          if (formation.targetCellId !== formation.currentCellId) {
            const targetUnit: Vec3 = [
              formation.targetUnitXyz[0],
              formation.targetUnitXyz[1],
              formation.targetUnitXyz[2],
            ];
            appendProjectedArcSegments(phaseSegs, fUnit, targetUnit, 10, 0.022, projectPoint);
          }
          if (formation.noRetreatOrder) {
            noRetreatCount += 1;
            const nrRingGeom = new THREE.RingGeometry(0.018 * markerScale, 0.024 * markerScale, 16);
            const nrRing = new THREE.Mesh(nrRingGeom, this.noRetreatRingMat);
            nrRing.position.copy(projectPoint(fUnit, 0.021));
            nrRing.quaternion.setFromUnitVectors(zUp, up);
            this.group.add(nrRing);
          }
        }
      }
      if (phaseSegs.length > 0) {
        const geom = new THREE.BufferGeometry().setFromPoints(phaseSegs);
        this.group.add(new THREE.LineSegments(geom, this.phaseLineMat));
      }

      if (activeModeId === "warfare") {
        for (const theater of snapshot.warfare?.theaters ?? []) {
          const dstCell = cellMap.get(theater.objectiveCellId);
          const theaterFormation = (snapshot.warfare?.formations ?? []).find(
            (f) => f.theaterId === theater.theaterId,
          );
          const srcUnit: Vec3 | null = theaterFormation
            ? [
                theaterFormation.currentUnitXyz[0],
                theaterFormation.currentUnitXyz[1],
                theaterFormation.currentUnitXyz[2],
              ]
            : hqCell?.centerUnit ?? null;
          if (!srcUnit || !dstCell) continue;
          theaterVectorCount += 1;
          const theaterSegs: THREE.Vector3[] = [];
          appendProjectedArcSegments(theaterSegs, srcUnit, dstCell.centerUnit, 12, 0.024, projectPoint);
          if (theaterSegs.length > 0) {
            const geom = new THREE.BufferGeometry().setFromPoints(theaterSegs);
            const line = new THREE.LineSegments(geom, this.theaterVectorMat);
            line.computeLineDistances();
            this.group.add(line);
          }
        }
      }
    }

    if (leaderSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(leaderSegs);
      this.group.add(new THREE.LineSegments(geom, this.leaderTickMat));
    }

    // 9. Procedural Tier-2/Tier-3 3D settlement architecture & military tent encampments (`UI-GLB-006`).
    const settlementClusterCount = this.detailClusters.rebuild(
      snapshot,
      topology,
      cellMap,
      declustered.formations,
      altitudeNorm,
      projectPoint,
      flatMode,
    );

    this.lastSummary = {
      activeModeId,
      previewDestinationCellId: activePreviewTarget,
      convoyCount: snapshot.trackedConvoys.length,
      confirmedConvoyCount,
      projectedConvoyCount,
      overdueConvoyCount,
      lostConvoyCount,
      windVectorCount,
      stormCellCount,
      floodedRiverCount: riverBuild.floodedRiverCount,
      renderedRiverCount: riverBuild.renderedRiverCount,
      settlementClusterCount,
      semanticZoomTier: this.lastSemanticTier,
      commContourCount,
      tradeRibbonCount,
      congestedCorridorCount,
      theaterVectorCount,
      formationCount,
      noRetreatCount,
    };
  }

  dispose(): void {
    this.clearFeatureMeshes();
    this.detailClusters.dispose();
    this.riverMat.dispose();
    this.roadMat.dispose();
    this.dashedRouteMat.dispose();
    this.previewRouteMat.dispose();
    this.windVectorMat.dispose();
    this.leaderTickMat.dispose();
    this.settlementMat.dispose();
    this.hqRingMat.dispose();
    this.contourRingMat.dispose();
    this.convoyConfirmedMat.dispose();
    this.convoyProjectedMat.dispose();
    this.convoyOverdueMat.dispose();
    this.convoyLostMat.dispose();
    this.floodRingMat.dispose();
    this.snowMarkerMat.dispose();
    this.tradeRibbonMat.dispose();
    this.congestionMat.dispose();
    this.theaterVectorMat.dispose();
    this.phaseLineMat.dispose();
    this.formationFreshMat.dispose();
    this.formationStaleMat.dispose();
    this.noRetreatRingMat.dispose();
  }
}
