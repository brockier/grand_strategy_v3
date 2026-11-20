# Module Design: Diegetic Map Overlays & Occlusion-Aware Picking

**Module ID:** `UI-MAP-03`  
**Code Location:** `code/ui/src/overlays/`  
**Milestone & Design Depth:** `M2–M7: Full Detailed Design`  
**Satisfies Requirements:** `UI-MAP-001`, `UI-MAP-002`, `UI-MAP-003`, `UI-MAP-004`, `UI-MAP-005.1`, `UI-MAP-006.1`..`006.2`, `UI-MAP-007.1`..`007.2`, `UI-MAP-008`, `UI-MAP-009`, `UI-MAP-010` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`selectKnownCells`, `selectHqPresence`, `selectTrackedConvoys`, `selectObservedWeather`)
  - `GlobeLayerHost` (Three.js scene root, unit-sphere cell transforms, raycaster)
  - `SkinShaderPalettePort` (glyph textures, contour/fog colors, overdue icons)
  - `UiIntentBus` (`SwitchMapOverlayMode`, `PreviewHqLatencyRoute`)
- **Provided Spine Ports (Outputs):**
  - Registered `MapLayerPlugin` instances inside `MapLayerRegistry`
  - Emits `FocusEntityInspector`, `FlyGlobeToCell`, and hover tooltip inspection records

---

## 2. Encapsulated Internal State & Layer Plugin Registry

Each overlay implements `MapLayerPlugin` independently so layers can be toggled, re-skinned, or swapped individually:

| Layer Plugin ID | Active Milestone | Visual Output |
| :--- | :--- | :--- |
| `ExplorationFogAndSurveyLayer` | `M3` (`Active`) | Three-tier epistemic mask (`Unexplored`, `Rumored`, `Surveyed` + survey-age fade) |
| `HqLiveClarityHaloLayer` | `M3` (`Active`) | Crisp observation halo around stationary or moving HQ coordinate |
| `ConvoyAndExpeditionLayer` | `M4` (`Active`) | Instanced sprites distinguishing `Confirmed`, dashed `GhostedProjected`, and `Overdue` |
| `HourlyWeatherOverlayLayer` | `M3` (`Active`) | Observed precipitation rain/snow particles, mud tint, flood river width, ice, wind arrows |
| `OcclusionAwareContinuousPicker` | `M2` (`Active`) | Continuous spherical `(latDeg, lonDeg)`, `unitXyz`, region, spline & marker raycaster gated by DOM occlusion |
| `TradeAndCongestionLayer` | `M5` (`Active`) | Reported trade ribbons, bottleneck highlights, regional price-spread summaries |
| `OperationalWarfareLayer` | `M7` (`Active`) | Theater objectives, corps/division arrows, phase lines, `"No Retreat"` badges, stale-timestamped fronts |

---

## 3. Algorithms, Shader Masks & Picking Rules

### 3.1 Three-Tier Epistemic Terrain & HQ Live Halo (`UI-MAP-001`, `UI-MAP-002`)
1. Maintain a compact per-cell GPU `DataTexture` (`RGBA8Uint` indexed by `CellId`):
   - `R`: Exploration tier (`0 = Unexplored`, `1 = Rumored`, `2 = Surveyed`, `3 = LiveHqSight`)
   - `G`: Survey age normalized (`0` fresh -> `255` decades stale)
   - `B`: Survey precision confidence (`0..255`)
   - `A`: Reserved flags (mineral prospect marker / hazard icon)
2. Fragment shaders render `Unexplored` cells as blank vellum/soft cloud wash; `Rumored` cells as sepia stippled sketches; `Surveyed` cells with progressive sepia aging; and `LiveHqSight` inside a bright crisp cartographic ink ring centered on `HqPresenceState`.

### 3.2 Ghosted Projections & Overdue Glyphs (`UI-MAP-003`)
1. `InstancedMesh` pools render mobile markers above cell terrain normals.
2. `ConfirmedAtHqSight`: solid full-alpha era icon matching carrier type.
3. `GhostedProjected`: `0.55` alpha translucent icon with animated dashed scheduled-path spline and tooltip `"Projected Position (Last Confirmed <date>)"`.
4. `OverdueAlert`: distinct crimson/amber pulsing exclamation ring and broken-path indicator once `player-view` transitions marker status past ETA grace window.

### 3.3 Observed Hourly Weather Visuals (`UI-MAP-004`)
Strictly filters by `ExplorationTier >= Surveyed` inside active report/sight windows (`ObservedWeather` never peeks at hidden `sim-core` storms outside HQ/report range!):
- Darkens ground roughness/color by `mud_index_01`.
- Expands river spline width and adds foam shading by `flood_stage_01`.
- Overlays snow/ice textures from `snow_depth_m` and `ice_thickness_m`.
- Renders localized GPU particle rain streaks/snowflakes and animated wind streamlines.

### 3.4 Occlusion-Aware Continuous 3D/2D Picking (`UI-MAP-005.1`, `UI-GLB-007`, `UI-MAP-009`)
1. Canvas `pointermove` and `pointerdown` handlers check `event.target === globeCanvas` and query whether any floating window, modal `<dialog>`, or HUD element covers `(clientX, clientY)`. If covered, picking aborts immediately (`UI-WIN-005` alignment).
2. In 3D `globe` mode, `pickGlobeAtNdc` performs 3D mesh raycasting (`THREE.Raycaster.intersectObjects` against `features.group.children` carrying typed `userData.pickHit` records) together with zoom-scaled spherical hit radii (`maxAngularDist` scaled linearly with camera distance and altitude). Priority ranks exact 3D feature meshes (`facility`, `settlement` cluster, `convoy`, `formation` at tight radius) -> de-clustered unit markers (`convoy` / `formation` ONLY at tight radius so garrisoned troops never hijack settlement/building/road clicks) -> `settlement` anchor -> `road` centerline -> `river` channel -> terrain `cell`. In 2D `equal_area` or `mercator` mode, rays intersect the `z = 0` chart plane and analytically unproject `(x, y)` back to `unitXyz` via `unprojectFlat2DToUnit(...)`.
3. Clicking `facility`, `settlement`, `road`, `river`, `convoy`, or `formation` focuses the dedicated Entity Inspector tab and attaches a crisp fitted 3D selection highlight (`SelectionHighlightRing` / mesh highlight) cleared on `Escape` or empty ocean/wilderness clicks.

### 3.5 `M5` Trade, `M7` Warfare, Geology & Bathymetry Lens, Unbroken Estuaries & Dam Inspector Controls (`UI-MAP-006.1`..`006.2`, `UI-MAP-007.1`..`007.2`, `UI-MAP-008`, `UI-MAP-009`, `UI-MAP-010`)
- `OverlayToolbarLayer` exposes lens buttons (`terrain` Satellite default, `parchment`, `geology` for `Geology & Bathymetry`, `epistemic`, `weather`, `logistics`, `trade`, `warfare`), 3 projection buttons (`3D Globe`, `2D Equal-Area`, `2D Mercator`), a `Controls (?)` HUD action button, and a lightweight DOM typography overlay (`#map-feature-labels`) inside `#globe-overlay-root` (`pointer-events: none`) showing non-overlapping settlement, watershed region, and regional formation labels synced to camera zoom tier.
- Selecting `geology` (`GeologyBathymetryOverlayLayer`) drives `uGeologyMode` to render explored continental and submarine tectonic provinces (`shield_craton`, `old_orogen`, `young_orogen`, `volcanic_arc`, `sedimentary_basin`, `continental_rift`, `platform`, `alluvial_plain`, `continental_shelf`, `continental_slope`, `mid_ocean_ridge`, `oceanic_trench`, `abyssal_plain`), fault instability contours, desert spring oases, and abyssal bathymetry while holding `Unexplored` cells sealed in twilight fog (`ENG-EPI-001`).
- Perennial rivers (`riverChannelDownstreamCellId != null`) stitch into continuous $C^1$ Catmull-Rom downstream chains across `surveyed` and `stale` cells with zero mid-course gaps, maintain strictly non-increasing 3D water elevation down to lake level or `0.0m` ocean shorelines, flare up to `2.2x` width across tidal `isEstuary` outlets, render flat contour-flooded surfaces for `Exorheic` freshwater lakes, `Endorheic` salt lakes, and impounded dam reservoirs plus pickable 3D masonry dam crests, and carry directional flow chevrons alongside `riverDischargeM3S`, `riverVelocityMps`, `navigability`, `lakeKind`, `salinityPerMille`, and `hydroPotentialKw`.
- Field Inspector `river` and `cell` views expose hydrological/geological dossier rows (tectonic province, roughness, soil texture or shelf/upwelling productivity, spring oasis status, salinity, depth-tier deposits) and interactive **Construct Dam** plus **Dam Rule-Curve** (`BalancedPower`, `FloodControl`, `IrrigationSupply`) controls issuing validated IPC commands (`constructDam`, `setDamRuleCurve`).
- Constructed and worn roads render in skin-driven weathered stone gray (`roadStoneRgb`, defaulting `#7a7d82` parchment / `#868b92` brass) strictly for edges with `roadBuilt = true` and inspectable `roadPurpose`, omitting dead-end spoke artifacts. Co-located settlements, resource facilities, and military formations separate radially with lit `MeshStandardMaterial` icons (`userData.pickHit`) and leader ticks (`semantic-zoom.ts`).

---

## 4. Isolated Cartridge Verification Strategy

1. **Epistemic Visual Distinction (`@req UI-MAP-001`, `@req UI-MAP-002`):** Renders fixture snapshots with all four knowledge states and asserts distinct pixel/LUT values and shifting HQ clarity halo on HQ move.
2. **Ghosted & Overdue Marker States (`@req UI-MAP-003`):** Steps mock snapshot through `GhostedProjected` -> `OverdueAlert` -> `ConfirmedAtHqSight` and verifies sprite class/uniform transitions.
3. **Weather Masking (`@req UI-MAP-004`):** Verifies storm rain/mud effects render on observed cells and produce zero visual artifacts over `Unexplored` cells.
4. **Window-Occluded Continuous Picking (`@req UI-MAP-005.1`):** Verifies covered pixels emit zero picks while exposed globe coordinates emit continuous `(latDeg, lonDeg)`, `unitXyz`, and `cellId`.
5. **Trade & Warfare Overlay Lenses (`@req UI-MAP-006.1`..`006.2`, `@req UI-MAP-007.1`..`007.2`):** Switches to `Trade` and `Warfare` lenses and asserts bottleneck counts, regional price spread timestamps, `"No Retreat"` posture markers, and stale unit timestamps.

---

## 5. Lift-and-Shift Replacement Guide

1. Every overlay layer is a standalone `MapLayerPlugin`. To replace 2D billboard convoy icons with animated 3D ship/wagon meshes or redesign weather particles, replace only the corresponding plugin inside `code/ui/src/overlays/`.
2. Zero changes to `src/globe`, `src/windows`, or `requirements/ui_requirements.md` are needed.
