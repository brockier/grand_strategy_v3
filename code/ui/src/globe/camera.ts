import * as THREE from "three";
import {
  CameraTelemetry,
  GlobeProjectionMode,
  norm3,
  projectUnitToFlat2D,
  unprojectFlat2DToUnit,
  Vec3,
} from "../spine";

export const MIN_ALTITUDE_RADIUS = 1.12;
export const MAX_ALTITUDE_RADIUS = 3.6;

interface ActiveFlyTo {
  readonly startQuat: THREE.Quaternion;
  readonly endQuat: THREE.Quaternion;
  readonly startAltNorm: number;
  readonly endAltNorm: number;
  readonly startPan: readonly [number, number];
  readonly endPan: readonly [number, number];
  elapsedMs: number;
  readonly durationMs: number;
}

interface ActiveMorphTransition {
  readonly startMorph: number;
  readonly endMorph: number;
  elapsedMs: number;
  readonly durationMs: number;
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

function smoothstep01(t: number): number {
  const x = clamp(t, 0, 1);
  return x * x * (3 - 2 * x);
}

/**
 * Pole-free quaternion orbital camera supporting continuous arcball panning across
 * both poles and antimeridian, tilt pitch, logarithmic altitude zoom, dynamic close-up FOV,
 * compass north realignment, great-circle fly-to, and 2D Equal-Area/Mercator framing (`UI-GLB-001`..`007`).
 */
export class GlobeOrbitalCamera {
  readonly camera: THREE.PerspectiveCamera;
  private focusQuat: THREE.Quaternion;
  private altitudeNorm: number;
  private pitchRad = 0.08;
  private flyTo: ActiveFlyTo | null = null;
  private morphAnim: ActiveMorphTransition | null = null;
  private focusCellId = 0;
  private projectionMode: GlobeProjectionMode = "globe";
  private projectionMorph = 0;
  private projectionOriginUnit: Vec3 = [0, 0, 1];
  private flatPanXY: [number, number] = [0, 0];

  constructor(aspect = 16 / 9, initialAltitudeNorm = 0.62) {
    this.camera = new THREE.PerspectiveCamera(44, Math.max(aspect, 0.1), 0.02, 40);
    this.focusQuat = new THREE.Quaternion();
    this.altitudeNorm = clamp(initialAltitudeNorm, 0, 1);
    this.syncCameraTransform();
  }

  setAspect(width: number, height: number): void {
    this.camera.aspect = Math.max(width, 1) / Math.max(height, 1);
    this.camera.updateProjectionMatrix();
  }

  setFocusCellId(cellId: number): void {
    this.focusCellId = cellId;
  }

  getAltitudeNorm(): number {
    return this.altitudeNorm;
  }

  getAltitudeRadius(): number {
    return (
      MIN_ALTITUDE_RADIUS *
      Math.pow(MAX_ALTITUDE_RADIUS / MIN_ALTITUDE_RADIUS, this.altitudeNorm)
    );
  }

  getCartographicBlend(): number {
    return smoothstep01((this.altitudeNorm - 0.25) / 0.5);
  }

  getProjectionMode(): GlobeProjectionMode {
    return this.projectionMode;
  }

  getProjectionMorph(): number {
    return this.projectionMorph;
  }

  getProjectionOriginUnit(): Vec3 {
    return this.projectionOriginUnit;
  }

  setProjectionMode(mode: GlobeProjectionMode, immediate = true): void {
    const prevMode = this.projectionMode;
    this.projectionMode = mode;
    if (mode !== "globe") {
      if (prevMode === "globe" || prevMode !== mode) {
        if (prevMode !== "globe") {
          const unproj = unprojectFlat2DToUnit(
            this.flatPanXY[0],
            this.flatPanXY[1],
            prevMode,
            this.projectionOriginUnit,
          );
          if (unproj) {
            this.alignFocusUnitPreservingUp(unproj);
          }
        }
        const curFocus = this.getFocusUnit();
        this.projectionOriginUnit = curFocus;
        const projCenter = projectUnitToFlat2D(curFocus, mode, this.projectionOriginUnit);
        this.flatPanXY = projCenter.valid
          ? [clamp(projCenter.x, -1.42, 1.42), clamp(projCenter.y, -1.42, 1.42)]
          : [0, 0];
      }
      if (immediate) {
        this.morphAnim = null;
        this.projectionMorph = 1;
      } else {
        this.morphAnim = {
          startMorph: this.projectionMorph,
          endMorph: 1,
          elapsedMs: 0,
          durationMs: 360,
        };
      }
    } else {
      if (prevMode !== "globe") {
        const unproj = unprojectFlat2DToUnit(
          this.flatPanXY[0],
          this.flatPanXY[1],
          prevMode,
          this.projectionOriginUnit,
        );
        if (unproj) {
          this.alignFocusUnitPreservingUp(unproj);
        }
        this.flatPanXY = [0, 0];
      }
      if (immediate) {
        this.morphAnim = null;
        this.projectionMorph = 0;
      } else {
        this.morphAnim = {
          startMorph: this.projectionMorph,
          endMorph: 0,
          elapsedMs: 0,
          durationMs: 360,
        };
      }
    }
    this.syncCameraTransform();
  }

  projectUnitPosition(unit: Vec3, surfaceLift = 0): THREE.Vector3 {
    const u = norm3(unit);
    const spherePt = new THREE.Vector3(u[0], u[1], u[2]).multiplyScalar(1 + surfaceLift);
    if (this.projectionMode === "globe" || this.projectionMorph < 0.001) {
      return spherePt;
    }
    const flat = projectUnitToFlat2D(u, this.projectionMode, this.projectionOriginUnit);
    const flatPt = new THREE.Vector3(flat.x, flat.y, surfaceLift);
    return spherePt.lerp(flatPt, this.projectionMorph);
  }

  getFocusUnit(): Vec3 {
    const v = new THREE.Vector3(0, 0, 1).applyQuaternion(this.focusQuat).normalize();
    return [v.x, v.y, v.z];
  }

  getUpUnit(): Vec3 {
    const u = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();
    return [u.x, u.y, u.z];
  }

  getHeadingRadians(): number {
    const f = new THREE.Vector3(...this.getFocusUnit());
    const u = new THREE.Vector3(...this.getUpUnit());
    const worldNorth = new THREE.Vector3(0, 1, 0);
    const projNorth = worldNorth.clone().sub(f.clone().multiplyScalar(worldNorth.dot(f)));
    if (projNorth.lengthSq() < 1e-6) {
      return 0;
    }
    projNorth.normalize();
    const right = new THREE.Vector3().crossVectors(projNorth, f).normalize();
    return Math.atan2(u.dot(right), u.dot(projNorth));
  }

  getTelemetry(): CameraTelemetry {
    return {
      altitudeNorm: this.altitudeNorm,
      altitudeRadius: this.getAltitudeRadius(),
      pitchRad: this.pitchRad,
      headingRad: this.getHeadingRadians(),
      focusCellId: this.focusCellId,
      focusUnit: this.getFocusUnit(),
    };
  }

  cancelFlyTo(): void {
    this.flyTo = null;
  }

  private alignFocusUnitPreservingUp(targetUnit: Vec3): void {
    const startFocus = new THREE.Vector3(...this.getFocusUnit()).normalize();
    const destFocus = new THREE.Vector3(targetUnit[0], targetUnit[1], targetUnit[2]).normalize();
    const arcQuat = new THREE.Quaternion().setFromUnitVectors(startFocus, destFocus);
    this.focusQuat.premultiply(arcQuat).normalize();
  }

  /**
   * Pans camera continuously along sphere tangent directions (or 2D flat coordinates in flat modes).
   */
  panByPixels(deltaX: number, deltaY: number, viewportHeight = 800): void {
    this.cancelFlyTo();
    if (this.projectionMode !== "globe") {
      const spanScale = (0.35 + 1.55 * this.altitudeNorm) / Math.max(viewportHeight, 200);
      this.flatPanXY[0] = clamp(this.flatPanXY[0] - deltaX * spanScale, -1.42, 1.42);
      this.flatPanXY[1] = clamp(this.flatPanXY[1] + deltaY * spanScale, -1.42, 1.42);
      const unproj = unprojectFlat2DToUnit(
        this.flatPanXY[0],
        this.flatPanXY[1],
        this.projectionMode,
        this.projectionOriginUnit,
      );
      if (unproj) {
        this.alignFocusUnitPreservingUp(unproj);
      }
      this.syncCameraTransform();
      return;
    }

    const scale = (0.9 * this.altitudeNorm + 0.12) * (1.8 / Math.max(viewportHeight, 200));
    const rightAxis = new THREE.Vector3(1, 0, 0).applyQuaternion(this.focusQuat).normalize();
    const upAxis = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();

    const qYaw = new THREE.Quaternion().setFromAxisAngle(upAxis, -deltaX * scale);
    const qPitch = new THREE.Quaternion().setFromAxisAngle(rightAxis, -deltaY * scale);
    this.focusQuat.premultiply(qYaw).premultiply(qPitch).normalize();
    this.syncCameraTransform();
  }

  /**
   * Adjusts tilt angle away from zenith while keeping horizon clamp `[0..0.85 rad]`.
   */
  tiltByPixels(deltaY: number): void {
    this.cancelFlyTo();
    this.pitchRad = clamp(this.pitchRad - deltaY * 0.004, 0, 0.85);
    this.syncCameraTransform();
  }

  /**
   * Zooms logarithmically between orbital view (`1.0`) and tactical close surface (`0.0`).
   */
  zoomBySteps(steps: number): void {
    this.cancelFlyTo();
    this.altitudeNorm = clamp(this.altitudeNorm + steps * 0.08, 0, 1);
    this.syncCameraTransform();
  }

  setAltitudeNorm(altNorm: number): void {
    this.cancelFlyTo();
    this.altitudeNorm = clamp(altNorm, 0, 1);
    this.syncCameraTransform();
  }

  /**
   * Smoothly or immediately realigns local camera up toward planetary North while preserving focus point.
   */
  resetNorth(immediate = false): void {
    const f = new THREE.Vector3(...this.getFocusUnit());
    const refNorth = Math.abs(f.y) < 0.99 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(0, 0, -1);
    const tangentNorth = refNorth.clone().sub(f.clone().multiplyScalar(refNorth.dot(f))).normalize();
    const right = new THREE.Vector3().crossVectors(tangentNorth, f).normalize();
    const orthoUp = new THREE.Vector3().crossVectors(f, right).normalize();
    const m = new THREE.Matrix4().makeBasis(right, orthoUp, f);
    const endQuat = new THREE.Quaternion().setFromRotationMatrix(m).normalize();

    if (immediate) {
      this.flyTo = null;
      this.focusQuat.copy(endQuat);
      this.pitchRad = 0;
      this.syncCameraTransform();
    } else {
      this.flyTo = {
        startQuat: this.focusQuat.clone(),
        endQuat,
        startAltNorm: this.altitudeNorm,
        endAltNorm: this.altitudeNorm,
        startPan: [this.flatPanXY[0], this.flatPanXY[1]],
        endPan: [this.flatPanXY[0], this.flatPanXY[1]],
        elapsedMs: 0,
        durationMs: 320,
      };
    }
  }

  /**
   * Starts a spherical geodesic slerp flight to any target unit vector and altitude (`UI-GLB-003`).
   */
  flyToUnitVector(
    targetUnit: Vec3,
    targetAltitudeNorm = 0.22,
    durationMs = 520,
    immediate = false,
  ): void {
    const startFocus = new THREE.Vector3(...this.getFocusUnit()).normalize();
    const destFocus = new THREE.Vector3(targetUnit[0], targetUnit[1], targetUnit[2]).normalize();
    const arcQuat = new THREE.Quaternion().setFromUnitVectors(startFocus, destFocus);
    const endQuat = arcQuat.multiply(this.focusQuat.clone()).normalize();
    const clampedAlt = clamp(targetAltitudeNorm, 0, 1);

    let endPan: readonly [number, number] = [this.flatPanXY[0], this.flatPanXY[1]];
    if (this.projectionMode !== "globe") {
      const projected = projectUnitToFlat2D(targetUnit, this.projectionMode, this.projectionOriginUnit);
      if (projected.valid) {
        endPan = [clamp(projected.x, -1.42, 1.42), clamp(projected.y, -1.42, 1.42)];
      }
    }

    if (immediate || durationMs <= 0) {
      this.flyTo = null;
      this.focusQuat.copy(endQuat);
      this.altitudeNorm = clampedAlt;
      this.flatPanXY = [endPan[0], endPan[1]];
      this.syncCameraTransform();
      return;
    }

    this.flyTo = {
      startQuat: this.focusQuat.clone(),
      endQuat,
      startAltNorm: this.altitudeNorm,
      endAltNorm: clampedAlt,
      startPan: [this.flatPanXY[0], this.flatPanXY[1]],
      endPan,
      elapsedMs: 0,
      durationMs,
    };
  }

  stepAnimation(deltaMs: number): boolean {
    let active = false;
    let changed = false;
    if (this.morphAnim) {
      changed = true;
      this.morphAnim.elapsedMs += Math.max(0, deltaMs);
      const rawM = clamp(this.morphAnim.elapsedMs / this.morphAnim.durationMs, 0, 1);
      const easedM = smoothstep01(rawM);
      this.projectionMorph =
        this.morphAnim.startMorph + (this.morphAnim.endMorph - this.morphAnim.startMorph) * easedM;
      if (rawM >= 1) {
        this.morphAnim = null;
      } else {
        active = true;
      }
    }

    if (this.flyTo) {
      changed = true;
      this.flyTo.elapsedMs += Math.max(0, deltaMs);
      const rawT = clamp(this.flyTo.elapsedMs / this.flyTo.durationMs, 0, 1);
      const eased = rawT < 0.5 ? 4 * rawT * rawT * rawT : 1 - Math.pow(-2 * rawT + 2, 3) / 2;

      this.focusQuat.slerpQuaternions(this.flyTo.startQuat, this.flyTo.endQuat, eased).normalize();
      this.altitudeNorm =
        this.flyTo.startAltNorm + (this.flyTo.endAltNorm - this.flyTo.startAltNorm) * eased;
      this.flatPanXY = [
        this.flyTo.startPan[0] + (this.flyTo.endPan[0] - this.flyTo.startPan[0]) * eased,
        this.flyTo.startPan[1] + (this.flyTo.endPan[1] - this.flyTo.startPan[1]) * eased,
      ];

      if (rawT >= 1) {
        this.flyTo = null;
      } else {
        active = true;
      }
    }

    if (changed) {
      this.syncCameraTransform();
    }
    return active;
  }

  private syncCameraTransform(): void {
    // Narrow FOV gracefully in close settlement/regional zooms while keeping MIN/MAX radius invariant.
    const closeZoomT = smoothstep01((0.24 - this.altitudeNorm) / 0.24);
    const targetFov = 44 - closeZoomT * 30;
    if (Math.abs(targetFov - this.camera.fov) > 1e-4) {
      this.camera.fov = targetFov;
      this.camera.updateProjectionMatrix();
    }

    const f = new THREE.Vector3(0, 0, 1).applyQuaternion(this.focusQuat).normalize();
    const u = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();
    const right = new THREE.Vector3().crossVectors(u, f).normalize();

    const radius = this.getAltitudeRadius();
    const altAboveSurface = Math.max(0.05, radius - 1.0);
    const tiltRatio = this.pitchRad * smoothstep01(1 - this.altitudeNorm);
    const camOffset = f
      .clone()
      .multiplyScalar(Math.cos(tiltRatio) * altAboveSurface)
      .add(u.clone().multiplyScalar(-Math.sin(tiltRatio) * altAboveSurface));

    const spherePos = f.clone().add(camOffset);
    if (spherePos.length() < 1.04) {
      spherePos.normalize().multiplyScalar(1.04);
    }

    const forwardLook = f.clone().sub(spherePos).normalize();
    const sphereUpRaw = new THREE.Vector3().crossVectors(right, forwardLook).normalize();
    const sphereUp = sphereUpRaw.lengthSq() > 0.1 ? sphereUpRaw : u;

    if (this.projectionMorph > 0.001) {
      const flatZ = 0.28 + Math.pow(this.altitudeNorm, 1.1) * 3.25;
      const flatPos = new THREE.Vector3(this.flatPanXY[0], this.flatPanXY[1], flatZ);
      const flatTarget = new THREE.Vector3(this.flatPanXY[0], this.flatPanXY[1], 0);
      const flatUp = new THREE.Vector3(0, 1, 0);

      const m = this.projectionMorph;
      this.camera.position.copy(spherePos.lerp(flatPos, m));
      this.camera.up.copy(sphereUp.lerp(flatUp, m).normalize());
      this.camera.lookAt(f.clone().lerp(flatTarget, m));
    } else {
      this.camera.position.copy(spherePos);
      this.camera.up.copy(sphereUp);
      this.camera.lookAt(f);
    }
    this.camera.updateMatrixWorld();
  }
}
