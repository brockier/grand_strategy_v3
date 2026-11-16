# Module Design: Geodesic 3D Globe, Orbital Camera & Cartographic Shaders

**Module ID:** `UI-GLB-02`  
**Code Location:** `code/ui/src/globe/`  
**Milestone & Design Depth:** `M2–M7: Full Detailed Design`  
**Satisfies Requirements:** `UI-GLB-001.1`, `UI-GLB-002`, `UI-GLB-003`, `UI-GLB-004`, `UI-GLB-005`, `UI-GLB-006`, `UI-GLB-007` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`selectGlobeTopology`, `selectKnownCells`, `selectKnownSettlements`)
  - `UiIntentBus` (`FlyGlobeToCell`, `SwitchMapOverlayMode`, `SwitchGlobeProjection`)
  - `SkinShaderPalettePort` (active color ramps, paper grain, ink hatching, terrain PBR textures)
  - Registered `MapLayerPlugin` instances from `MapLayerRegistry`
- **Provided Spine Ports (Outputs):**
  - `GlobeLayerHost` (Three.js scene hook, shared cell elevation lookup, camera altitude norm, active projection mode, `projectUnitToScreen`)
  - Continuous `cameraAltitudeNorm` telemetry stream to `UI-AUD-09`
  - Pointer raycast dispatch hooks consumed by `UI-MAP-03` in both 3D `globe` and 2D `equal_area` / `mercator` views

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Watertight Welded Subdivided Icosphere + Frustum Patch LOD (`UI-GLB-001.1`, `UI-GLB-006`)

- Builds base indexed `THREE.BufferGeometry` directly from `PlanetGeometryPort` topology transferred via `DiegeticStore::selectGlobeTopology()` (`m = 24`, `5,762` sample vertices, `11,520` outward-wound spherical triangles, Euler `V - E + F = 2`, zero hex/pentagon seams).
- **Frustum-Culled Spherical Patch LOD (`lod-patches.ts`):** Splits the globe into 20 icosahedral sector patches cached across 4 subdivision levels (`LOD 0` = `1x`, `LOD 1` = `4x`, `LOD 2` = `16x`, `LOD 3` = `64x` triangles near focus at low altitude) with continuous `aCornerCells` + interpolated `aBarycentric` weights and skirt rim vertices to prevent inter-LOD cracks.
- **No Voronoi Hex Boundaries:** Nearest-vertex winner selection is eliminated; every pixel interpolates elevation, climate, wetness, fertility, strata, and river plumes smoothly with sub-cell fractal coastal noise (`coastalWarp`) before testing land-vs-water (`h > 0`).
- Smooth circular cartographic reticles (`dot(normalize(vSpherePos), uHoveredUnitPos)`) mark hovered and selected continuous coordinates across both 3D and 2D views.

### 2.2 Quaternion Orbital Camera + Optical Telephoto Zoom & 2D Projections (`UI-GLB-002`, `UI-GLB-007`)

```typescript
interface GlobeCameraState {
  focusUnitNormal: THREE.Vector3; // Unit vector pointing to camera center on sphere
  northTangent: THREE.Vector3;    // Orthonormal heading vector
  altitudeRadius: number;         // Logarithmic zoom range [1.12 .. 3.60]
  fovDeg: number;                 // Continuous zoom narrowing 44 deg -> 16 deg below altNorm 0.22
  pitchRad: number;               // Clamped [0.0 .. 0.85]
  projectionMode: "globe" | "equal_area" | "mercator";
  projectionMorph01: number;      // Smooth 0 <-> 1 transition tween
  activeFlyTween: FlyTweenState | null;
}
```

### 2.3 Dual Double-Buffered GPU Upload State (`UI-GLB-005`)

Snapshot updates write updated cell bytes into two off-screen double-buffered `128x128` `RGBA8` staging textures:
- `uCellDataTex`: `[elevationByte, wetnessByte, biomeCodeByte, epistemicAlphaByte]` (strictly `[0, 0, 0, 0]` on `Unexplored`).
- `uBiophysicalTex`: `[tempByte, aridityByte, packedStrataFertilityByte, packedStrahlerPlumeByte]` (strictly `[0, 0, 0, 0]` on `Unexplored`).

---

## 3. Algorithms, Shaders & Zoom-Blend Pipeline

### 3.1 Satellite-First Biophysical PBR Shader + Optional Parchment Lens (`UI-GLB-003`, `UI-GLB-004`)
In [shaders.ts](file:///home/roger/Projects/ng/code/ui/src/globe/shaders.ts):
1. Compute `cartographicBlend = smoothstep(0.25, 0.75, altitudeNorm)` (`0 -> 1`).
2. **Vertex Relief & Projection Morph:** Displace vertex radial height by smooth interpolated elevation plus zoom-scaled spherical ridged fBm micro-relief (strictly masked by `knownGate = step(0.55, kSum)` so unexplored horizon remains smooth twilight curvature), then morph cleanly between 3D sphere coordinates and 2D `EqualArea` (`Lambert Azimuthal Equal-Area`) or `Mercator` coordinates via `uProjectionMorph`.
3. **Biophysical Satellite Surface Pass (`uParchmentMode < 0.5`, Default at All Zooms):**
   - Continuous Whittaker reflectance from `(tempC, aridity01, fertility01, slope, elevM)` matching ESA Sentinel-2 true-color imagery: Sahara ochre sand, Mediterranean scrub, Atlantic pasture-field mosaic, dark Nordic taiga, slope-exposed bedrock strata, and aspect/temperature snowpack & glaciers.
   - Shallow-to-abyssal water absorption with suspended teal-ochre river-mouth sediment plumes, capillary Fresnel sky reflection, and polar sea-ice floes.
   - Screen-space derivative / spherical gradient micro-bump lighting + Rayleigh blue limb glow scaled by `cartographicBlend`.
   - **Twilight Unmapped Globe + Cloud Feathering:** `kSum < 0.52` renders pure dark twilight unmapped planet with subtle graticules; `0.52 .. 0.86` feathers surveyed terrain softly through multi-octave cloud banks.
4. **Selectable Antique Parchment Lens (`uParchmentMode > 0.5`):** Cross-fades to aged paper grain, crisp `fwidth` contour isolines, `1px` shoreline ink, and slope cross-hatching at any altitude.

For complete patch LOD, hydrology ribbon, and 4-tier semantic zoom specifications, see [10_photorealistic_planet_and_semantic_zoom_lod.md](file:///home/roger/Projects/ng/design/ui/10_photorealistic_planet_and_semantic_zoom_lod.md).

### 3.2 Gimbal-Free Polar Navigation (`UI-GLB-002`)
1. Mouse drag computes incremental arcball quaternion around camera right/up vectors directly on `focusUnitNormal`, eliminating Euler pole singularities.
2. Pressing compass north-reset smoothly slerps `northTangent` back toward geographic North Pole meridian.
3. Clicking any hyperlinked coordinate (`FlyGlobeToCell`) interpolates along the sphere's shortest great-circle slerp geodesic while easing `altitudeRadius`.

---

## 4. Isolated Cartridge Verification Strategy

1. **Manifold Geometry Audit (`@req UI-GLB-001.1`):** Unit test builds welded subdivided icosphere mesh (`m = 8`), asserts `642` vertices, `1280` triangles, zero unmatched half-edges, Euler `V - E + F = 2`, zero hex border drawing in GLSL, zero NaN normals, and pole-to-pole continuity.
2. **Orbital & Polar Camera Tests (`@req UI-GLB-002`):** Playwright input tests drag across `+90` North Pole, tilt, reset north, zoom, and trigger `FlyGlobeToCell`, confirming finite non-NaN camera matrices and centered target coordinates.
3. **Relief & Parchment Golden Snapshots (`@req UI-GLB-003`, `@req UI-GLB-004`):** Captures WebGPU/WebGL2 canvas screenshots at `altitude = 1.15` (3D relief), `1.9` (mid-blend), and `3.2` (full parchment contours), asserting non-zero image diffs across transitions and structural similarity against golden baselines.
4. **60 FPS Decoupled Stress Benchmark (`@req UI-GLB-005`):** Streams 500 simulated `DiegeticSnapshot` updates per second while rotating camera and verifies 95th-percentile frame interval `<= 16.7ms`.

---

## 5. Lift-and-Shift Replacement Guide

1. `code/ui/src/globe/` can swap Three.js WebGPU for raw `wgpu`/WebGPU bindings or swap shader aesthetics completely without modifying window management, data ledgers, overlays, or audio.
2. Any replacement renderer MUST host registered `MapLayerPlugin` instances through `GlobeLayerHost`, emit `cameraAltitudeNorm`, and perform occlusion-checked cell picking.
