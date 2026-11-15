import * as THREE from "three";
import {
  add3,
  DiegeticSnapshot,
  DiegeticStoreAccess,
  generateGoldbergTopology,
  GlobeLayerHost,
  GlobePalette,
  GlobeRayHit,
  GoldbergTopologyData,
  greatCircleRadians,
  MapLayerPlugin,
  norm3,
  scale3,
  sub3,
  UiIntentBus,
  Vec3,
} from "../spine";
import { GlobeOrbitalCamera } from "./camera";
import { FeatureOverlayRenderSummary, GlobeFeatureOverlayGroup } from "./features";
import { buildGoldbergGlobeGeometry, GoldbergMeshBuild, unitToLatLonDegrees, verifyGoldbergManifold } from "./mesh";
import { createGlobeShaderController, GlobeShaderController } from "./shaders";

export { buildGoldbergGlobeGeometry, unitToLatLonDegrees, verifyGoldbergManifold } from "./mesh";
export type { ManifoldVerificationReport } from "./mesh";
export { GlobeOrbitalCamera } from "./camera";
export { createGlobeShaderController } from "./shaders";
export type { FeatureOverlayRenderSummary } from "./features";

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
 * Initializes the spherical Goldberg planetary renderer (`UI-GLB-001`..`UI-GLB-005`),
 * probing WebGPU while compiling and rendering the WebGL2 GLSL3 shader pipeline.
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
  const shaderCtrl: GlobeShaderController = createGlobeShaderController(options.initialPalette);
  const features = new GlobeFeatureOverlayGroup(options.initialPalette);

  let hoveredCellId: number | null = null;
  let selectedCellId: number | null = null;
  let hoveredUnitXyz: Vec3 | null = null;
  let selectedUnitXyz: Vec3 | null = null;
  let activeOverlayMode = "terrain";
  let weatherOverlayActive = false;
  let previewDestinationCellId: number | null = null;

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
  let globeMesh: THREE.Mesh | null = null;
  let rafHandle = 0;
  let glContext: WebGL2RenderingContext | null = null;

  const frameTimes: number[] = [];
  let lastFrameTs = typeof performance !== "undefined" ? performance.now() : Date.now();
  let lastFrameTimeMs = 0;

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
    const width = Math.max(canvas.clientWidth || canvas.width || 1280, 1);
    const height = Math.max(canvas.clientHeight || canvas.height || 720, 1);
    renderer.setSize(width, height, false);
    orbitalCamera.setAspect(width, height);

    scene = new THREE.Scene();
    globeMesh = new THREE.Mesh(meshBuild.geometry, shaderCtrl.material);
    scene.add(globeMesh);
    scene.add(features.group);

    renderer.compile(scene, orbitalCamera.camera);
    shaderCompiled = true;
  }

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

  const findNearestCellId = (unit: Vec3): number => {
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
    const b = origin.dot(dir);
    const c = origin.dot(origin) - 1;
    const disc = b * b - c;
    if (disc < 0) return null;
    const t = -b - Math.sqrt(disc);
    if (t < 0) return null;

    const hitPt = origin.clone().addScaledVector(dir, t).normalize();
    const hitUnit: Vec3 = [hitPt.x, hitPt.y, hitPt.z];
    const { latDeg, lonDeg } = unitToLatLonDegrees(hitUnit);
    const nearestCellId = findNearestCellId(hitUnit);
    const snap = currentSnapshot ?? store.getSnapshot();
    const spacingRad = (2.1 / Math.sqrt(Math.max(topology.cellCount, 10) / 10));

    if (snap) {
      // 1. Tracked convoys highest priority (continuous coordinate aware)
      for (const convoy of snap.trackedConvoys) {
        const cCell = meshBuild.cellById.get(convoy.projectedCellId);
        const cPos: Vec3 | null = convoy.currentUnitXyz
          ? [convoy.currentUnitXyz[0], convoy.currentUnitXyz[1], convoy.currentUnitXyz[2]]
          : cCell?.centerUnit ?? null;
        if (!cPos) continue;
        if (
          greatCircleRadians(hitUnit, cPos) < spacingRad * 0.45 ||
          convoy.projectedCellId === nearestCellId
        ) {
          return {
            hitKind: "convoy",
            cellId: nearestCellId,
            convoyId: convoy.convoyId,
            label: `${convoy.label} (${convoy.status})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 2. Observed military formations (`UI-MAP-007.2`)
      for (const formation of snap.warfare?.formations ?? []) {
        const fPos: Vec3 = [
          formation.currentUnitXyz[0],
          formation.currentUnitXyz[1],
          formation.currentUnitXyz[2],
        ];
        if (
          greatCircleRadians(hitUnit, fPos) < spacingRad * 0.45 ||
          formation.currentCellId === nearestCellId
        ) {
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

      // 3. Surveyed settlements
      for (const settlement of snap.knownSettlements ?? []) {
        const sCell = meshBuild.cellById.get(settlement.cellId);
        if (!sCell) continue;
        if (
          settlement.cellId === nearestCellId ||
          greatCircleRadians(hitUnit, sCell.centerUnit) < spacingRad * 0.45
        ) {
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

      // 4. Surveyed roads
      for (const road of snap.surveyedRoads ?? []) {
        const aCell = meshBuild.cellById.get(road.fromCellId);
        const bCell = meshBuild.cellById.get(road.toCellId);
        if (!aCell || !bCell) continue;
        if (
          pointToSphericalSegmentDist(hitUnit, aCell.centerUnit, bCell.centerUnit) <
          spacingRad * 0.28
        ) {
          return {
            hitKind: "road",
            cellId: nearestCellId,
            edgeId: road.edgeId,
            fromCellId: road.fromCellId,
            toCellId: road.toCellId,
            label: `${road.infraKey} (Edge ${road.edgeId})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 5. Surveyed rivers
      for (const cell of snap.knownCells) {
        if (cell.tier !== "surveyed") continue;
        if (cell.riverDownstreamCellId === null || cell.riverDownstreamCellId === undefined) continue;
        const aCell = meshBuild.cellById.get(cell.cellId);
        const bCell = meshBuild.cellById.get(cell.riverDownstreamCellId);
        if (!aCell || !bCell) continue;
        if (
          pointToSphericalSegmentDist(hitUnit, aCell.centerUnit, bCell.centerUnit) <
          spacingRad * 0.25
        ) {
          return {
            hitKind: "river",
            cellId: cell.cellId,
            toCellId: cell.riverDownstreamCellId,
            flowNorm01: cell.riverFlowNorm01 ?? 0.5,
            label: `River Course (Cell ${cell.cellId})`,
            unitXyz: hitUnit,
            latDeg,
            lonDeg,
          };
        }
      }

      // 6. Fallback cell respecting Unexplored epistemic air-gap
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
      if (known.tier === "stale") {
        return {
          hitKind: "cell",
          cellId: nearestCellId,
          label: `Stale Recon (Cell ${nearestCellId})`,
          unitXyz: hitUnit,
          latDeg,
          lonDeg,
        };
      }
      return {
        hitKind: "cell",
        cellId: nearestCellId,
        label: `${known.biomeKey ?? (known.isOcean ? "ocean" : "Surveyed")} (Cell ${nearestCellId}, ${Math.round(known.elevationM)}m)`,
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

  const layerHost: GlobeLayerHost = {
    overlayRootId,
    getCanvasElement: () => canvas,
    pickGlobeAtNdc,
    isPointOccludedByChrome: isPointOccluded,
  };

  const sunDir = new THREE.Vector3(0.45, 0.72, 0.52).normalize();
  const renderOnce = () => {
    const blend = orbitalCamera.getCartographicBlend();
    shaderCtrl.setCartographicState(
      blend,
      sunDir,
      hoveredCellId,
      selectedCellId,
      weatherOverlayActive,
      hoveredUnitXyz,
      selectedUnitXyz,
    );
    const focusCellId = findNearestCellId(orbitalCamera.getFocusUnit());
    orbitalCamera.setFocusCellId(focusCellId);
    if (renderer && scene) {
      renderer.render(scene, orbitalCamera.camera);
    }
  };

  const emitTelemetry = () => {
    bus.emit({
      kind: "UpdateCameraTelemetry",
      telemetry: orbitalCamera.getTelemetry(),
    });
  };

  const stepRender = (dtMs?: number) => {
    const now = typeof performance !== "undefined" ? performance.now() : Date.now();
    const delta = dtMs ?? Math.max(0.1, now - lastFrameTs);
    lastFrameTs = now;
    lastFrameTimeMs = delta;
    frameTimes.push(delta);
    if (frameTimes.length > 120) frameTimes.shift();

    orbitalCamera.stepAnimation(delta);
    renderOnce();

    for (const layer of layers) {
      layer.onFrameRender(orbitalCamera.getAltitudeNorm(), delta / 1000);
    }
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
    const steps = ev.deltaY > 0 ? 1 : -1;
    orbitalCamera.zoomBySteps(steps);
    renderOnce();
    emitTelemetry();
  };

  const onContextMenu = (ev: MouseEvent) => {
    ev.preventDefault();
  };

  canvas.addEventListener("pointerdown", onPointerDown);
  canvas.addEventListener("pointermove", onPointerMove);
  canvas.addEventListener("pointerup", clearPointer);
  canvas.addEventListener("lostpointercapture", clearPointer);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  canvas.addEventListener("contextmenu", onContextMenu);

  const unsubBus = bus.subscribe((intent) => {
    if (intent.kind === "FlyGlobeToCell") {
      const target = meshBuild.cellById.get(intent.cellId);
      if (target) {
        selectedCellId = intent.cellId;
        orbitalCamera.flyToUnitVector(target.centerUnit, 0.2, 500, false);
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
      if (currentSnapshot) {
        features.rebuild(currentSnapshot, topology, activeOverlayMode, previewDestinationCellId);
      }
      renderOnce();
    } else if (intent.kind === "PreviewHqLatencyRoute") {
      previewDestinationCellId = intent.destinationCellId;
      if (currentSnapshot) {
        features.rebuild(currentSnapshot, topology, activeOverlayMode, previewDestinationCellId);
      }
      renderOnce();
    } else if (intent.kind === "HoverMapFeature") {
      hoveredCellId = intent.hit?.cellId ?? null;
      hoveredUnitXyz = intent.hit?.unitXyz
        ? [intent.hit.unitXyz[0], intent.hit.unitXyz[1], intent.hit.unitXyz[2]]
        : hoveredCellId !== null
          ? meshBuild.cellById.get(hoveredCellId)?.centerUnit ?? null
          : null;
      renderOnce();
    } else if (intent.kind === "SelectMapFeature") {
      selectedCellId = intent.hit?.cellId ?? null;
      selectedUnitXyz = intent.hit?.unitXyz
        ? [intent.hit.unitXyz[0], intent.hit.unitXyz[1], intent.hit.unitXyz[2]]
        : selectedCellId !== null
          ? meshBuild.cellById.get(selectedCellId)?.centerUnit ?? null
          : null;
      renderOnce();
    }
  });

  const updateFromSnapshot = (snapshot: DiegeticSnapshot) => {
    currentSnapshot = snapshot;
    if (snapshot.globeTopology.subdivision !== subdivision && snapshot.globeTopology.subdivision >= 1) {
      subdivision = snapshot.globeTopology.subdivision;
      topology = generateGoldbergTopology(subdivision);
      const nextBuild = buildGoldbergGlobeGeometry(topology);
      if (globeMesh) {
        meshBuild.geometry.dispose();
        globeMesh.geometry = nextBuild.geometry;
      }
      meshBuild = nextBuild;
    }
    shaderCtrl.uploadSnapshot(snapshot, selectedCellId, hoveredCellId);
    features.rebuild(snapshot, topology, activeOverlayMode, previewDestinationCellId);
    for (const layer of layers) {
      layer.onSnapshotUpdate(snapshot);
    }
    renderOnce();
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
      const w = Math.max(width, 1);
      const h = Math.max(height, 1);
      orbitalCamera.setAspect(w, h);
      renderer.setSize(w, h, false);
      renderOnce();
    },
    dispose() {
      if (rafHandle && typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(rafHandle);
      }
      unsubBus();
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", clearPointer);
      canvas.removeEventListener("lostpointercapture", clearPointer);
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("contextmenu", onContextMenu);
      meshBuild.geometry.dispose();
      shaderCtrl.dispose();
      features.dispose();
      renderer?.dispose();
    },
  };
}
