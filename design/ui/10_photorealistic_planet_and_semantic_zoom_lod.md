# Module Design: Photorealistic Planet Rendering, Projections & Semantic Zoom LOD

**Module ID:** `UI-GLB-10`  
**Code Location:** `code/ui/src/globe/lod-patches.ts`, `code/ui/src/globe/hydrology-ribbons.ts`, `code/ui/src/globe/semantic-zoom.ts`, `code/ui/src/globe/shaders.ts`  
**Milestone & Design Depth:** `M7: Full Detailed Design`  
**Satisfies Requirements:** `UI-GLB-003`, `UI-GLB-004`, `UI-GLB-006`, `UI-GLB-007`, `UI-MAP-008` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`KnownCellDto` biophysical fields, `KnownSettlementDto`, `SurveyedRoadEdgeDto`, `TrackedConvoyDto`, `ObservedWeatherDto`, `WarfareSnapshotDto`, `FacilityRowDto`, `PopulationCohortRowDto`)
  - `topology.ts` (`GoldbergTopologyData`, `projectUnitToFlat2D`, `unprojectFlat2DToUnit`, `greatCircleRadians`, `tangentBasis`, `slerpUnitVec3`)
  - `UiIntentBus` (`SwitchGlobeProjection`, `SwitchMapOverlayMode`, `UpdateCameraTelemetry`)
  - `SkinShaderPalettePort` (optional parchment lens and ink/paper skin palette values)
- **Provided Spine Ports (Outputs):**
  - Frustum-culled multi-tier LOD patch meshes (`GlobePatchLodManager`)
  - Curved fog-clipped hydrology & road triangle-strip ribbons (`buildHydrologyAndRoadRibbons`)
  - De-clustered lit 3D markers & Tier-3 settlement/encampment clusters (`buildSemanticZoomGroup`)
  - Screen-space projection metadata for DOM map labels (`LabelCandidate[]`)

---

## 2. Sub-Module Breakdown (Lift-and-Shift Units Inside `src/globe/`)

### 2.1 `lod-patches.ts` — Frustum-Culled Spherical Patch Quadtree LOD
- Groups base icosahedral triangles (`20 * m^2`, `11,520` at `m = 24`) into the 20 base icosahedron sector patches.
- Precomputes or caches subdivided `BufferGeometry` buffers at 4 resolution tiers while preserving `aCornerCells` (`vec3`) and linearly interpolated `aBarycentric` weights across every child vertex:
  - `LOD 0` (`1x` triangles per base face): orbital view (`altitudeNorm >= 0.65`) or back-hemisphere sectors
  - `LOD 1` (`4x` triangles per base face): sectors within `48 deg` of camera focus at `altitudeNorm < 0.65`
  - `LOD 2` (`16x` triangles per base face): focus sectors within `24 deg` at `altitudeNorm < 0.35`
  - `LOD 3` (`64x` triangles per base face): immediate camera focus sectors within `11 deg` at `altitudeNorm < 0.16`, adding small inward-angled patch boundary skirts to conceal T-junction seams
- Both vertex shader displacement and pixel bump micro-relief scale cleanly across subdivided vertices so zooming in continuously sharpens fjords, ridges, valleys, and cultivated micro-patchwork.

### 2.2 `shaders.ts` — Satellite Biophysical PBR & Parchment Dual-Pass Shader
- Samples double-buffered `uCellDataTex` (`[elev, wetness, biomeCode, alpha]`) and `uBiophysicalTex` (`[temp, aridity, strataFertility, strahlerPlume]`), interpolating across barycentric weights `w0, w1, w2` with zero nearest-neighbor Voronoi boundaries.
- **Sub-Cell Fractal Coastline (`coastalWarp`):** Adds multi-octave domain-warped fBm gated by `coastTransition` (`smoothstep(0.05, 0.32, landFrac) * smoothstep(0.05, 0.32, 1.0 - landFrac)`) and latitude (`|lat|` boosts fjord dissection) so coastal fractal displacement applies strictly across mixed land/ocean triangles before sea-level thresholding (`h > 0`), keeping pure-land cells and cell centers dry while eliminating hexagonal shoreline chords.
- **Dark Twilight Unmapped Globe + Cloud Feathering:** `kSum < 0.52` renders pure dark twilight spherical space/globe (strict zero leakage of unexplored cells); `0.52 <= kSum < 0.86` feathers surveyed terrain smoothly through multi-octave cloud fBm.
- **Biophysical Reflectance Synthesis:** Blends dry sand/erg, Mediterranean scrub, temperate lowland meadows & field-patch micro-grain, boreal spruce taiga, slope-exposed bedrock (`limestone`, `granite`, `basalt`), temperature-driven alpine snowpack, shallow-shelf turquoise -> deep abyssal navy ocean absorption, coastal river sediment plumes, capillary water glint, polar pack ice, Rayleigh horizon glow, and optional `Parchment` chart lens (`uParchmentMode`).
- **3D <-> 2D Projection Morph (`uProjectionMode`, `uProjectionMorph`):** Smoothly transitions vertex coordinates between unit-sphere `Globe`, focus-centered `EqualArea` (`Lambert Azimuthal Equal-Area`), and focus-meridian-centered `Mercator` while culling wrapped antimeridian triangles (`vSeamCull`).

### 2.3 `hydrology-ribbons.ts` — Fog-Clipped Meandering Triangle-Strip Ribbons
- Never emits river segments into `unexplored` space: when `src` is known (`surveyed` or `stale`) and `dst` is `unexplored`, clips the geodesic path at `t = 0.42` inside explored territory and tapers ribbon width cleanly to `0`.
- Terminates rivers strictly at the land/sea interface: evaluates the deterministic shader `effectiveElevM` function along each spline and bisects land-to-sea crossings to stop at `effectiveElevM = +2.0m` with a tapered mouth cap so zero river geometry enters ocean pixels (`effectiveElevM < 0`).
- Generates smooth spherical Catmull-Rom ribbons displaced laterally in local tangent space by deterministic multi-harmonic meanders (`sin(pi * t)` envelope weighted by inverse slope so flat plains meander more than steep gorges).
- Width scales continuously with `strahlerOrder` and `floodStage01`, gently swelling just inland before tapering cleanly into the shoreline mouth cap, eliminating all 3-line parallel flood artifacts and ocean-crossing river extensions.

### 2.4 `semantic-zoom.ts` — 4-Tier Semantic Zoom, De-Clustered Icons & 3D Settlements
| Tier | `altitudeNorm` Range | Terrain / Hydrology | Entities, Formations & Architecture |
| :--- | :--- | :--- | :--- |
| `orbital` (`0`) | `0.70 .. 1.00` | `LOD 0..1` globe + Rayleigh limb + `strahler >= 4` rivers | HQ ring, major settlement medallions, theater arcs |
| `continental` (`1`) | `0.40 .. 0.70` | `LOD 1..2` + `strahler >= 3` rivers + trunk roads | Watershed region titles, settlement nameplates, grouped formations |
| `regional` (`2`) | `0.15 .. 0.40` | `LOD 2..3` + `strahler >= 2` tributaries + all roads | Radially offset lit 3D formation standards & convoy sprites with leader ticks |
| `settlement` (`3`) | `0.00 .. 0.15` | `LOD 3` sub-km relief + `strahler >= 1` streams + field mosaic | Procedural 3D settlement cluster (HQ keep, facility workshops/warehouses, coastal pier, terraced crop fields, regimental camp tents) |

---

## 3. Verification & Lift-and-Shift Contracts
- Each sub-module (`lod-patches.ts`, `hydrology-ribbons.ts`, `semantic-zoom.ts`, `shaders.ts`) depends exclusively on `../spine` contracts and `three`, enabling independent replacement of LOD patch geometry or settlement kitbash generators without touching simulation crates, window management, or HUD/ledger views.
- Verified in `m8_photorealistic_globe_lod_and_projections.test.ts` and `m5_m6_m7_electron_e2e.test.ts` against `@req UI-GLB-006`, `@req UI-GLB-007`, and `@req UI-MAP-008`.
