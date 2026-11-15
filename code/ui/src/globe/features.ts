import * as THREE from "three";
import {
  DiegeticSnapshot,
  GlobePalette,
  GoldbergCellGeometry,
  GoldbergTopologyData,
  RgbTuple,
  Vec3,
} from "../spine";
import { geodesicPointsBetween } from "./mesh";

function rgbToThreeColor(rgb: RgbTuple): THREE.Color {
  return new THREE.Color(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
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
  readonly commContourCount: number;
  readonly tradeRibbonCount: number;
  readonly congestedCorridorCount: number;
  readonly theaterVectorCount: number;
  readonly formationCount: number;
  readonly noRetreatCount: number;
}

/**
 * Manages 3D surveyed settlements, rivers, road splines, HQ beacon rings,
 * communication contours, relocation preview arcs, dead-reckoning convoy markers,
 * and observed weather glyphs slightly above the Goldberg spherical shell
 * (`UI-MAP-001`, `UI-MAP-002`, `UI-MAP-003`, `UI-MAP-004`).
 */
export class GlobeFeatureOverlayGroup {
  readonly group = new THREE.Group();
  private riverMat: THREE.LineBasicMaterial;
  private roadMat: THREE.LineBasicMaterial;
  private dashedRouteMat: THREE.LineDashedMaterial;
  private previewRouteMat: THREE.LineDashedMaterial;
  private windVectorMat: THREE.LineBasicMaterial;
  private settlementMat: THREE.MeshBasicMaterial;
  private hqRingMat: THREE.MeshBasicMaterial;
  private contourRingMat: THREE.MeshBasicMaterial;
  private convoyConfirmedMat: THREE.MeshBasicMaterial;
  private convoyProjectedMat: THREE.MeshBasicMaterial;
  private convoyOverdueMat: THREE.MeshBasicMaterial;
  private convoyLostMat: THREE.LineBasicMaterial;
  private floodRingMat: THREE.MeshBasicMaterial;
  private snowMarkerMat: THREE.MeshBasicMaterial;
  private tradeRibbonMat: THREE.LineBasicMaterial;
  private congestionMat: THREE.LineBasicMaterial;
  private theaterVectorMat: THREE.LineDashedMaterial;
  private phaseLineMat: THREE.LineBasicMaterial;
  private formationFreshMat: THREE.MeshBasicMaterial;
  private formationStaleMat: THREE.MeshBasicMaterial;
  private noRetreatRingMat: THREE.MeshBasicMaterial;
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
    commContourCount: 0,
    tradeRibbonCount: 0,
    congestedCorridorCount: 0,
    theaterVectorCount: 0,
    formationCount: 0,
    noRetreatCount: 0,
  };

  constructor(palette?: GlobePalette) {
    this.riverMat = new THREE.LineBasicMaterial({ color: rgbToThreeColor(palette?.riverWaterRgb ?? [54, 108, 133]) });
    this.roadMat = new THREE.LineBasicMaterial({ color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]) });
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
    this.settlementMat = new THREE.MeshBasicMaterial({ color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]) });
    this.hqRingMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      side: THREE.DoubleSide,
    });
    this.contourRingMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.compassRoseRgb ?? [110, 85, 57]),
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.45,
    });
    this.convoyConfirmedMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.peakSnowRgb ?? [244, 239, 228]),
    });
    this.convoyProjectedMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      transparent: true,
      opacity: 0.58,
    });
    this.convoyOverdueMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      wireframe: true,
    });
    this.convoyLostMat = new THREE.LineBasicMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
      transparent: true,
      opacity: 0.85,
    });
    this.floodRingMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.riverWaterRgb ?? [54, 108, 133]),
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.65,
    });
    this.snowMarkerMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.peakSnowRgb ?? [244, 239, 228]),
      transparent: true,
      opacity: 0.75,
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
    this.formationFreshMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.settlementMarkerRgb ?? [139, 46, 34]),
    });
    this.formationStaleMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.roadOchreRgb ?? [166, 89, 40]),
      transparent: true,
      opacity: 0.55,
    });
    this.noRetreatRingMat = new THREE.MeshBasicMaterial({
      color: rgbToThreeColor(palette?.contourInkRgb ?? [59, 44, 29]),
      side: THREE.DoubleSide,
    });
  }

  setPalette(palette: GlobePalette): void {
    this.riverMat.color.copy(rgbToThreeColor(palette.riverWaterRgb));
    this.roadMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.dashedRouteMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.previewRouteMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.windVectorMat.color.copy(rgbToThreeColor(palette.contourInkRgb));
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
    this.formationFreshMat.color.copy(rgbToThreeColor(palette.settlementMarkerRgb));
    this.formationStaleMat.color.copy(rgbToThreeColor(palette.roadOchreRgb));
    this.noRetreatRingMat.color.copy(rgbToThreeColor(palette.contourInkRgb));
  }

  getSummary(): FeatureOverlayRenderSummary {
    return this.lastSummary;
  }

  rebuild(
    snapshot: DiegeticSnapshot,
    topology: GoldbergTopologyData,
    activeModeId = "terrain",
    previewDestinationCellId: number | null = null,
  ): void {
    while (this.group.children.length > 0) {
      const child = this.group.children.pop();
      if (!child) continue;
      if ((child as THREE.Mesh).geometry) {
        (child as THREE.Mesh).geometry.dispose();
      }
    }

    const cellMap = new Map<number, GoldbergCellGeometry>();
    for (const c of topology.cells) cellMap.set(c.cellId, c);

    // 1. Surveyed rivers only (`UI-MAP-003` / `UI-MAP-004`: never render rivers for Unexplored or Rumored cells; widen on flooded cells)
    const weatherByCell = new Map<number, (typeof snapshot.observedWeather)[number]>();
    for (const w of snapshot.observedWeather) {
      weatherByCell.set(w.cellId, w);
    }

    const riverSegs: THREE.Vector3[] = [];
    const floodedRiverSegs: THREE.Vector3[] = [];
    let floodedRiverCount = 0;
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
      const obsW = weatherByCell.get(cell.cellId);
      if (obsW && (obsW.floodStage01 ?? 0) > 0.25 && arc.length >= 2) {
        floodedRiverCount += 1;
        const chord = arc[arc.length - 1].clone().sub(arc[0]);
        const radial = arc[0].clone().normalize();
        const side = new THREE.Vector3().crossVectors(radial, chord).normalize().multiplyScalar(0.004);
        for (const sign of [-1, 1]) {
          const offset = side.clone().multiplyScalar(sign);
          for (let i = 0; i < arc.length - 1; i += 1) {
            floodedRiverSegs.push(arc[i].clone().add(offset), arc[i + 1].clone().add(offset));
          }
        }
      }
    }
    if (riverSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(riverSegs);
      this.group.add(new THREE.LineSegments(geom, this.riverMat));
    }
    if (floodedRiverSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(floodedRiverSegs);
      this.group.add(new THREE.LineSegments(geom, this.riverMat));
    }

    // 2. Surveyed road edges (solid) vs washed-out spans (dashed)
    const roadSegs: THREE.Vector3[] = [];
    const washedRoadSegs: THREE.Vector3[] = [];
    for (const road of snapshot.surveyedRoads ?? []) {
      const src = cellMap.get(road.fromCellId);
      const dst = cellMap.get(road.toCellId);
      if (!src || !dst) continue;
      const arc = geodesicPointsBetween(src.centerUnit, dst.centerUnit, 8, 1.009);
      const bucket = road.washedOut ? washedRoadSegs : roadSegs;
      for (let i = 0; i < arc.length - 1; i += 1) {
        bucket.push(arc[i], arc[i + 1]);
      }
    }
    if (roadSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(roadSegs);
      this.group.add(new THREE.LineSegments(geom, this.roadMat));
    }
    if (washedRoadSegs.length > 0) {
      const geom = new THREE.BufferGeometry().setFromPoints(washedRoadSegs);
      const line = new THREE.LineSegments(geom, this.dashedRouteMat);
      line.computeLineDistances();
      this.group.add(line);
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

    // 4. Live HQ beacon ring, communication latency contours (`UI-MAP-001`), and relocation preview arc (`UI-HUD-005`)
    const hqCell = cellMap.get(snapshot.hqPresence.cellId);
    if (hqCell) {
      const ringGeom = new THREE.RingGeometry(0.022, 0.032, 24);
      const ring = new THREE.Mesh(ringGeom, this.hqRingMat);
      const n = new THREE.Vector3(...hqCell.centerUnit).normalize();
      ring.position.copy(n.clone().multiplyScalar(1.018));
      ring.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
      this.group.add(ring);
    }

    let commContourCount = 0;
    const contours = snapshot.hqPresence.communicationContours ?? [];
    if (activeModeId === "epistemic" || activeModeId === "logistics") {
      for (const sample of contours) {
        const cell = cellMap.get(sample.cellId);
        if (!cell) continue;
        commContourCount += 1;
        const inner = 0.012 + Math.min(0.025, sample.estimatedCourierHours * 0.0005);
        const ringGeom = new THREE.RingGeometry(inner, inner + 0.005, 16);
        const ring = new THREE.Mesh(ringGeom, this.contourRingMat);
        const n = new THREE.Vector3(...cell.centerUnit).normalize();
        ring.position.copy(n.clone().multiplyScalar(1.014));
        ring.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
        this.group.add(ring);
      }
    }

    const activePreviewTarget =
      previewDestinationCellId ??
      (snapshot.hqPresence.inTransit ? (snapshot.hqPresence.destinationCellId ?? null) : null);
    if (hqCell && activePreviewTarget !== null) {
      const dstCell = cellMap.get(activePreviewTarget);
      if (dstCell) {
        const arcPts = geodesicPointsBetween(hqCell.centerUnit, dstCell.centerUnit, 14, 1.024);
        const arcGeom = new THREE.BufferGeometry().setFromPoints(arcPts);
        const arcLine = new THREE.Line(arcGeom, this.previewRouteMat);
        arcLine.computeLineDistances();
        this.group.add(arcLine);

        const targetRingGeom = new THREE.RingGeometry(0.020, 0.030, 20);
        const targetRing = new THREE.Mesh(targetRingGeom, this.hqRingMat);
        const dn = new THREE.Vector3(...dstCell.centerUnit).normalize();
        targetRing.position.copy(dn.clone().multiplyScalar(1.019));
        targetRing.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), dn);
        this.group.add(targetRing);
      }
    }

    // 5. Dead-reckoning vs live tracked convoys (`UI-MAP-002` + Option B continuous coordinates)
    let confirmedConvoyCount = 0;
    let projectedConvoyCount = 0;
    let overdueConvoyCount = 0;
    let lostConvoyCount = 0;
    const dashedRouteSegs: THREE.Vector3[] = [];
    const lostCrossSegs: THREE.Vector3[] = [];

    for (const convoy of snapshot.trackedConvoys) {
      const cell = cellMap.get(convoy.projectedCellId);
      const convoyUnit: Vec3 | null = convoy.currentUnitXyz
        ? [convoy.currentUnitXyz[0], convoy.currentUnitXyz[1], convoy.currentUnitXyz[2]]
        : cell?.centerUnit ?? null;
      if (!convoyUnit) continue;
      const statusLower = convoy.status.toLowerCase();
      const pos = new THREE.Vector3(
        convoyUnit[0] * 1.022,
        convoyUnit[1] * 1.022,
        convoyUnit[2] * 1.022,
      );

      if (statusLower.includes("lost") || statusLower.includes("missing")) {
        lostConvoyCount += 1;
        const n = new THREE.Vector3(...convoyUnit).normalize();
        const refAxis = Math.abs(n.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0);
        const east = new THREE.Vector3().crossVectors(refAxis, n).normalize().multiplyScalar(0.014);
        const north = new THREE.Vector3().crossVectors(n, east).normalize().multiplyScalar(0.014);
        lostCrossSegs.push(pos.clone().sub(east), pos.clone().add(east));
        lostCrossSegs.push(pos.clone().sub(north), pos.clone().add(north));
      } else if (statusLower.includes("overdue")) {
        overdueConvoyCount += 1;
        const marker = new THREE.Mesh(new THREE.BoxGeometry(0.020, 0.020, 0.020), this.convoyOverdueMat);
        marker.position.copy(pos);
        this.group.add(marker);
      } else if (statusLower.includes("confirmed") || statusLower.includes("live")) {
        confirmedConvoyCount += 1;
        const marker = new THREE.Mesh(new THREE.BoxGeometry(0.016, 0.016, 0.016), this.convoyConfirmedMat);
        marker.position.copy(pos);
        this.group.add(marker);
      } else {
        projectedConvoyCount += 1;
        const marker = new THREE.Mesh(new THREE.OctahedronGeometry(0.013, 0), this.convoyProjectedMat);
        marker.position.copy(pos);
        this.group.add(marker);
      }

      const waypoints = convoy.routeWaypointsXyz;
      if (waypoints && waypoints.length >= 2) {
        for (let i = 0; i + 1 < waypoints.length; i += 1) {
          const wpA: Vec3 = [waypoints[i][0], waypoints[i][1], waypoints[i][2]];
          const wpB: Vec3 = [waypoints[i + 1][0], waypoints[i + 1][1], waypoints[i + 1][2]];
          const arc = geodesicPointsBetween(wpA, wpB, 4, 1.016);
          for (let k = 0; k < arc.length - 1; k += 1) {
            dashedRouteSegs.push(arc[k], arc[k + 1]);
          }
        }
      } else {
        const route = convoy.routeCells ?? [];
        for (let i = 0; i + 1 < route.length; i += 1) {
          const src = cellMap.get(route[i]);
          const dst = cellMap.get(route[i + 1]);
          if (!src || !dst) continue;
          const arc = geodesicPointsBetween(src.centerUnit, dst.centerUnit, 6, 1.016);
          for (let k = 0; k < arc.length - 1; k += 1) {
            dashedRouteSegs.push(arc[k], arc[k + 1]);
          }
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

    // 6. Observed weather wind chevrons, flood rings, and snow/ice markers (`UI-MAP-004`)
    let windVectorCount = 0;
    let stormCellCount = 0;
    const windSegs: THREE.Vector3[] = [];
    const showWeatherDetails = activeModeId === "weather" || activeModeId === "terrain";

    if (showWeatherDetails) {
      for (const w of snapshot.observedWeather) {
        const cell = cellMap.get(w.cellId);
        if (!cell) continue;
        const u = w.windUMps ?? 0;
        const v = w.windVMps ?? 0;
        const windSpeed = Math.hypot(u, v);
        const flood = w.floodStage01 ?? 0;
        const snow = w.snowDepthM ?? 0;
        const n = new THREE.Vector3(...cell.centerUnit).normalize();

        if (windSpeed > 0.4) {
          windVectorCount += 1;
          const pole = Math.abs(n.y) < 0.9 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(1, 0, 0);
          const east = new THREE.Vector3().crossVectors(pole, n).normalize();
          const north = new THREE.Vector3().crossVectors(n, east).normalize();
          const dir = east.multiplyScalar(u / windSpeed).add(north.multiplyScalar(v / windSpeed)).normalize();
          const len = Math.min(0.036, 0.014 + windSpeed * 0.0014);
          const base = n.clone().multiplyScalar(1.020);
          const tip = base.clone().add(dir.clone().multiplyScalar(len));
          windSegs.push(base, tip);
        }

        if (w.precipMmHr > 1.5 || flood > 0.25) {
          stormCellCount += 1;
          if (activeModeId === "weather") {
            const floodGeom = new THREE.RingGeometry(0.015, 0.022, 16);
            const floodRing = new THREE.Mesh(floodGeom, this.floodRingMat);
            floodRing.position.copy(n.clone().multiplyScalar(1.015));
            floodRing.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
            this.group.add(floodRing);
          }
        }

        if (snow > 0.05 && activeModeId === "weather") {
          const snowGeom = new THREE.SphereGeometry(0.009, 8, 6);
          const snowMesh = new THREE.Mesh(snowGeom, this.snowMarkerMat);
          snowMesh.position.copy(n.clone().multiplyScalar(1.016));
          this.group.add(snowMesh);
        }
      }
      if (windSegs.length > 0) {
        const geom = new THREE.BufferGeometry().setFromPoints(windSegs);
        this.group.add(new THREE.LineSegments(geom, this.windVectorMat));
      }
    }

    // 7. M5 Trade ribbons & corridor congestion bottlenecks (`UI-MAP-006.1`, `UI-MAP-006.2`)
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
          const arc = geodesicPointsBetween(srcUnit, dstUnit, 12, 1.019);
          for (let i = 0; i < arc.length - 1; i += 1) {
            tradeSegs.push(arc[i], arc[i + 1]);
          }
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
          const arc = geodesicPointsBetween(cellA.centerUnit, cellB.centerUnit, 10, 1.019);
          for (let k = 0; k < arc.length - 1; k += 1) {
            tradeSegs.push(arc[k], arc[k + 1]);
          }
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
        const arc = geodesicPointsBetween(src.centerUnit, dst.centerUnit, 8, 1.021);
        for (let k = 0; k < arc.length - 1; k += 1) {
          congestedSegs.push(arc[k], arc[k + 1]);
        }
      }
      if (congestedSegs.length > 0) {
        const geom = new THREE.BufferGeometry().setFromPoints(congestedSegs);
        this.group.add(new THREE.LineSegments(geom, this.congestionMat));
      }
    }

    // 8. M7 Operational warfare theaters, phase lines, no-retreat postures & courier-lagged formations (`UI-MAP-007.1`, `UI-MAP-007.2`)
    let theaterVectorCount = 0;
    let formationCount = 0;
    let noRetreatCount = 0;
    if (activeModeId === "warfare" || activeModeId === "terrain") {
      const currentTick = snapshot.clockAndNotifications.tick;
      const phaseSegs: THREE.Vector3[] = [];
      for (const formation of snapshot.warfare?.formations ?? []) {
        formationCount += 1;
        const fUnit: Vec3 = [
          formation.currentUnitXyz[0],
          formation.currentUnitXyz[1],
          formation.currentUnitXyz[2],
        ];
        const ageTicks = Math.max(0, currentTick - formation.observedTick);
        const isFresh = ageTicks <= 6;
        const markerGeom = isFresh
          ? new THREE.BoxGeometry(0.017, 0.017, 0.017)
          : new THREE.OctahedronGeometry(0.015, 0);
        const markerMesh = new THREE.Mesh(
          markerGeom,
          isFresh ? this.formationFreshMat : this.formationStaleMat,
        );
        markerMesh.position.set(fUnit[0] * 1.023, fUnit[1] * 1.023, fUnit[2] * 1.023);
        this.group.add(markerMesh);

        if (activeModeId === "warfare") {
          if (formation.targetCellId !== formation.currentCellId) {
            const targetUnit: Vec3 = [
              formation.targetUnitXyz[0],
              formation.targetUnitXyz[1],
              formation.targetUnitXyz[2],
            ];
            const arc = geodesicPointsBetween(fUnit, targetUnit, 10, 1.022);
            for (let k = 0; k < arc.length - 1; k += 1) {
              phaseSegs.push(arc[k], arc[k + 1]);
            }
          }
          if (formation.noRetreatOrder) {
            noRetreatCount += 1;
            const nrRingGeom = new THREE.RingGeometry(0.020, 0.027, 16);
            const nrRing = new THREE.Mesh(nrRingGeom, this.noRetreatRingMat);
            const n = new THREE.Vector3(...fUnit).normalize();
            nrRing.position.copy(n.clone().multiplyScalar(1.024));
            nrRing.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
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
          const arcPts = geodesicPointsBetween(srcUnit, dstCell.centerUnit, 12, 1.024);
          const geom = new THREE.BufferGeometry().setFromPoints(arcPts);
          const line = new THREE.Line(geom, this.theaterVectorMat);
          line.computeLineDistances();
          this.group.add(line);
        }
      }
    }

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
      floodedRiverCount,
      commContourCount,
      tradeRibbonCount,
      congestedCorridorCount,
      theaterVectorCount,
      formationCount,
      noRetreatCount,
    };
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
    this.dashedRouteMat.dispose();
    this.previewRouteMat.dispose();
    this.windVectorMat.dispose();
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
