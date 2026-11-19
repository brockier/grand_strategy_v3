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
  readonly startManualPitchOffset: number;
  readonly endManualPitchOffset: number;
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
 * both poles and antimeridian, inertial cursor-directed altitude zoom, automatic 0->45 deg
 * terrain-flyover pitch with manual offset, dynamic perspective near-plane tuning,
 * compass north realignment, great-circle fly-to, and 2D Equal-Area/Mercator framing (`UI-GLB-001`..`007`).
 */
export class GlobeOrbitalCamera {
  readonly camera: THREE.PerspectiveCamera;
  private focusQuat: THREE.Quaternion;
  private altitudeNorm: number;
  private targetAltitudeNorm: number;
  private zoomCursorTargetUnit: Vec3 | null = null;
  private manualPitchOffsetRad = 0;
  private terrainReliefSampler: ((unit: Vec3) => number) | null = null;
  private flyTo: ActiveFlyTo | null = null;
  private morphAnim: ActiveMorphTransition | null = null;
  private focusCellId = 0;
  private projectionMode: GlobeProjectionMode = "globe";
  private projectionMorph = 0;
  private projectionOriginUnit: Vec3 = [0, 0, 1];
  private flatPanXY: [number, number] = [0, 0];

  constructor(aspect = 16 / 9, initialAltitudeNorm = 0.62) {
    this.camera = new THREE.PerspectiveCamera(44, Math.max(aspect, 0.1), 0.015, 40);
    this.focusQuat = new THREE.Quaternion();
    this.altitudeNorm = clamp(initialAltitudeNorm, 0, 1);
    this.targetAltitudeNorm = this.altitudeNorm;
    this.syncCameraTransform();
  }

  setTerrainReliefSampler(sampler: ((unit: Vec3) => number) | null): void {
    this.terrainReliefSampler = sampler;
    this.syncCameraTransform();
  }

  refreshSurfaceTransform(): void {
    this.syncCameraTransform();
  }

  getAutoPitchRad(): number {
    return smoothstep01((0.45 - this.altitudeNorm) / 0.43) * (Math.PI / 4);
  }

  getEffectivePitchRad(): number {
    return clamp(this.getAutoPitchRad() + this.manualPitchOffsetRad, 0, 1.02);
  }

  getTargetAltitudeNorm(): number {
    return this.targetAltitudeNorm;
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
      pitchRad: this.getEffectivePitchRad(),
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
    this.targetAltitudeNorm = this.altitudeNorm;
    this.zoomCursorTargetUnit = null;
    if (this.projectionMode !== "globe") {
      const spanScale = (0.14 + 1.55 * this.altitudeNorm) / Math.max(viewportHeight, 200);
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

    const scale = (1.05 * this.altitudeNorm + 0.045) * (1.8 / Math.max(viewportHeight, 200));
    const rightAxis = new THREE.Vector3(1, 0, 0).applyQuaternion(this.focusQuat).normalize();
    const upAxis = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();

    const qYaw = new THREE.Quaternion().setFromAxisAngle(upAxis, -deltaX * scale);
    const qPitch = new THREE.Quaternion().setFromAxisAngle(rightAxis, -deltaY * scale);
    this.focusQuat.premultiply(qYaw).premultiply(qPitch).normalize();
    this.syncCameraTransform();
  }

  /**
   * Adjusts manual tilt offset relative to the altitude auto-pitch curve while keeping `[0 .. 1.02 rad]`.
   */
  tiltByPixels(deltaY: number): void {
    this.cancelFlyTo();
    const autoPitch = this.getAutoPitchRad();
    const desiredEffective = clamp(autoPitch + this.manualPitchOffsetRad - deltaY * 0.004, 0, 1.02);
    this.manualPitchOffsetRad = clamp(desiredEffective - autoPitch, -Math.PI / 4, 0.95);
    this.syncCameraTransform();
  }

  /**
   * Rotates camera heading yaw around the focus normal (`+deltaYawRad` = left / CCW, `-` = right / CW) (`UI-GLB-008`).
   */
  rotateYawByRadians(deltaYawRad: number): void {
    this.cancelFlyTo();
    if (!Number.isFinite(deltaYawRad) || Math.abs(deltaYawRad) < 1e-9) {
      return;
    }
    const focusAxis = new THREE.Vector3(0, 0, 1).applyQuaternion(this.focusQuat).normalize();
    const qYaw = new THREE.Quaternion().setFromAxisAngle(focusAxis, deltaYawRad);
    this.focusQuat.premultiply(qYaw).normalize();
    this.syncCameraTransform();
  }

  /**
   * Integrates held WASDQE heading-relative movement and yaw rotation cleanly over `deltaMs` (`UI-GLB-008`).
   */
  stepKeyboardNavigation(
    forwardInput: number,
    rightInput: number,
    yawInput: number,
    deltaMs: number,
  ): boolean {
    if (forwardInput === 0 && rightInput === 0 && yawInput === 0) {
      return false;
    }
    const dt = clamp(deltaMs, 0, 100) / 1000;
    if (dt <= 0) {
      return false;
    }

    this.cancelFlyTo();
    this.targetAltitudeNorm = this.altitudeNorm;
    this.zoomCursorTargetUnit = null;

    if (yawInput !== 0 && this.projectionMode === "globe") {
      const focusAxis = new THREE.Vector3(0, 0, 1).applyQuaternion(this.focusQuat).normalize();
      const qYaw = new THREE.Quaternion().setFromAxisAngle(focusAxis, yawInput * 1.65 * dt);
      this.focusQuat.premultiply(qYaw).normalize();
    }

    if (forwardInput !== 0 || rightInput !== 0) {
      const mag = Math.hypot(forwardInput, rightInput) || 1;
      const nf = forwardInput / mag;
      const nr = rightInput / mag;

      if (this.projectionMode !== "globe") {
        const flatSpeed = (0.16 + 1.35 * this.altitudeNorm) * dt;
        this.flatPanXY[0] = clamp(this.flatPanXY[0] + nr * flatSpeed, -1.42, 1.42);
        this.flatPanXY[1] = clamp(this.flatPanXY[1] + nf * flatSpeed, -1.42, 1.42);
        const unproj = unprojectFlat2DToUnit(
          this.flatPanXY[0],
          this.flatPanXY[1],
          this.projectionMode,
          this.projectionOriginUnit,
        );
        if (unproj) {
          this.alignFocusUnitPreservingUp(unproj);
        }
      } else {
        const panRad = (0.048 + 0.92 * this.altitudeNorm) * dt;
        const rightAxis = new THREE.Vector3(1, 0, 0).applyQuaternion(this.focusQuat).normalize();
        const upAxis = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();
        const qFwd = new THREE.Quaternion().setFromAxisAngle(rightAxis, -nf * panRad);
        const qSide = new THREE.Quaternion().setFromAxisAngle(upAxis, nr * panRad);
        this.focusQuat.premultiply(qSide).premultiply(qFwd).normalize();
      }
    }

    this.syncCameraTransform();
    return true;
  }

  /**
   * Queues an inertially smoothed wheel zoom impulse steered toward an optional world unit coordinate.
   */
  zoomSmoothByWheelDelta(deltaAltitudeNorm: number, cursorTargetUnit: Vec3 | null = null): void {
    this.cancelFlyTo();
    this.targetAltitudeNorm = clamp(this.targetAltitudeNorm + deltaAltitudeNorm, 0, 1);
    if (deltaAltitudeNorm < 0 && cursorTargetUnit) {
      this.zoomCursorTargetUnit = norm3(cursorTargetUnit);
    } else if (deltaAltitudeNorm > 0) {
      this.zoomCursorTargetUnit = null;
    }
  }

  /**
   * Steps altitude logarithmically between orbital view (`1.0`) and tactical close surface (`0.0`).
   */
  zoomBySteps(steps: number): void {
    this.cancelFlyTo();
    this.altitudeNorm = clamp(this.altitudeNorm + steps * 0.08, 0, 1);
    this.targetAltitudeNorm = this.altitudeNorm;
    this.zoomCursorTargetUnit = null;
    this.syncCameraTransform();
  }

  setAltitudeNorm(altNorm: number): void {
    this.cancelFlyTo();
    this.altitudeNorm = clamp(altNorm, 0, 1);
    this.targetAltitudeNorm = this.altitudeNorm;
    this.zoomCursorTargetUnit = null;
    this.syncCameraTransform();
  }

  /**
   * Smoothly or immediately realigns local camera up toward planetary North and resets manual pitch offset.
   */
  resetNorth(immediate = false): void {
    const f = new THREE.Vector3(...this.getFocusUnit());
    const refNorth = Math.abs(f.y) < 0.99 ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(0, 0, -1);
    const tangentNorth = refNorth.clone().sub(f.clone().multiplyScalar(refNorth.dot(f))).normalize();
    const right = new THREE.Vector3().crossVectors(tangentNorth, f).normalize();
    const orthoUp = new THREE.Vector3().crossVectors(f, right).normalize();
    const m = new THREE.Matrix4().makeBasis(right, orthoUp, f);
    const endQuat = new THREE.Quaternion().setFromRotationMatrix(m).normalize();

    this.zoomCursorTargetUnit = null;
    this.targetAltitudeNorm = this.altitudeNorm;

    if (immediate) {
      this.flyTo = null;
      this.focusQuat.copy(endQuat);
      this.manualPitchOffsetRad = 0;
      this.syncCameraTransform();
    } else {
      this.flyTo = {
        startQuat: this.focusQuat.clone(),
        endQuat,
        startAltNorm: this.altitudeNorm,
        endAltNorm: this.altitudeNorm,
        startManualPitchOffset: this.manualPitchOffsetRad,
        endManualPitchOffset: 0,
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

    this.zoomCursorTargetUnit = null;
    this.targetAltitudeNorm = clampedAlt;

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
      this.targetAltitudeNorm = clampedAlt;
      this.flatPanXY = [endPan[0], endPan[1]];
      this.syncCameraTransform();
      return;
    }

    this.flyTo = {
      startQuat: this.focusQuat.clone(),
      endQuat,
      startAltNorm: this.altitudeNorm,
      endAltNorm: clampedAlt,
      startManualPitchOffset: this.manualPitchOffsetRad,
      endManualPitchOffset: this.manualPitchOffsetRad,
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
      this.targetAltitudeNorm = this.altitudeNorm;
      this.manualPitchOffsetRad =
        this.flyTo.startManualPitchOffset +
        (this.flyTo.endManualPitchOffset - this.flyTo.startManualPitchOffset) * eased;
      this.flatPanXY = [
        this.flyTo.startPan[0] + (this.flyTo.endPan[0] - this.flyTo.startPan[0]) * eased,
        this.flyTo.startPan[1] + (this.flyTo.endPan[1] - this.flyTo.startPan[1]) * eased,
      ];

      if (rawT >= 1) {
        this.flyTo = null;
      } else {
        active = true;
      }
    } else if (Math.abs(this.targetAltitudeNorm - this.altitudeNorm) > 1e-5) {
      changed = true;
      const dt = Math.min(Math.max(0, deltaMs), 100);
      const alpha = 1 - Math.exp(-dt / 85);
      const prevAlt = this.altitudeNorm;
      const diff = this.targetAltitudeNorm - prevAlt;
      const nextAlt = Math.abs(diff) < 1e-4 ? this.targetAltitudeNorm : prevAlt + diff * alpha;
      this.altitudeNorm = nextAlt;

      if (nextAlt < prevAlt && this.zoomCursorTargetUnit) {
        const steerFrac = clamp((prevAlt - nextAlt) / Math.max(0.06, prevAlt + 0.02), 0, 0.35) * 0.65;
        if (this.projectionMode !== "globe") {
          const proj = projectUnitToFlat2D(
            this.zoomCursorTargetUnit,
            this.projectionMode,
            this.projectionOriginUnit,
          );
          if (proj.valid) {
            const tx = clamp(proj.x, -1.42, 1.42);
            const ty = clamp(proj.y, -1.42, 1.42);
            this.flatPanXY[0] = clamp(
              this.flatPanXY[0] + (tx - this.flatPanXY[0]) * steerFrac,
              -1.42,
              1.42,
            );
            this.flatPanXY[1] = clamp(
              this.flatPanXY[1] + (ty - this.flatPanXY[1]) * steerFrac,
              -1.42,
              1.42,
            );
            const unproj = unprojectFlat2DToUnit(
              this.flatPanXY[0],
              this.flatPanXY[1],
              this.projectionMode,
              this.projectionOriginUnit,
            );
            if (unproj) {
              this.alignFocusUnitPreservingUp(unproj);
            }
          }
        } else {
          const curFocus = new THREE.Vector3(...this.getFocusUnit()).normalize();
          const destFocus = new THREE.Vector3(
            this.zoomCursorTargetUnit[0],
            this.zoomCursorTargetUnit[1],
            this.zoomCursorTargetUnit[2],
          ).normalize();
          if (curFocus.distanceToSquared(destFocus) > 1e-8) {
            const fullArc = new THREE.Quaternion().setFromUnitVectors(curFocus, destFocus);
            const stepArc = new THREE.Quaternion().slerpQuaternions(
              new THREE.Quaternion(),
              fullArc,
              steerFrac,
            );
            this.focusQuat.premultiply(stepArc).normalize();
          }
        }
      }

      if (Math.abs(this.targetAltitudeNorm - this.altitudeNorm) <= 1e-4) {
        this.altitudeNorm = this.targetAltitudeNorm;
        this.zoomCursorTargetUnit = null;
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
    // Keep natural aerial perspective FOV (44 -> 38 deg) & refine near plane for close-up terrain.
    const closeZoomT = smoothstep01((0.28 - this.altitudeNorm) / 0.28);
    const targetFov = 44 - closeZoomT * 6;
    const targetNear = this.altitudeNorm < 0.35 ? 0.0015 : 0.015;
    if (
      Math.abs(targetFov - this.camera.fov) > 1e-4 ||
      Math.abs(targetNear - this.camera.near) > 1e-5
    ) {
      this.camera.fov = targetFov;
      this.camera.near = targetNear;
      this.camera.updateProjectionMatrix();
    }

    const f = new THREE.Vector3(0, 0, 1).applyQuaternion(this.focusQuat).normalize();
    const u = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();
    const right = new THREE.Vector3().crossVectors(u, f).normalize();

    const focusUnit: Vec3 = [f.x, f.y, f.z];
    const focusRelief = Math.max(0, this.terrainReliefSampler ? this.terrainReliefSampler(focusUnit) : 0);
    const focusTarget = f.clone().multiplyScalar(1.0 + focusRelief);

    const minVisualStandOff = 0.028;
    const maxVisualStandOff = MAX_ALTITUDE_RADIUS - 1.0;
    const visualStandOff =
      minVisualStandOff * Math.pow(maxVisualStandOff / minVisualStandOff, this.altitudeNorm);

    const effectivePitch = this.getEffectivePitchRad();
    const camOffset = f
      .clone()
      .multiplyScalar(Math.cos(effectivePitch) * visualStandOff)
      .add(u.clone().multiplyScalar(-Math.sin(effectivePitch) * visualStandOff));

    const spherePos = focusTarget.clone().add(camOffset);
    const camDir = spherePos.clone().normalize();
    const underCamRelief = Math.max(
      0,
      this.terrainReliefSampler ? this.terrainReliefSampler([camDir.x, camDir.y, camDir.z]) : 0,
    );
    const minSafeRadius = 1.0 + underCamRelief + 0.012;
    if (spherePos.length() < minSafeRadius) {
      spherePos.copy(camDir).multiplyScalar(minSafeRadius);
    }

    const forwardLook = focusTarget.clone().sub(spherePos).normalize();
    const sphereUpRaw = new THREE.Vector3().crossVectors(right, forwardLook).normalize();
    const sphereUp = sphereUpRaw.lengthSq() > 0.1 ? sphereUpRaw : u;

    if (this.projectionMorph > 0.001) {
      const flatZ = 0.16 + Math.pow(this.altitudeNorm, 1.15) * 3.35;
      const flatPos = new THREE.Vector3(this.flatPanXY[0], this.flatPanXY[1], flatZ);
      const flatTarget = new THREE.Vector3(this.flatPanXY[0], this.flatPanXY[1], focusRelief * 0.65);
      const flatUp = new THREE.Vector3(0, 1, 0);

      const m = this.projectionMorph;
      this.camera.position.copy(spherePos.lerp(flatPos, m));
      this.camera.up.copy(sphereUp.lerp(flatUp, m).normalize());
      this.camera.lookAt(focusTarget.clone().lerp(flatTarget, m));
    } else {
      this.camera.position.copy(spherePos);
      this.camera.up.copy(sphereUp);
      this.camera.lookAt(focusTarget);
    }
    this.camera.updateMatrixWorld();
  }
}
