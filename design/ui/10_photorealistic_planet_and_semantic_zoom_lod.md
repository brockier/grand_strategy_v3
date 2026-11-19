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
- Precomputes or caches subdivided `BufferGeometry` buffers at 5 resolution tiers while preserving `aCornerCells` (`vec3`) and linearly interpolated `aBarycentric` weights across every child vertex:
  - `LOD 0` (`1x` triangles per base face): orbital view (`altitudeNorm >= 0.68`) or back-hemisphere sectors
  - `LOD 1` (`4x` triangles per base face): sectors within `0.85..0.98 rad` of camera focus below `altitudeNorm < 0.68`
  - `LOD 2` (`16x` triangles per base face): focus sectors within `0.44..0.62 rad` below `altitudeNorm < 0.42`
  - `LOD 3` (`64x` triangles per base face): regional sectors within `0.26..0.34 rad` below `altitudeNorm < 0.28`
  - `LOD 4` (`256x` triangles per base face): immediate close-up focus sectors within `0.14 rad` below `altitudeNorm < 0.16`, adding inward-angled patch boundary skirts (`0.0065`) to conceal T-junction seams
- Unified 3D vertex relief, world-space normal hillshading, and procedural surface micro-detail scale seamlessly across subdivided vertices so zooming in continuously sharpens coastal cliffs, rolling lowland hills, carved river valleys, alpine ridges, and cultivated fields.

### 2.2 `shaders.ts` — Satellite Biophysical PBR & Parchment Dual-Pass Shader
- Samples double-buffered `uCellDataTex` (`[elev, wetness, biomeCode, alpha]`) and `uBiophysicalTex` (`[temp, aridity, strataFertility, strahlerPlume]`) in both `VERTEX_SHADER` and `FRAGMENT_SHADER`, interpolating across barycentric weights `w0, w1, w2` with zero nearest-neighbor Voronoi boundaries.
- **Full-Spectrum Diorama 3D Land Relief:** `VERTEX_SHADER` combines coastal bluffs, concave hypsometric elevation (`pow(posElev / 4200, 0.65)`), multi-octave rolling lowland hills, incised `strahler` river valleys, and ridged alpine crags across every explored land vertex while keeping unexplored vertices and oceans at `0.0` relief.
- **Sub-Cell Fractal Coastline (`coastalWarp`):** Adds multi-octave domain-warped fBm gated by `coastTransition` (`smoothstep(0.05, 0.32, landFrac) * smoothstep(0.05, 0.32, 1.0 - landFrac)`) and latitude (`|lat|` boosts fjord dissection) so coastal fractal displacement applies strictly across mixed land/ocean triangles before sea-level thresholding (`h > 0`), keeping pure-land cells and cell centers dry while eliminating hexagonal shoreline chords.
- **Dark Twilight Unmapped Globe + Cloud Feathering:** `kSum < 0.35` renders pure dark twilight spherical space/globe (strict zero leakage of unexplored cells); feathered boundary band blends surveyed terrain smoothly through multi-octave cloud fBm.
- **Biophysical Reflectance & Close-Up Micro-Detail Synthesis:** Blends dry sand/erg, Mediterranean scrub, temperate lowland meadows & field-patch micro-grain (with close-zoom plowed furrows and hedgerow edges), forest canopy crown ambient occlusion, boreal spruce taiga, slope-exposed fractured rock & scree (`limestone`, `granite`, `basalt`), temperature-driven alpine snowpack, shallow-shelf turquoise -> deep abyssal navy ocean absorption, coastal river sediment plumes, surf breaker foam, capillary water glint, polar pack ice, Rayleigh horizon glow, world-space tangent sunlight shading across `0 deg -> 45 deg` camera pitch, and optional `Parchment` chart lens (`uParchmentMode`).
- **3D <-> 2D Projection Morph (`uProjectionMode`, `uProjectionMorph`):** Smoothly transitions vertex coordinates between unit-sphere `Globe`, focus-centered `EqualArea` (`Lambert Azimuthal Equal-Area`), and focus-meridian-centered `Mercator` while culling wrapped antimeridian triangles (`vSeamCull`).

### 2.3 `hydrology-ribbons.ts` — Contiguous Flow-Directed Hydrology Ribbons & Lakes
- Chains perennial river cells (`riverChannelDownstreamCellId != null`) into maximal upstream-to-terminus paths ending strictly at another perennial confluence junction (snapping endpoints continuously into the trunk spline), an inland lake (`isLake == true`), or an ocean shoreline (`effectiveElevM = +2.0m`) so adjacent reaches never disconnect across cell boundaries.
- Clips seamlessly only at the outer `unexplored` fog boundary (`t = 0.42` fade taper) while connecting `surveyed` and `stale` river segments continuously across `riverChannelDownstreamCellId` hops.
- Generates smooth continuous $C^1$ spherical Catmull-Rom ribbons displaced laterally by deterministic meanders scaled inversely by slope, widens downstream smoothly with `sqrt(riverDischargeM3S)` + `floodStage01`, and renders directional chevron meshes/markers pointing downstream along local river tangents. `vertexReliefHeight` matches `VERTEX_SHADER` identically and stays invariant across `altitudeNorm` so rivers, lakes, stone-grey roads, buildings, and trees remain terrain-locked.

### 2.4 `semantic-zoom.ts` — 4-Tier Semantic Zoom, Pickable 3D Structures, Grey Roads & Instanced Forests
| Tier | `altitudeNorm` Range | Terrain / Camera Pitch / Hydrology | Entities, Formations, Forests & Architecture |
| :--- | :--- | :--- | :--- |
| `orbital` (`0`) | `0.70 .. 1.00` | `LOD 0..1` globe, `0 deg` nadir view, Rayleigh limb, `strahler >= 4` rivers | HQ ring, major settlement medallions, theater arcs |
| `continental` (`1`) | `0.40 .. 0.70` | `LOD 1..2`, `0 deg` -> early pitch onset below `0.45`, `strahler >= 3` rivers, arterial roads | Watershed region titles, settlement nameplates, grouped formations |
| `regional` (`2`) | `0.15 .. 0.40` | `LOD 2..3`, smooth oblique transition toward `45 deg`, `strahler >= 2` tributaries, purpose-built grey roads | Radially offset terrain-grounded 3D formation standards & convoy sprites, regional civic keeps, resource facilities (`userData.pickHit`), `InstancedMesh` woodland crowns |
| `settlement` (`3`) | `0.00 .. 0.15` | `LOD 3..4` (`256x`) relief, `45 deg` oblique terrain flyover, `strahler >= 1` streams + flow chevrons, furrow/hedgerow/surf micro-detail | Grounded sloped-foundation settlements with pickable 3D meshes (`userData.pickHit`: walled HQ citadel, civic hall, street-aligned gabled houses, chimneyed smelters, waterwheel mills, storehouse barns, mines & woodlot camps), shoreline-bisected stone quays & piers, stone-grey constructed roads, hedged crop terraces, parapet tent camps, and dense `InstancedMesh` conifer & broadleaf forests |

---

## 3. Verification & Lift-and-Shift Contracts
- Each sub-module (`lod-patches.ts`, `hydrology-ribbons.ts`, `semantic-zoom.ts`, `shaders.ts`) depends exclusively on `../spine` contracts and `three`, enabling independent replacement of LOD patch geometry or settlement kitbash generators without touching simulation crates, window management, or HUD/ledger views.
- Verified in `m8_photorealistic_globe_lod_and_projections.test.ts` and `m5_m6_m7_electron_e2e.test.ts` against `@req UI-GLB-006`, `@req UI-GLB-007`, and `@req UI-MAP-008`.
