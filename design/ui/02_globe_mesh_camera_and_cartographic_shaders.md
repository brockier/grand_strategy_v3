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
- **Frustum-Culled Spherical Patch LOD (`lod-patches.ts`):** Splits the globe into 20 icosahedral sector patches cached across 5 subdivision levels (`LOD 0` = `1x`, `LOD 1` = `4x`, `LOD 2` = `16x`, `LOD 3` = `64x`, `LOD 4` = `256x` triangles near focus at low altitude via `SUBDIV_PER_LOD = [1, 2, 4, 8, 16]`) with continuous `aCornerCells` + interpolated `aBarycentric` weights and skirt rim vertices to prevent inter-LOD cracks.
- **No Voronoi Hex Boundaries:** Nearest-vertex winner selection is eliminated; every pixel interpolates elevation, climate, wetness, fertility, strata, and river plumes smoothly with sub-cell fractal coastal noise (`coastalWarp`) before testing land-vs-water (`h > 0`).
- Smooth circular cartographic reticles (`dot(normalize(vSpherePos), uHoveredUnitPos)`) mark hovered continuous coordinates, while empty clicks never leave stale yellow rings and explicit feature selection highlights clear on `Escape`.

### 2.2 Quaternion Orbital Camera, `WASDQE` Navigation, Yaw Rotation & 2D Projections (`UI-GLB-002`, `UI-GLB-007`, `UI-GLB-008`)

```typescript
interface GlobeCameraState {
  focusUnitNormal: THREE.Vector3;     // Unit vector pointing to camera focus on sphere
  northTangent: THREE.Vector3;        // Orthonormal heading vector rotated by Q/E & yaw drag
  altitudeRadius: number;             // Logarithmic zoom range [1.12 .. 3.60]
  targetAltitudeNorm: number;         // Damped wheel-zoom destination in [0 .. 1]
  zoomCursorTargetUnit: Vec3 | null;  // Anchor unit coordinate under mouse cursor during zoom-in
  fovDeg: number;                     // Natural wide perspective 44 deg -> 38 deg below altNorm 0.28
  autoPitchRad: number;               // Smooth 0 deg (>= 0.45) -> 45 deg (<= 0.02) close-up tilt
  manualPitchOffsetRad: number;       // Vertical Right-Drag / Shift-Left-Drag pitch offset
  projectionMode: "globe" | "equal_area" | "mercator";
  projectionMorph01: number;          // Smooth 0 <-> 1 transition tween
  activeFlyTween: FlyTweenState | null;
}
```

- Horizontal `Right-Drag` / `Shift+Left-Drag` and held `Q`/`E` rotate `northTangent` smoothly around `focusUnitNormal` (`Q` counter-clockwise, `E` clockwise).
- Held `W`/`A`/`S`/`D` pan along camera-heading forward/left/backward/right orthogonal tangents without colliding with floating window shortcuts (`Ledger: P`, `Dispatches: J`, `Warfare: F`, `Alerts: V`, `Skins: Y`, `Controls (?): ? / F1`).

### 2.3 Dual Double-Buffered GPU Upload State (`UI-GLB-005`)

Snapshot updates write updated cell bytes into two off-screen double-buffered `128x128` `RGBA8` staging textures:
- `uCellDataTex`: `[elevationByte, wetnessByte, biomeCodeByte, epistemicAlphaByte]` (`biomeCodeByte` encodes lake cells cleanly for inland water shading; strictly `[0, 0, 0, 0]` on `Unexplored`).
- `uBiophysicalTex`: `[tempByte, aridityByte, packedStrataFertilityByte, packedStrahlerPlumeByte]` (strictly `[0, 0, 0, 0]` on `Unexplored`).

---

## 3. Algorithms, Shaders & Zoom-Blend Pipeline

### 3.1 Satellite-First Biophysical PBR Shader + Optional Parchment Lens (`UI-GLB-003`, `UI-GLB-004`)
In [shaders.ts](file:///home/roger/Projects/ng/code/ui/src/globe/shaders.ts):
1. Compute `cartographicBlend = smoothstep(0.25, 0.75, altitudeNorm)` (`0 -> 1`).
2. **Unified 3D Diorama Land Relief & Projection Morph:** Both `VERTEX_SHADER` (`uCellDataTex` + `uBiophysicalTex`) and the CPU surface sampler (`createShaderTerrainSurfaceSampler`) compute an identical altitude-invariant 3D displacement across all explored land (`effectiveElevM > 0`), combining coastal bluffs, concave hypsometric plains and plateaus, rolling 3D lowland hills, incised `strahler` river valleys, and craggy alpine peaks, while strictly masking unexplored horizon vertices to zero (`kSum < 0.35`). The vertex shader morphs cleanly between 3D sphere coordinates and 2D `EqualArea` (`Lambert Azimuthal Equal-Area`) or `Mercator` coordinates via `uProjectionMorph`.
3. **Biophysical Satellite Surface Pass (`uParchmentMode < 0.5`, Default at All Zooms):**
   - Continuous Whittaker reflectance from `(tempC, aridity01, fertility01, slope, elevM)` matching ESA Sentinel-2 true-color imagery: Sahara ochre sand, Mediterranean scrub, Atlantic pasture-field mosaic, dark Nordic taiga, slope-exposed bedrock strata, and aspect/temperature snowpack & glaciers.
   - Shallow-to-abyssal water absorption with suspended teal-ochre river-mouth sediment plumes, capillary Fresnel sky reflection, coastal surf foam lines at close zoom, and polar sea-ice floes.
   - World-space tangent normal perturbation aligned to `uSunDir` keeps terrain lighting invariant across `0 deg -> 45 deg` pitch and polar orbits, fading in close-up crop furrows, hedgerows, forest canopy crowns, and fractured cliff scree as altitude decreases.
   - **Twilight Unmapped Globe + Cloud Feathering:** `kSum < 0.52` renders pure dark twilight unmapped planet with subtle graticules; `0.52 .. 0.86` feathers surveyed terrain softly through multi-octave cloud banks.
4. **Selectable Antique Parchment Lens (`uParchmentMode > 0.5`):** Cross-fades to aged paper grain, crisp `fwidth` contour isolines, `1px` shoreline ink, and slope cross-hatching at any altitude.

For complete patch LOD, hydrology ribbon, and 4-tier semantic zoom specifications, see [10_photorealistic_planet_and_semantic_zoom_lod.md](file:///home/roger/Projects/ng/design/ui/10_photorealistic_planet_and_semantic_zoom_lod.md).

### 3.2 Gimbal-Free Polar Navigation, Inertial Cursor-Zoom & Terrain Clearance (`UI-GLB-002`)
1. Mouse drag computes incremental arcball quaternion around camera right/up vectors directly on `focusUnitNormal`, eliminating Euler pole singularities.
2. Wheel input normalizes `deltaY` across pixel/line/page modes, updates `targetAltitudeNorm`, picks the 3D ray-marched cursor target coordinate when zooming inward, and exponentially damps `altitudeNorm` + focus steering per animation frame while smoothly transitioning pitch from vertical (`0 deg`) to oblique terrain flyover (`45 deg`) with a terrain-clearance minimum radius guard and low-altitude `camera.near` refinement (`0.0015`).
3. Pressing compass north-reset smoothly slerps `northTangent` back toward geographic North Pole meridian and zeros `manualPitchOffsetRad` back to the default altitude-pitch curve.
4. Clicking any hyperlinked coordinate (`FlyGlobeToCell`) interpolates along the sphere's shortest great-circle slerp geodesic while easing `altitudeRadius`.

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
