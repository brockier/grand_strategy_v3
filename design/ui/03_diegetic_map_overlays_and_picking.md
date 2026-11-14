# Module Design: Diegetic Map Overlays & Occlusion-Aware Picking

**Module ID:** `UI-MAP-03`  
**Code Location:** `apps/ui-shell/src/overlays/`  
**Milestone & Design Depth:** `M2–M4: Full Detailed Design` + `M5`/`M7: Locked Plugin Slots`  
**Satisfies Requirements:** `UI-MAP-001`, `UI-MAP-002`, `UI-MAP-003`, `UI-MAP-004`, `UI-MAP-005`, `UI-MAP-006`, `UI-MAP-007` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

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
| `OcclusionAwareCellPicker` | `M2` (`Active`) | Spherical cell/settlement/convoy raycaster gated by DOM occlusion |
| `TradeAndCongestionLayer` | `M5` (`Draft` Slot) | Reported trade ribbons, bottleneck highlights, regional price-spread choropleth |
| `OperationalWarfareLayer` | `M7` (`Draft` Slot) | Theater boundaries, corps/division arrows, phase lines, `"No Retreat"` badges, stale fronts |

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

### 3.4 Occlusion-Aware Spherical Picking (`UI-MAP-005`)
1. Canvas `pointermove` and `pointerdown` handlers check `event.target === globeCanvas` and query whether any floating window DOM element covers `(clientX, clientY)`. If covered by a window or HUD element, picking aborts immediately (`UI-WIN-005` alignment).
2. Unoccluded rays intersect the sphere (`r = 1.0`), locate the exact hit triangle's `aCellId` attribute (with accurate bounding-sphere checks on elevated settlement/convoy markers first), and publish hover/select events onto `UiIntentBus`.

### 3.5 Locked `M5` / `M7` Overlay Plugin Slots (`UI-MAP-006`, `UI-MAP-007`)
`TradeAndCongestionLayer` and `OperationalWarfareLayer` pre-register stub descriptors bound to `SwitchMapOverlayMode` (`modeId = "trade"` and `"warfare"`), consuming strongly typed placeholder slices on `DiegeticSnapshot` so `M5` and `M7` slot cleanly into the map without touching globe shaders.

---

## 4. Isolated Cartridge Verification Strategy

1. **Epistemic Visual Distinction (`@req UI-MAP-001`, `@req UI-MAP-002`):** Renders fixture snapshots with all four knowledge states and asserts distinct pixel/LUT values and shifting HQ clarity halo on HQ move.
2. **Ghosted & Overdue Marker States (`@req UI-MAP-003`):** Steps mock snapshot through `GhostedProjected` -> `OverdueAlert` -> `ConfirmedAtHqSight` and verifies sprite class/uniform transitions.
3. **Weather Masking (`@req UI-MAP-004`):** Verifies storm rain/mud effects render on observed cells and produce zero visual artifacts over `Unexplored` cells.
4. **Window-Occluded Picking (`@req UI-MAP-005`):** Playwright places a floating window over Cell `42`, clicks the covered pixel, verifies zero globe pick emission, drags window clear, clicks Cell `42`, and asserts exact selection of Cell `42`.

---

## 5. Lift-and-Shift Replacement Guide

1. Every overlay layer is a standalone `MapLayerPlugin`. To replace 2D billboard convoy icons with animated 3D ship/wagon meshes or redesign weather particles, replace only the corresponding plugin inside `apps/ui-shell/src/overlays/`.
2. Zero changes to `src/globe`, `src/windows`, or `requirements/ui_requirements.md` are needed.
