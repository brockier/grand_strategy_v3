# Module Design: Geodesic 3D Globe, Orbital Camera & Cartographic Shaders

**Module ID:** `UI-GLB-02`  
**Code Location:** `apps/ui-shell/src/globe/`  
**Milestone & Design Depth:** `M2: Full Detailed Design`  
**Satisfies Requirements:** `UI-GLB-001`, `UI-GLB-002`, `UI-GLB-003`, `UI-GLB-004`, `UI-GLB-005` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`selectGlobeTopology`, `selectKnownCells`)
  - `UiIntentBus` (`FlyGlobeToCell`, `SwitchMapOverlayMode`)
  - `SkinShaderPalettePort` (active color ramps, paper grain, ink hatching, terrain PBR textures)
  - Registered `MapLayerPlugin` instances from `MapLayerRegistry`
- **Provided Spine Ports (Outputs):**
  - `GlobeLayerHost` (Three.js scene hook, shared cell elevation lookup, camera altitude norm)
  - Continuous `cameraAltitudeNorm` telemetry stream to `UI-AUD-09`
  - Pointer raycast dispatch hooks consumed by `UI-MAP-03`

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Watertight Goldberg Mesh Chunks (`UI-GLB-001`)

- Builds indexed `THREE.BufferGeometry` directly from `PlanetGeometryPort` topology transferred via `DiegeticStore::selectGlobeTopology()`:
  - Each hex or pentagon cell decomposes into a center vertex plus 5 or 6 corner rim vertices (`5` or `6` fan triangles per cell) plus vertical skirt walls between adjacent cells of different surveyed elevations so 3D extrusion never leaves floating gaps.
  - Partitioned into `20` icosahedral face chunks for frustum culling and crisp vertex attributes (`aCellId: float`, `aNormal: vec3`, `aElevationNorm: float`, `aBarycentric: vec3` for crisp anti-aliased hex borders and slope hatching).
  - Spherical UV mapping avoids equirectangular polar pinching by computing triplanar tangent-space procedural coordinates from 3D sphere normals inside shaders (`UI-GLB-001`).

### 2.2 Quaternion Orbital Camera Controller (`UI-GLB-002`)

```typescript
interface GlobeCameraState {
  focusUnitNormal: THREE.Vector3; // Unit vector pointing to camera center on sphere
  northTangent: THREE.Vector3;    // Orthonormal heading vector
  altitudeRadius: number;         // Logarithmic zoom range [1.08 .. 3.80]
  pitchRad: number;               // Clamped [0.0 .. 0.85]
  activeFlyTween: FlyTweenState | null;
}
```

### 2.3 Double-Buffered GPU Upload State (`UI-GLB-005`)

Snapshot updates write updated cell floats into off-screen `Float32Array` staging textures (`DataTexture`) and swap references between frames so `requestAnimationFrame` never stalls waiting on synchronous IPC deserialization or tick bursts.

---

## 3. Algorithms, Shaders & Zoom-Blend Pipeline

### 3.1 Continuous Physical-to-Cartographic Altitude Cross-Fade (`UI-GLB-003`, `UI-GLB-004`)
In the globe fragment/vertex shader (`WGSL` / `GLSL` unified via Three.js `ShaderMaterial` / TSL node material):
1. Compute `cartographicBlend = smoothstep(altReliefMax, altMapMin, cameraAltitude)`.
2. **Vertex Relief:** Displace vertex radial height by `mix(0.0, surveyedElevationScaled, 1.0 - cartographicBlend)` so mountains flatten smoothly into paper contours as camera zooms to orbit.
3. **Physical Terrain Pass (`1.0 - cartographicBlend`):** Triplanar-sampled PBR biome textures (bedrock, forest, grass, desert, snow), directional sun shadow lighting, dynamic bathymetry water waves, river channel shading, and instanced settlement/road splines.
4. **Cartographic Parchment Pass (`cartographicBlend`):**
   - Triplanar seamless aged parchment grain texture multiplied with muted skin palette tints.
   - Screen-space elevation contour lines via `fract(elevationMeters / contourStep)` modulated by `fwidth(elevationMeters)` so ink line thickness remains crisp at every zoom level.
   - Slope cross-hatching driven by surface gradient magnitude and compass rhumb-line graticules drawn in subtle ink opacity.

### 3.2 Gimbal-Free Polar Navigation (`UI-GLB-002`)
1. Mouse drag computes incremental arcball quaternion around camera right/up vectors directly on `focusUnitNormal`, eliminating Euler pole singularities.
2. Pressing compass north-reset smoothly slerps `northTangent` back toward geographic North Pole meridian.
3. Clicking any hyperlinked coordinate (`FlyGlobeToCell`) interpolates along the sphere's shortest great-circle slerp geodesic while easing `altitudeRadius`.

---

## 4. Isolated Cartridge Verification Strategy

1. **Manifold Geometry Audit (`@req UI-GLB-001`):** Unit test builds Goldberg mesh for `m = 16`, asserts exact `12` pentagons, closed vertex welding, zero NaN normals, and pole-to-pole triangle aspect limits.
2. **Orbital & Polar Camera Tests (`@req UI-GLB-002`):** Playwright input tests drag across `+90` North Pole, tilt, reset north, zoom, and trigger `FlyGlobeToCell`, confirming finite non-NaN camera matrices and centered target coordinates.
3. **Relief & Parchment Golden Snapshots (`@req UI-GLB-003`, `@req UI-GLB-004`):** Captures WebGPU/WebGL2 canvas screenshots at `altitude = 1.15` (3D relief), `1.9` (mid-blend), and `3.2` (full parchment contours), asserting non-zero image diffs across transitions and structural similarity against golden baselines.
4. **60 FPS Decoupled Stress Benchmark (`@req UI-GLB-005`):** Streams 500 simulated `DiegeticSnapshot` updates per second while rotating camera and verifies 95th-percentile frame interval `<= 16.7ms`.

---

## 5. Lift-and-Shift Replacement Guide

1. `apps/ui-shell/src/globe/` can swap Three.js WebGPU for raw `wgpu`/WebGPU bindings or swap shader aesthetics completely without modifying window management, data ledgers, overlays, or audio.
2. Any replacement renderer MUST host registered `MapLayerPlugin` instances through `GlobeLayerHost`, emit `cameraAltitudeNorm`, and perform occlusion-checked cell picking.
