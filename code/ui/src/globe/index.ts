import * as THREE from "three";
import {
  add3,
  DiegeticSnapshot,
  DiegeticStoreAccess,
  generateGoldbergTopology,
  GlobeLayerHost,
  GlobePalette,
  GlobeProjectionMode,
  GlobeRayHit,
  GoldbergTopologyData,
  greatCircleRadians,
  MapLayerPlugin,
  norm3,
  scale3,
  sub3,
  tangentBasis,
  UiIntentBus,
  unprojectFlat2DToUnit,
  Vec3,
} from "../spine";
import { GlobeOrbitalCamera } from "./camera";
import { FeatureOverlayRenderSummary, GlobeFeatureOverlayGroup, MapLabelAnchor } from "./features";
import { createShaderTerrainSurfaceSampler, ShaderTerrainSurfaceSampler } from "./hydrology-ribbons";
import { GlobePatchLodManager } from "./lod-patches";
import { buildGoldbergGlobeGeometry, GoldbergMeshBuild, unitToLatLonDegrees, verifyGoldbergManifold } from "./mesh";
import {
  classifySemanticZoomTier,
  computeDeclusteredEntityOffsets,
  SemanticZoomTier,
} from "./semantic-zoom";
import { createGlobeShaderController, GlobeShaderController } from "./shaders";

export { buildGoldbergGlobeGeometry, unitToLatLonDegrees, verifyGoldbergManifold } from "./mesh";
export type { ManifoldVerificationReport } from "./mesh";
export { GlobeOrbitalCamera } from "./camera";
export { createGlobeShaderController } from "./shaders";
export { GlobeFeatureOverlayGroup } from "./features";
export type { FeatureOverlayRenderSummary, MapLabelAnchor } from "./features";
export { GlobePatchLodManager } from "./lod-patches";
export {
  classifySemanticZoomTier,
  computeDeclusteredEntityOffsets,
  SettlementDetailClusterGroup,
} from "./semantic-zoom";
export type { SemanticZoomTier } from "./semantic-zoom";

export interface FramePixelSample {
  readonly width: number;
  readonly height: number;
  readonly averageRgb: readonly [number, number, number];
  readonly rgba: Uint8Array;
}

export interface GlobeViewportStatus {
  readonly backend: "webgpu" | "webgl2" | "noop";
  readonly shaderCompiled: boolean;
  readonly cellCount: number;
  readonly pentagonCount: number;
  readonly p95FrameTimeMs: number;
  readonly lastFrameTimeMs: number;
  readonly activeDataTexIndex: number;
  readonly cartographicBlend: number;
  readonly activeOverlayMode: string;
  readonly projectionMode: GlobeProjectionMode;
  readonly semanticZoomTier: SemanticZoomTier;
  readonly patchLodCounts: readonly [number, number, number, number, number];
  readonly featureSummary: FeatureOverlayRenderSummary;
}

export interface GlobeViewportOptions {
  readonly initialPalette?: GlobePalette;
  readonly isPointOccluded?: (clientX: number, clientY: number) => boolean;
}

export interface GlobeViewportController {
  readonly status: GlobeViewportStatus;
  readonly camera: GlobeOrbitalCamera;
  readonly topology: GoldbergTopologyData;
  readonly layerHost: GlobeLayerHost;
  updateFromSnapshot(snapshot: DiegeticSnapshot): void;
  setSkinPalette(palette: GlobePalette): void;
  setHoveredCell(cellId: number | null): void;
  setSelectedCell(cellId: number | null): void;
  getMapLabelAnchors(): readonly MapLabelAnchor[];
  stepRender(dtMs?: number): void;
  captureFrameSample(sampleGridSize?: number): FramePixelSample | null;
  resize(width: number, height: number): void;
  dispose(): void;
}

function pointToSphericalSegmentDist(p: Vec3, a: Vec3, b: Vec3): number {
  const ab = sub3(b, a);
  const lenSq = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
  if (lenSq < 1e-9) return greatCircleRadians(p, a);
  const ap = sub3(p, a);
  const t = Math.max(0, Math.min(1, (ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / lenSq));
  const closest = norm3(add3(a, scale3(ab, t)));
  return greatCircleRadians(p, closest);
}

/**
 * Initializes the photorealistic spherical & 2D-projected Goldberg planetary renderer
 * (`UI-GLB-001`..`UI-GLB-007`) with `LOD 0..4` patch refinement and WebGL2 GLSL3 shading.
 */
export async function initializeGlobeViewport(
  canvas: HTMLCanvasElement,
  store: DiegeticStoreAccess,
  bus: UiIntentBus,
  layers: readonly MapLayerPlugin[],
  options: GlobeViewportOptions = {},
): Promise<GlobeViewportController> {
  let backend: "webgpu" | "webgl2" | "noop" = "noop";
  let shaderCompiled = false;
  let currentSnapshot: DiegeticSnapshot | null = store.getSnapshot();
  let subdivision = currentSnapshot?.globeTopology.subdivision ?? 3;
  let topology = generateGoldbergTopology(subdivision);
  let meshBuild: GoldbergMeshBuild = buildGoldbergGlobeGeometry(topology);
  let terrainSampler: ShaderTerrainSurfaceSampler | null = currentSnapshot
    ? createShaderTerrainSurfaceSampler(currentSnapshot, topology)
    : null;
  let lastFocusCellHint = 0;

  try {
    const navGpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown | null> } }).gpu;
    if (navGpu) {
      const adapter = await navGpu.requestAdapter();
      if (adapter) {
        backend = "webgpu";
      }
    }
  } catch {
    // Graceful WebGL2 fallback follows
  }

  const orbitalCamera = new GlobeOrbitalCamera(
    (canvas.clientWidth || 1280) / Math.max(canvas.clientHeight || 720, 1),
    0.62,
  );
  orbitalCamera.setTerrainReliefSampler((u) =>
    terrainSampler
      ? terrainSampler.sample(u, lastFocusCellHint, lastFocusCellHint, orbitalCamera.getAltitudeNorm())
          .vertexReliefHeight
      : 0,
  );
  const shaderCtrl: GlobeShaderController = createGlobeShaderController(options.initialPalette);
  const features = new GlobeFeatureOverlayGroup(options.initialPalette);
  const patchLod = new GlobePatchLodManager(topology, shaderCtrl.material);

  const selectionRingMat = new THREE.MeshBasicMaterial({
    color: new THREE.Color(
      ...(options.initialPalette?.peakSnowRgb ?? [244, 239, 228]).map((c) => c / 255) as [
        number,
        number,
        number,
      ],
    ).lerp(
      new THREE.Color(
        ...(options.initialPalette?.compassRoseRgb ?? [110, 85, 57]).map((c) => c / 255) as [
          number,
          number,
          number,
        ],
      ),
      0.38,
    ),
    side: THREE.DoubleSide,
    transparent: true,
    opacity: 0.85,
    depthTest: false,
  });
  const selectionRingMesh = new THREE.Mesh(
    new THREE.RingGeometry(0.0075, 0.0105, 28),
    selectionRingMat,
  );
  selectionRingMesh.visible = false;
  selectionRingMesh.renderOrder = 10;

  let hoveredCellId: number | null = null;
  let selectedCellId: number | null = null;
  let hoveredUnitXyz: Vec3 | null = null;
  let selectedUnitXyz: Vec3 | null = null;
  let suppressImmediateHoverRender = false;
  let activeOverlayMode = "terrain";
  let weatherOverlayActive = false;
  let previewDestinationCellId: number | null = null;
  let hasCenteredInitialColony = false;
  let lastScenarioIdentity: string | null = null;

  const overlayRootId = "globe-overlay-root";
  let overlayRoot = document.getElementById(overlayRootId);
  if (!overlayRoot && canvas.parentElement) {
    overlayRoot = document.createElement("div");
    overlayRoot.id = overlayRootId;
    overlayRoot.className = "ng-globe-overlay-root";
    canvas.parentElement.appendChild(overlayRoot);
  }

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let dirLight: THREE.DirectionalLight | null = null;
  let rafHandle = 0;
  let glContext: WebGL2RenderingContext | null = null;

  const sunDir = new THREE.Vector3(0.45, 0.72, 0.52).normalize();
  const gl = canvas.getContext("webgl2", {
    antialias: true,
    alpha: false,
    preserveDrawingBuffer: true,
  });

  if (gl) {
    glContext = gl;
    renderer = new THREE.WebGLRenderer({ canvas, context: gl, antialias: true });
    if (backend === "noop") {
      backend = "webgl2";
    }
    const width = canvas.clientWidth >= 64 ? canvas.clientWidth : canvas.width >= 64 ? canvas.width : 1280;
    const height = canvas.clientHeight >= 64 ? canvas.clientHeight : canvas.height >= 64 ? canvas.height : 720;
    renderer.setSize(width, height, false);
    orbitalCamera.setAspect(width, height);

    scene = new THREE.Scene();
    const hemiLight = new THREE.HemisphereLight(
      new THREE.Color(0.86, 0.90, 0.96),
      new THREE.Color(0.24, 0.21, 0.18),
      1.35,
    );
    dirLight = new THREE.DirectionalLight(new THREE.Color(1.0, 0.96, 0.90), 1.85);
    dirLight.position.copy(sunDir).multiplyScalar(8);
    scene.add(hemiLight);
    scene.add(dirLight);
    scene.add(patchLod.group);
    scene.add(features.group);
    scene.add(selectionRingMesh);

    renderer.compile(scene, orbitalCamera.camera);
    shaderCompiled = true;
  }

  const frameTimes: number[] = [];
  let lastFrameTs = typeof performance !== "undefined" ? performance.now() : Date.now();
  let lastFrameTimeMs = 0;

  const raycaster = new THREE.Raycaster();
  const isPointOccluded = (clientX: number, clientY: number): boolean => {
    if (options.isPointOccluded) return options.isPointOccluded(clientX, clientY);
    if (typeof document.elementsFromPoint !== "function") return false;
    const elements = document.elementsFromPoint(clientX, clientY);
    for (const el of elements) {
      if (el === canvas) break;
      if (el instanceof HTMLElement && el.closest("[data-chrome-occluder='true']")) {
        return true;
      }
    }
    return false;
  };

  const projectFeaturePoint = (unit: Vec3, lift: number): THREE.Vector3 =>
    orbitalCamera.projectUnitPosition(unit, lift);

  const findNearestCellId = (unit: Vec3): number => {
    if (terrainSampler) {
      lastFocusCellHint = terrainSampler.nearestCellId(unit, lastFocusCellHint);
      return lastFocusCellHint;
    }
    let bestId = 0;
    let bestDot = -2;
    for (const c of topology.cells) {
      const d =
        c.centerUnit[0] * unit[0] +
        c.centerUnit[1] * unit[1] +
        c.centerUnit[2] * unit[2];
      if (d > bestDot) {
        bestDot = d;
        bestId = c.cellId;
      }
    }
    return bestId;
  };

  const pickGlobeAtNdc = (ndcX: number, ndcY: number): GlobeRayHit | null => {
    raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), orbitalCamera.camera);
    const origin = raycaster.ray.origin;
    const dir = raycaster.ray.direction;

    const projMode = orbitalCamera.getProjectionMode();
    const isFlatView = projMode !== "globe" && orbitalCamera.getProjectionMorph() >= 0.5;
    const altNorm = orbitalCamera.getAltitudeNorm();

    // 1. Direct 3D mesh raycast against visible structures, facilities, settlements, convoys & formations (`UI-MAP-009`).
    features.group.updateMatrixWorld(true);
    const featureHits = raycaster.intersectObjects(features.group.children, true);
    for (const inter of featureHits) {
      if (inter.distance <= 0.004) continue;
      let curObj: THREE.Object3D | null = inter.object;
      let baseHit: GlobeRayHit | undefined;
      while (curObj) {
        if (curObj.userData?.pickHit) {
          baseHit = curObj.userData.pickHit as GlobeRayHit;
          break;
        }
        curObj = curObj.parent;
      }
      if (!baseHit) continue;
      if (!isFlatView) {
        const ptNorm = inter.point.clone().normalize();
        const toCam = orbitalCamera.camera.position.clone().sub(inter.point);
        if (ptNorm.dot(toCam) <= -0.02) continue;
      }
      const uVec: Vec3 = isFlatView
        ? unprojectFlat2DToUnit(
            inter.point.x,
            inter.point.y,
            projMode,
            orbitalCamera.getProjectionOriginUnit(),
          ) ?? [0, 0, 1]
        : norm3([inter.point.x, inter.point.y, inter.point.z]);
      const { latDeg, lonDeg } = unitToLatLonDegrees(uVec);
      return {
        ...baseHit,
        unitXyz: uVec,
        latDeg,
        lonDeg,
      };
    }

    let hitUnit: Vec3;
    if (isFlatView) {
      if (Math.abs(dir.z) < 1e-6) return null;
      const tPlane = -origin.z / dir.z;
      if (tPlane < 0) return null;
      const px = origin.x + dir.x * tPlane;
      const py = origin.y + dir.y * tPlane;
      const unproj = unprojectFlat2DToUnit(px, py, projMode, orbitalCamera.getProjectionOriginUnit());
      if (!unproj) return null;
      hitUnit = unproj;
    } else {
      const rMax = 1.1;
      const b = origin.dot(dir);
      const cOuter = origin.dot(origin) - rMax * rMax;
      const discOuter = b * b - cOuter;
      if (discOuter < 0) return null;
      const sqrtOuter = Math.sqrt(discOuter);
      const tEnter = Math.max(0, -b - sqrtOuter);

      const cInner = origin.dot(origin) - 1.0;
      const discInner = b * b - cInner;
      const tInnerHit = discInner >= 0 ? -b - Math.sqrt(discInner) : -1;
      const hasInnerHit = discInner >= 0 && tInnerHit >= 0;
      const tExit = hasInnerHit ? tInnerHit : Math.max(tEnter, -b + sqrtOuter);

      if (terrainSampler && tExit > tEnter) {
        const steps = 16;
        let prevT = tEnter;
        let marchCellHint = lastFocusCellHint;
        let foundUnit: Vec3 | null = null;

        for (let i = 0; i <= steps; i += 1) {
          const t = tEnter + ((tExit - tEnter) * i) / steps;
          const pt = origin.clone().addScaledVector(dir, t);
          const r = pt.length();
          if (r < 1e-6) continue;
          const u: Vec3 = [pt.x / r, pt.y / r, pt.z / r];
          marchCellHint = terrainSampler.nearestCellId(u, marchCellHint);
          const terrainR = 1.0 + terrainSampler.sample(u, marchCellHint, marchCellHint, altNorm).vertexReliefHeight;
          if (r <= terrainR) {
            if (i === 0) {
              foundUnit = u;
            } else {
              let lo = prevT;
              let hi = t;
              let bestU = u;
              for (let iter = 0; iter < 6; iter += 1) {
                const mid = (lo + hi) * 0.5;
                const mPt = origin.clone().addScaledVector(dir, mid);
                const mR = mPt.length();
                if (mR < 1e-6) break;
                const mU: Vec3 = [mPt.x / mR, mPt.y / mR, mPt.z / mR];
                marchCellHint = terrainSampler.nearestCellId(mU, marchCellHint);
                const mTerrR =
                  1.0 + terrainSampler.sample(mU, marchCellHint, marchCellHint, altNorm).vertexReliefHeight;
                if (mR <= mTerrR) {
                  hi = mid;
                  bestU = mU;
                } else {
                  lo = mid;
                }
              }
              foundUnit = bestU;
            }
            break;
          }
          prevT = t;
        }

        if (foundUnit) {
          hitUnit = foundUnit;
        } else if (hasInnerHit) {
          const hitPt = origin.clone().addScaledVector(dir, tExit).normalize();
          hitUnit = [hitPt.x, hitPt.y, hitPt.z];
        } else {
          return null;
        }
      } else {
        if (!hasInnerHit) return null;
        const hitPt = origin.clone().addScaledVector(dir, tExit).normalize();
        hitUnit = [hitPt.x, hitPt.y, hitPt.z];
      }
    }

    const { latDeg, lonDeg } = unitToLatLonDegrees(hitUnit);
    const nearestCellId = findNearestCellId(hitUnit);
    const snap = currentSnapshot ?? store.getSnapshot();
    const spacingRad = 2.1 / Math.sqrt(Math.max(topology.cellCount, 10) / 10);
    const zoomScale = Math.max(0.14, Math.min(1.0, 0.12 + 0.88 * altNorm));

    if (snap) {
      const declustered = computeDeclusteredEntityOffsets(snap, meshBuild.cellById, altNorm);

      // 2. Tracked convoys matched tightly at declustered coordinates (never hijacking whole cell)
      for (const convoy of snap.trackedConvoys) {
        const cCell = meshBuild.cellById.get(convoy.projectedCellId);
        const dec = declustered.convoys.get(convoy.convoyId);
        const cPos: Vec3 | null =
          dec?.displayUnit ??
          (convoy.currentUnitXyz
            ? [convoy.currentUnitXyz[0], convoy.currentUnitXyz[1], convoy.currentUnitXyz[2]]
            : cCell?.centerUnit ?? null);
        if (!cPos) continue;
        if (greatCircleRadians(hitUnit, cPos) < spacingRad * 0.22 * zoomScale) {
          return {
            hitKind: "convoy",
            cellId: convoy.projectedCellId,
            convoyId: convoy.convoyId,
            label: `${convoy.label} (${convoy.status})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 3. Declustered military formations matched tightly at rendered marker coordinate
      for (const formation of snap.warfare?.formations ?? []) {
        const dec = declustered.formations.get(formation.formationId);
        const fPos: Vec3 = dec?.displayUnit ?? [
          formation.currentUnitXyz[0],
          formation.currentUnitXyz[1],
          formation.currentUnitXyz[2],
        ];
        if (greatCircleRadians(hitUnit, fPos) < spacingRad * 0.22 * zoomScale) {
          return {
            hitKind: "formation",
            cellId: formation.currentCellId,
            formationId: formation.formationId,
            label: `${formation.label} (${formation.commanderName})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 4. Surveyed settlements matched at zoom-scaled beacon radius
      for (const settlement of snap.knownSettlements ?? []) {
        const sCell = meshBuild.cellById.get(settlement.cellId);
        if (!sCell) continue;
        if (greatCircleRadians(hitUnit, sCell.centerUnit) < spacingRad * 0.28 * zoomScale) {
          return {
            hitKind: "settlement",
            cellId: settlement.cellId,
            settlementId: settlement.settlementId,
            label: settlement.isHq ? `${settlement.settlementName} (HQ)` : settlement.settlementName,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 5. Surveyed constructed / traffic-worn roads
      for (const road of snap.surveyedRoads ?? []) {
        const aCell = meshBuild.cellById.get(road.fromCellId);
        const bCell = meshBuild.cellById.get(road.toCellId);
        if (!aCell || !bCell) continue;
        if (
          pointToSphericalSegmentDist(hitUnit, aCell.centerUnit, bCell.centerUnit) <
          spacingRad * 0.18 * zoomScale
        ) {
          return {
            hitKind: "road",
            cellId: nearestCellId,
            edgeId: road.edgeId,
            fromCellId: road.fromCellId,
            toCellId: road.toCellId,
            roadPurpose: road.roadPurpose,
            purposeLabel: road.purposeLabel,
            label: road.purposeLabel ? `${road.infraKey} — ${road.purposeLabel}` : `${road.infraKey} (Edge ${road.edgeId})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 6. Surveyed & Stale perennial rivers
      for (const cell of snap.knownCells) {
        if (cell.tier !== "surveyed" && cell.tier !== "stale") continue;
        if (cell.isOcean || cell.elevationM < 0) continue;
        const dsId =
          cell.riverChannelDownstreamCellId !== undefined
            ? cell.riverChannelDownstreamCellId
            : (cell.riverDownstreamCellId ?? null);
        if (dsId === null || dsId === undefined) continue;
        const aCell = meshBuild.cellById.get(cell.cellId);
        const bCell = meshBuild.cellById.get(dsId);
        if (!aCell || !bCell) continue;
        if (
          pointToSphericalSegmentDist(hitUnit, aCell.centerUnit, bCell.centerUnit) <
          spacingRad * 0.18 * zoomScale
        ) {
          return {
            hitKind: "river",
            cellId: cell.cellId,
            toCellId: dsId,
            flowNorm01: cell.riverFlowNorm01 ?? 0.5,
            dischargeM3S: cell.riverDischargeM3S,
            velocityMps: cell.riverVelocityMps,
            hydroPotentialKw: cell.hydroPotentialKw,
            label:
              cell.riverDischargeM3S !== undefined
                ? `River Course (${Math.round(cell.riverDischargeM3S)} m3/s -> Cell ${dsId})`
                : `River Course (Cell ${cell.cellId} -> ${dsId})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 7. Fallback cell respecting Unexplored epistemic air-gap
      const known = snap.knownCells.find((k) => k.cellId === nearestCellId);
      if (!known || known.tier === "unexplored") {
        return {
          hitKind: "cell",
          cellId: nearestCellId,
          label: `Unexplored Territory (Cell ${nearestCellId})`,
          unitXyz: hitUnit,
          latDeg,
          lonDeg,
        };
      }
      if (known.tier === "rumored") {
        return {
          hitKind: "cell",
          cellId: nearestCellId,
          label: `Rumored Terrain (Cell ${nearestCellId}, est. ${Math.round(known.elevationM)}m, ${Math.round(known.confidence01 * 100)}% conf)`,
          unitXyz: hitUnit,
          latDeg,
          lonDeg,
        };
      }
      const sampledSurf = terrainSampler?.sample(hitUnit, nearestCellId, nearestCellId, altNorm);
      const localElevM =
        sampledSurf && sampledSurf.isExplored
          ? Math.round(sampledSurf.effectiveElevM)
          : Math.round(known.elevationM);
      const centerElevM = Math.round(known.elevationM);

      if (known.tier === "stale") {
        return {
          hitKind: "cell",
          cellId: nearestCellId,
          localElevationM: localElevM,
          label: `Stale Recon (Cell ${nearestCellId}, ${localElevM}m)`,
          unitXyz: hitUnit,
          latDeg,
          lonDeg,
        };
      }
      const elevText =
        Math.abs(localElevM - centerElevM) >= 3
          ? `${localElevM}m local · ${centerElevM}m cell node`
          : `${localElevM}m`;
      return {
        hitKind: "cell",
        cellId: nearestCellId,
        localElevationM: localElevM,
        label: `${known.biomeKey ?? (known.isOcean ? "ocean" : "Surveyed")} (Cell ${nearestCellId}, ${elevText})`,
        unitXyz: hitUnit,
        latDeg,
        lonDeg,
      };
    }

    return {
      hitKind: "cell",
      cellId: nearestCellId,
      label: `Cell ${nearestCellId}`,
      unitXyz: hitUnit,
      latDeg,
      lonDeg,
    };
  };

  const projectUnitToScreen = (unit: readonly [number, number, number]) => {
    const mode = orbitalCamera.getProjectionMode();
    if (mode === "globe") {
      const camPos = orbitalCamera.camera.position;
      const surfaceVec = new THREE.Vector3(unit[0], unit[1], unit[2]);
      if (surfaceVec.dot(camPos.clone().sub(surfaceVec)) <= -0.04) {
        return { xPx: -1000, yPx: -1000, visible: false };
      }
    }
    const altNorm = orbitalCamera.getAltitudeNorm();
    const uVec: Vec3 = [unit[0], unit[1], unit[2]];
    const relief = terrainSampler
      ? terrainSampler.sample(uVec, lastFocusCellHint, lastFocusCellHint, altNorm).vertexReliefHeight
      : 0;
    const lift = (mode !== "globe" ? relief * 0.65 : relief) + 0.016;
    const worldPt = projectFeaturePoint(uVec, lift);
    const ndc = worldPt.project(orbitalCamera.camera);
    if (
      !Number.isFinite(ndc.x) ||
      !Number.isFinite(ndc.y) ||
      ndc.z < -1 ||
      ndc.z > 1 ||
      ndc.x < -1.06 ||
      ndc.x > 1.06 ||
      ndc.y < -1.06 ||
      ndc.y > 1.06
    ) {
      return { xPx: -1000, yPx: -1000, visible: false };
    }
    const width = canvas.clientWidth >= 64 ? canvas.clientWidth : canvas.width >= 64 ? canvas.width : 1280;
    const height = canvas.clientHeight >= 64 ? canvas.clientHeight : canvas.height >= 64 ? canvas.height : 720;
    return {
      xPx: (ndc.x * 0.5 + 0.5) * width,
      yPx: (-ndc.y * 0.5 + 0.5) * height,
      visible: true,
    };
  };

  let renderOnceImpl = () => {};

  const layerHost: GlobeLayerHost = {
    overlayRootId,
    getCanvasElement: () => canvas,
    pickGlobeAtNdc,
    isPointOccludedByChrome: isPointOccluded,
    projectUnitToScreen,
    getMapLabelAnchors: () => features.getMapLabelAnchors(),
    getProjectionMode: () => orbitalCamera.getProjectionMode(),
    setProjectionMode: (mode: GlobeProjectionMode) => {
      orbitalCamera.setProjectionMode(mode, true);
      if (currentSnapshot) {
        features.rebuild(
          currentSnapshot,
          topology,
          activeOverlayMode,
          previewDestinationCellId,
          orbitalCamera.getAltitudeNorm(),
          projectFeaturePoint,
          mode !== "globe",
          orbitalCamera.getFocusUnit(),
          terrainSampler ?? undefined,
        );
      }
      renderOnceImpl();
    },
  };

  let activeSelectionHit: GlobeRayHit | null = null;
  const zUpAxis = new THREE.Vector3(0, 0, 1);

  const syncSelectionRingMesh = () => {
    if (!activeSelectionHit || activeSelectionHit.hitKind === "cell") {
      selectionRingMesh.visible = false;
      return;
    }
    const fallbackCell = meshBuild.cellById.get(activeSelectionHit.cellId);
    const uVec: Vec3 | null = activeSelectionHit.unitXyz
      ? [activeSelectionHit.unitXyz[0], activeSelectionHit.unitXyz[1], activeSelectionHit.unitXyz[2]]
      : fallbackCell?.centerUnit ?? null;
    if (!uVec) {
      selectionRingMesh.visible = false;
      return;
    }
    const altNorm = orbitalCamera.getAltitudeNorm();
    const flatMode = orbitalCamera.getProjectionMode() !== "globe";
    const relief = terrainSampler
      ? terrainSampler.sample(uVec, activeSelectionHit.cellId, activeSelectionHit.cellId, altNorm).vertexReliefHeight
      : 0;
    const lift = (flatMode ? relief * 0.65 : relief) + 0.0055;
    selectionRingMesh.position.copy(projectFeaturePoint(uVec, lift));
    const surfNorm = flatMode ? zUpAxis : new THREE.Vector3(uVec[0], uVec[1], uVec[2]).normalize();
    selectionRingMesh.quaternion.setFromUnitVectors(zUpAxis, surfNorm);
    const baseRingScale =
      activeSelectionHit.hitKind === "settlement"
        ? 1.35
        : activeSelectionHit.hitKind === "facility"
          ? 0.55
          : 0.85;
    const densityScale = Math.sqrt(Math.max(1, topology.cellCount) / 642);
    const ringScale =
      baseRingScale * Math.max(0.28, (0.55 + 0.45 * altNorm) / Math.max(1, densityScale * 0.55));
    selectionRingMesh.scale.set(ringScale, ringScale, 1);
    selectionRingMesh.visible = true;
  };

  renderOnceImpl = () => {
    const altNorm = orbitalCamera.getAltitudeNorm();
    const blend = orbitalCamera.getCartographicBlend();
    const projMode = orbitalCamera.getProjectionMode();
    const projMorph = orbitalCamera.getProjectionMorph();
    const projOrigin = orbitalCamera.getProjectionOriginUnit();
    const focusUnit = orbitalCamera.getFocusUnit();

    patchLod.updateForCamera(focusUnit, altNorm, projMorph);
    syncSelectionRingMesh();

    orbitalCamera.camera.updateMatrixWorld();
    const mElem = orbitalCamera.camera.matrixWorld.elements;
    const camRight = new THREE.Vector3(mElem[0], mElem[1], mElem[2]);
    const camUp = new THREE.Vector3(mElem[4], mElem[5], mElem[6]);
    const camOut = new THREE.Vector3(mElem[8], mElem[9], mElem[10]);
    const sceneSun = camOut
      .clone()
      .multiplyScalar(0.66)
      .addScaledVector(camRight, 0.38)
      .addScaledVector(camUp, 0.65)
      .normalize();
    if (dirLight) {
      dirLight.position.copy(sceneSun).multiplyScalar(8);
    }
    if (projMode === "globe") {
      sunDir.copy(sceneSun);
    } else {
      const [east, north] = tangentBasis(projOrigin);
      const flatSun = norm3(
        add3(scale3(projOrigin, 0.66), add3(scale3(east, 0.38), scale3(north, 0.65))),
      );
      sunDir.set(flatSun[0], flatSun[1], flatSun[2]);
    }

    shaderCtrl.setCartographicState(
      blend,
      sunDir,
      hoveredCellId,
      selectedCellId,
      weatherOverlayActive,
      hoveredUnitXyz,
      selectedUnitXyz,
    );
    shaderCtrl.setAltitudeNorm?.(altNorm);
    shaderCtrl.setParchmentMode?.(activeOverlayMode === "parchment");
    shaderCtrl.setProjectionState?.(projMode, projMorph, projOrigin);

    features.refreshForCamera(
      altNorm,
      projectFeaturePoint,
      projMode !== "globe",
      focusUnit,
      terrainSampler ?? undefined,
    );

    const focusCellId = findNearestCellId(focusUnit);
    orbitalCamera.setFocusCellId(focusCellId);
    if (renderer && scene) {
      renderer.render(scene, orbitalCamera.camera);
    }
  };
  const renderOnce = () => renderOnceImpl();

  const emitTelemetry = () => {
    bus.emit({
      kind: "UpdateCameraTelemetry",
      telemetry: orbitalCamera.getTelemetry(),
    });
  };

  const heldNavKeys = new Set<string>();
  const isNavEditableTarget = (el: Element | null): boolean => {
    if (!el || !(el instanceof HTMLElement)) return false;
    const tag = el.tagName.toUpperCase();
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
  };

  const readNavAxesFromKeys = (): { forward: number; right: number; yaw: number } => {
    const forward = (heldNavKeys.has("w") ? 1 : 0) - (heldNavKeys.has("s") ? 1 : 0);
    const right = (heldNavKeys.has("d") ? 1 : 0) - (heldNavKeys.has("a") ? 1 : 0);
    const yaw = (heldNavKeys.has("q") ? 1 : 0) - (heldNavKeys.has("e") ? 1 : 0);
    return { forward, right, yaw };
  };

  const stepRender = (dtMs?: number) => {
    const now = typeof performance !== "undefined" ? performance.now() : Date.now();
    const delta = dtMs ?? Math.max(0.1, now - lastFrameTs);
    lastFrameTs = now;
    lastFrameTimeMs = delta;
    frameTimes.push(delta);
    if (frameTimes.length > 120) frameTimes.shift();

    const { forward, right, yaw } = readNavAxesFromKeys();
    const keyActive = orbitalCamera.stepKeyboardNavigation(forward, right, yaw, delta);
    const camActive = orbitalCamera.stepAnimation(delta) || keyActive;
    if (camActive) {
      orbitalCamera.camera.updateMatrixWorld();
      suppressImmediateHoverRender = true;
      try {
        emitTelemetry();
      } finally {
        suppressImmediateHoverRender = false;
      }
    }

    suppressImmediateHoverRender = true;
    try {
      for (const layer of layers) {
        layer.onFrameRender(orbitalCamera.getAltitudeNorm(), delta / 1000);
      }
    } finally {
      suppressImmediateHoverRender = false;
    }
    renderOnce();
  };

  const rafLoop = () => {
    stepRender();
    rafHandle = requestAnimationFrame(rafLoop);
  };

  if (renderer && scene) {
    renderOnce();
    if (typeof requestAnimationFrame === "function") {
      rafHandle = requestAnimationFrame(rafLoop);
    }
  }

  for (const layer of layers) {
    layer.mount(layerHost, store, bus);
  }

  // Canvas pointer & wheel interaction (`UI-GLB-001`, `UI-WIN-005`)
  let activePointerId: number | null = null;
  let dragButton = 0;
  let lastX = 0;
  let lastY = 0;

  const onPointerDown = (ev: PointerEvent) => {
    if (isPointOccluded(ev.clientX, ev.clientY)) return;
    activePointerId = ev.pointerId;
    dragButton = ev.button;
    lastX = ev.clientX;
    lastY = ev.clientY;
    canvas.setPointerCapture?.(ev.pointerId);
  };

  const onPointerMove = (ev: PointerEvent) => {
    if (activePointerId !== ev.pointerId) return;
    const dx = ev.clientX - lastX;
    const dy = ev.clientY - lastY;
    lastX = ev.clientX;
    lastY = ev.clientY;
    if (dragButton === 2 || ev.shiftKey) {
      orbitalCamera.tiltByPixels(dy);
      orbitalCamera.rotateYawByRadians(-dx * 0.0055);
    } else {
      orbitalCamera.panByPixels(dx, dy, canvas.clientHeight || 720);
    }
    renderOnce();
    emitTelemetry();
  };

  const clearPointer = (ev: PointerEvent) => {
    if (activePointerId !== ev.pointerId) return;
    activePointerId = null;
    if (canvas.hasPointerCapture?.(ev.pointerId)) {
      canvas.releasePointerCapture(ev.pointerId);
    }
  };

  const onWheel = (ev: WheelEvent) => {
    if (isPointOccluded(ev.clientX, ev.clientY)) return;
    ev.preventDefault();
    const pixelDelta =
      ev.deltaMode === 1 ? ev.deltaY * 32 : ev.deltaMode === 2 ? ev.deltaY * 320 : ev.deltaY;
    const deltaNorm = Math.max(-0.12, Math.min(0.12, pixelDelta * 0.00045));

    const rect = typeof canvas.getBoundingClientRect === "function" ? canvas.getBoundingClientRect() : null;
    const w = rect && rect.width >= 64 ? rect.width : canvas.clientWidth >= 64 ? canvas.clientWidth : 1280;
    const h = rect && rect.height >= 64 ? rect.height : canvas.clientHeight >= 64 ? canvas.clientHeight : 720;
    const left = rect ? rect.left : 0;
    const top = rect ? rect.top : 0;
    const ndcX = ((ev.clientX - left) / w) * 2 - 1;
    const ndcY = -(((ev.clientY - top) / h) * 2 - 1);
    const cursorHit = deltaNorm < 0 ? pickGlobeAtNdc(ndcX, ndcY) : null;

    orbitalCamera.zoomSmoothByWheelDelta(
      deltaNorm,
      cursorHit ? [cursorHit.unitXyz[0], cursorHit.unitXyz[1], cursorHit.unitXyz[2]] : null,
    );
    if (!rafHandle) {
      stepRender(16);
    } else {
      emitTelemetry();
    }
  };

  const onContextMenu = (ev: MouseEvent) => {
    ev.preventDefault();
  };

  const onNavKeyDown = (ev: KeyboardEvent) => {
    if (ev.key === "Escape") {
      bus.emit({ kind: "SelectMapFeature", hit: null });
      return;
    }
    if (ev.ctrlKey || ev.altKey || ev.metaKey) return;
    const ownerDoc = canvas.ownerDocument;
    if (isNavEditableTarget(ownerDoc?.activeElement ?? null)) return;
    const k = ev.key.toLowerCase();
    if (k === "w" || k === "a" || k === "s" || k === "d" || k === "q" || k === "e") {
      heldNavKeys.add(k);
    }
  };

  const onNavKeyUp = (ev: KeyboardEvent) => {
    heldNavKeys.delete(ev.key.toLowerCase());
  };

  const clearNavKeys = () => {
    heldNavKeys.clear();
  };

  const winRef = canvas.ownerDocument?.defaultView ?? (typeof window !== "undefined" ? window : null);
  winRef?.addEventListener("keydown", onNavKeyDown);
  winRef?.addEventListener("keyup", onNavKeyUp);
  winRef?.addEventListener("blur", clearNavKeys);

  canvas.addEventListener("pointerdown", onPointerDown);
  canvas.addEventListener("pointermove", onPointerMove);
  canvas.addEventListener("pointerup", clearPointer);
  canvas.addEventListener("lostpointercapture", clearPointer);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  canvas.addEventListener("contextmenu", onContextMenu);

  const rebuildCurrentFeatures = () => {
    if (!currentSnapshot) return;
    features.rebuild(
      currentSnapshot,
      topology,
      activeOverlayMode,
      previewDestinationCellId,
      orbitalCamera.getAltitudeNorm(),
      projectFeaturePoint,
      orbitalCamera.getProjectionMode() !== "globe",
      orbitalCamera.getFocusUnit(),
      terrainSampler ?? undefined,
    );
  };

  const unsubBus = bus.subscribe((intent) => {
    if (intent.kind === "FlyGlobeToCell") {
      const target = meshBuild.cellById.get(intent.cellId);
      if (target) {
        selectedCellId = intent.cellId;
        selectedUnitXyz = null;
        orbitalCamera.flyToUnitVector(target.centerUnit, intent.zoomAltitude ?? 0.2, 500, false);
        renderOnce();
        emitTelemetry();
      }
    } else if (intent.kind === "ResetGlobeNorth") {
      orbitalCamera.resetNorth(false);
      renderOnce();
      emitTelemetry();
    } else if (intent.kind === "SkinBundleUpdated") {
      shaderCtrl.setPalette(intent.bundle.globePalette);
      features.setPalette(intent.bundle.globePalette);
      renderOnce();
    } else if (intent.kind === "SwitchMapOverlayMode") {
      activeOverlayMode = intent.modeId;
      weatherOverlayActive = intent.modeId === "weather";
      rebuildCurrentFeatures();
      renderOnce();
    } else if (intent.kind === "SwitchGlobeProjection") {
      orbitalCamera.setProjectionMode(intent.projection, true);
      rebuildCurrentFeatures();
      renderOnce();
      emitTelemetry();
    } else if (intent.kind === "PreviewHqLatencyRoute") {
      previewDestinationCellId = intent.destinationCellId;
      rebuildCurrentFeatures();
      renderOnce();
    } else if (intent.kind === "HoverMapFeature") {
      hoveredCellId = intent.hit?.cellId ?? null;
      hoveredUnitXyz = intent.hit?.unitXyz
        ? [intent.hit.unitXyz[0], intent.hit.unitXyz[1], intent.hit.unitXyz[2]]
        : hoveredCellId !== null
          ? meshBuild.cellById.get(hoveredCellId)?.centerUnit ?? null
          : null;
      if (!suppressImmediateHoverRender) {
        renderOnce();
      }
    } else if (intent.kind === "SelectMapFeature") {
      activeSelectionHit = intent.hit;
      if (!intent.hit || intent.hit.hitKind === "cell") {
        // Clear surface reticle & selection ring so clicking empty terrain/ocean leaves zero lingering yellow circle.
        selectedCellId = null;
        selectedUnitXyz = null;
      } else {
        selectedCellId = intent.hit.cellId;
        selectedUnitXyz = null;
      }
      renderOnce();
    }
  });

  const updateFromSnapshot = (snapshot: DiegeticSnapshot) => {
    currentSnapshot = snapshot;
    if (snapshot.globeTopology.subdivision !== subdivision && snapshot.globeTopology.subdivision >= 1) {
      subdivision = snapshot.globeTopology.subdivision;
      topology = generateGoldbergTopology(subdivision);
      meshBuild.geometry.dispose();
      meshBuild = buildGoldbergGlobeGeometry(topology);
      patchLod.rebuildTopology(topology);
    }
    terrainSampler = createShaderTerrainSurfaceSampler(snapshot, topology);
    const scenarioIdentity = `${snapshot.scenarioKey}:${snapshot.globeTopology.cellCount}`;
    let centeredColonyNow = false;
    if (!hasCenteredInitialColony || lastScenarioIdentity !== scenarioIdentity) {
      const hqSettlement = snapshot.knownSettlements?.find((s) => s.isHq);
      const startCellId =
        snapshot.hqPresence?.cellId ??
        hqSettlement?.cellId ??
        snapshot.knownSettlements?.[0]?.cellId ??
        null;
      const startCell = startCellId !== null ? meshBuild.cellById.get(startCellId) : undefined;
      if (startCell) {
        hasCenteredInitialColony = true;
        lastScenarioIdentity = scenarioIdentity;
        orbitalCamera.flyToUnitVector(
          startCell.centerUnit,
          orbitalCamera.getAltitudeNorm(),
          0,
          true,
        );
        orbitalCamera.resetNorth(true);
        orbitalCamera.setFocusCellId(startCell.cellId);
        centeredColonyNow = true;
      }
    }
    orbitalCamera.refreshSurfaceTransform();
    shaderCtrl.uploadSnapshot(snapshot, selectedCellId, hoveredCellId);
    rebuildCurrentFeatures();
    for (const layer of layers) {
      layer.onSnapshotUpdate(snapshot);
    }
    renderOnce();
    if (centeredColonyNow) {
      emitTelemetry();
    }
  };

  if (currentSnapshot) {
    updateFromSnapshot(currentSnapshot);
  }
  emitTelemetry();

  const status: GlobeViewportStatus = {
    get backend() {
      return backend;
    },
    get shaderCompiled() {
      return shaderCompiled;
    },
    get cellCount() {
      return topology.cellCount;
    },
    get pentagonCount() {
      return topology.pentagonCount;
    },
    get p95FrameTimeMs() {
      if (frameTimes.length === 0) return 0;
      const sorted = [...frameTimes].sort((a, b) => a - b);
      const idx = Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95));
      return sorted[idx];
    },
    get lastFrameTimeMs() {
      return lastFrameTimeMs;
    },
    get activeDataTexIndex() {
      return shaderCtrl.activeTextureIndex();
    },
    get cartographicBlend() {
      return orbitalCamera.getCartographicBlend();
    },
    get activeOverlayMode() {
      return activeOverlayMode;
    },
    get projectionMode() {
      return orbitalCamera.getProjectionMode();
    },
    get semanticZoomTier() {
      return classifySemanticZoomTier(orbitalCamera.getAltitudeNorm());
    },
    get patchLodCounts() {
      return patchLod.getLodCounts();
    },
    get featureSummary() {
      return features.getSummary();
    },
  };

  return {
    status,
    camera: orbitalCamera,
    get topology() {
      return topology;
    },
    layerHost,
    updateFromSnapshot,
    setSkinPalette(palette: GlobePalette) {
      shaderCtrl.setPalette(palette);
      features.setPalette(palette);
      renderOnce();
    },
    setHoveredCell(cellId: number | null) {
      hoveredCellId = cellId;
      hoveredUnitXyz = cellId !== null ? meshBuild.cellById.get(cellId)?.centerUnit ?? null : null;
      renderOnce();
    },
    setSelectedCell(cellId: number | null) {
      selectedCellId = cellId;
      selectedUnitXyz = cellId !== null ? meshBuild.cellById.get(cellId)?.centerUnit ?? null : null;
      renderOnce();
    },
    getMapLabelAnchors() {
      return features.getMapLabelAnchors();
    },
    stepRender,
    captureFrameSample(sampleGridSize = 32): FramePixelSample | null {
      if (!glContext || !renderer || !scene) return null;
      renderOnce();
      const w = glContext.drawingBufferWidth;
      const h = glContext.drawingBufferHeight;
      if (w <= 0 || h <= 0) return null;
      const sampleW = Math.min(w, Math.max(4, sampleGridSize));
      const sampleH = Math.min(h, Math.max(4, sampleGridSize));
      const startX = Math.max(0, Math.floor((w - sampleW) / 2));
      const startY = Math.max(0, Math.floor((h - sampleH) / 2));
      const rgba = new Uint8Array(sampleW * sampleH * 4);
      glContext.readPixels(startX, startY, sampleW, sampleH, glContext.RGBA, glContext.UNSIGNED_BYTE, rgba);
      let rSum = 0;
      let gSum = 0;
      let bSum = 0;
      const n = sampleW * sampleH;
      for (let i = 0; i < rgba.length; i += 4) {
        rSum += rgba[i];
        gSum += rgba[i + 1];
        bSum += rgba[i + 2];
      }
      return {
        width: sampleW,
        height: sampleH,
        averageRgb: [rSum / n, gSum / n, bSum / n],
        rgba,
      };
    },
    resize(width: number, height: number) {
      if (!renderer) return;
      const w = width >= 64 ? width : 1280;
      const h = height >= 64 ? height : 720;
      orbitalCamera.setAspect(w, h);
      renderer.setSize(w, h, false);
      renderOnce();
    },
    dispose() {
      if (rafHandle && typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(rafHandle);
      }
      unsubBus();
      winRef?.removeEventListener("keydown", onNavKeyDown);
      winRef?.removeEventListener("keyup", onNavKeyUp);
      winRef?.removeEventListener("blur", clearNavKeys);
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", clearPointer);
      canvas.removeEventListener("lostpointercapture", clearPointer);
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("contextmenu", onContextMenu);
      meshBuild.geometry.dispose();
      selectionRingMesh.geometry.dispose();
      selectionRingMat.dispose();
      patchLod.dispose();
      shaderCtrl.dispose();
      features.dispose();
      renderer?.dispose();
    },
  };
}
