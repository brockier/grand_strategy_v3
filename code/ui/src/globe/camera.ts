import * as THREE from "three";
import { CameraTelemetry, Vec3 } from "../spine";

export const MIN_ALTITUDE_RADIUS = 1.12;
export const MAX_ALTITUDE_RADIUS = 3.6;

interface ActiveFlyTo {
  readonly startQuat: THREE.Quaternion;
  readonly endQuat: THREE.Quaternion;
  readonly startAltNorm: number;
  readonly endAltNorm: number;
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
 * both poles and antimeridian, tilt pitch, logarithmic altitude zoom, compass north
 * realignment, and great-circle geodesic fly-to transitions (`UI-GLB-001`, `UI-GLB-003`).
 */
export class GlobeOrbitalCamera {
  readonly camera: THREE.PerspectiveCamera;
  private focusQuat: THREE.Quaternion;
  private altitudeNorm: number;
  private pitchRad = 0.08;
  private flyTo: ActiveFlyTo | null = null;
  private focusCellId = 0;

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
    // Project world north onto tangent plane perpendicular to f
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

  /**
   * Pans camera continuously along sphere tangent directions without any polar stop.
   */
  panByPixels(deltaX: number, deltaY: number, viewportHeight = 800): void {
    this.cancelFlyTo();
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

    if (immediate || durationMs <= 0) {
      this.flyTo = null;
      this.focusQuat.copy(endQuat);
      this.altitudeNorm = clampedAlt;
      this.syncCameraTransform();
      return;
    }

    this.flyTo = {
      startQuat: this.focusQuat.clone(),
      endQuat,
      startAltNorm: this.altitudeNorm,
      endAltNorm: clampedAlt,
      elapsedMs: 0,
      durationMs,
    };
  }

  stepAnimation(deltaMs: number): boolean {
    if (!this.flyTo) return false;
    this.flyTo.elapsedMs += Math.max(0, deltaMs);
    const rawT = clamp(this.flyTo.elapsedMs / this.flyTo.durationMs, 0, 1);
    const eased = rawT < 0.5 ? 4 * rawT * rawT * rawT : 1 - Math.pow(-2 * rawT + 2, 3) / 2;

    this.focusQuat.slerpQuaternions(this.flyTo.startQuat, this.flyTo.endQuat, eased).normalize();
    this.altitudeNorm =
      this.flyTo.startAltNorm + (this.flyTo.endAltNorm - this.flyTo.startAltNorm) * eased;
    this.syncCameraTransform();

    if (rawT >= 1) {
      this.flyTo = null;
      return false;
    }
    return true;
  }

  private syncCameraTransform(): void {
    const f = new THREE.Vector3(0, 0, 1).applyQuaternion(this.focusQuat).normalize();
    const u = new THREE.Vector3(0, 1, 0).applyQuaternion(this.focusQuat).normalize();
    const right = new THREE.Vector3().crossVectors(u, f).normalize();

    const radius = this.getAltitudeRadius();
    const altAboveSurface = Math.max(0.05, radius - 1.0);
    // Tilt pivots camera direction slightly behind focus along -u tangent without touching focus point
    const tiltRatio = this.pitchRad * smoothstep01(1 - this.altitudeNorm);
    const camOffset = f
      .clone()
      .multiplyScalar(Math.cos(tiltRatio) * altAboveSurface)
      .add(u.clone().multiplyScalar(-Math.sin(tiltRatio) * altAboveSurface));

    const pos = f.clone().add(camOffset);
    // Ensure camera always remains safely outside unit sphere
    if (pos.length() < 1.04) {
      pos.normalize().multiplyScalar(1.04);
    }

    this.camera.position.copy(pos);
    const forwardLook = f.clone().sub(pos).normalize();
    const camUp = new THREE.Vector3().crossVectors(right, forwardLook).normalize();
    this.camera.up.copy(camUp.lengthSq() > 0.1 ? camUp : u);
    this.camera.lookAt(f);
    this.camera.updateMatrixWorld();
  }
}
