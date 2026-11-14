# Module Design: External CSS Skin Engine, 9-Slice Chrome & AI Asset Resolver

**Module ID:** `UI-SKN-08`  
**Code Location:** `apps/ui-shell/src/skins/`  
**Milestone & Design Depth:** `M2: Full Detailed Design`  
**Satisfies Requirements:** `UI-SKN-001`, `UI-SKN-002`, `UI-SKN-003`, `UI-SKN-004`, `UI-SKN-005` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - External skin directories (`skins/<skin-id>/skin_manifest.toml`, `.css` files, 9-slice border PNG/WebP sheets, fonts, paper textures, AI asset manifests)
  - `UiIntentBus` (`SwitchActiveSkin`)
  - Active scenario mod icon/portrait tables via `SimBridgeApi`
- **Provided Spine Ports (Outputs):**
  - `SkinShaderPalettePort` consumed by `UI-GLB-02` and `UI-MAP-03`
  - `AssetUrlResolverPort` consumed by `UI-DAT-05` and `UI-DIS-06`
  - Live injected `<link id="active-skin-css">` DOM stylesheet and CSS custom property tokens

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Mandatory CSS Token Contract (`UI-SKN-001`)

Every skin stylesheet (`skins/<skin-id>/theme.css`) MUST define the complete variable contract under `:root`:

```css
:root {
  --skin-bg-panel: #f0e4cc;
  --skin-bg-inset: #e3d3b4;
  --skin-text-primary: #231c15;
  --skin-text-muted: #5d5041;
  --skin-accent-primary: #8b2e22;
  --skin-accent-warning: #b36b12;
  --skin-font-display: "CinzelSkin", serif;
  --skin-font-body: "SpectralSkin", serif;
  --skin-font-mono: "JetBrainsMonoSkin", monospace;
  --skin-frame-image: url("./assets/window_9slice.png");
  --skin-frame-slice: 24;
  --skin-frame-border-width: 18px;
  --skin-chart-series-1: #2a5c8a;
  --skin-chart-series-2: #8b2e22;
  --skin-chart-series-3: #3d6e41;
}
```

### 2.2 9-Slice Window Chrome & Globe Palette Manifest (`skin_manifest.toml`)

```toml
schema_version = 1
skin_id = "cartographer-parchment"
display_name = "Cartographer's Parchment"
stylesheet = "theme.css"
audio_manifest = "audio/audio_manifest.toml"

[globe_palette]
ocean_shallow_hex = "#87a3ad"
ocean_deep_hex = "#3d5966"
land_low_hex = "#d8caa6"
land_high_hex = "#8c7258"
contour_ink_hex = "#3b2c1d"
unexplored_vellum_hex = "#e6d8b9"
parchment_grain_texture = "textures/parchment_tile.png"
hatch_pattern_texture = "textures/ink_hatch_tile.png"
```

### 2.3 AI Portrait & Icon Manifest Index

Maps `(entity_category, semantic_key, age_bracket, era_tag, health_band)` to normalized asset paths verified by `tools/asset-pipeline/`, maintaining an explicit fallback table (`fallback_silhouette_portrait.svg`, `fallback_commodity.svg`).

---

## 3. Algorithms, 9-Slice Scaling & Live Hot-Reload

### 3.1 Zero-Hardcoded-Style ESLint Rule (`UI-SKN-001`)
Custom ESLint plugin / style audit checks all `.ts` and `.tsx` files under `src/windows/`, `src/views/`, `src/dispatches/`, and `src/hud/` rejecting inline CSS hex colors, inline `fontFamily` strings, or pixel border styles—forcing 100 percent reliance on external skin CSS classes and `var(--skin-*)` tokens.

### 3.2 Distortion-Free 9-Slice CSS Chrome (`UI-SKN-002`)
Window frames, title bars, buttons, and dividers apply:
```css
.ng-window-frame {
  border-style: solid;
  border-width: var(--skin-frame-border-width);
  border-image-source: var(--skin-frame-image);
  border-image-slice: var(--skin-frame-slice) fill;
  border-image-repeat: round;
}
```
Corners render at crisp `1:1` artist-intended aspect ratio while edges repeat/stretch seamlessly across windows sized from `240x160` alerts to `1400x800` ledgers.

### 3.3 Atomic Live Skin Swap & Dev Hot-Reload (`UI-SKN-003`, `UI-SKN-004`)
1. When `SwitchActiveSkin { skinId }` arrives—or file watcher detects CSS edit in dev mode:
   - Preload the target `.css` stylesheet in an off-DOM `<link>` element (`rel="preload"` -> swap `<link id="active-skin-css">` `href` cleanly with cache-buster query in dev mode, removing old link only after `onload`).
   - Parse `[globe_palette]` and push updated `SkinShaderPalettePort` uniforms and `THREE.Texture` bindings into `UI-GLB-02` for the next render frame.
   - Zero window React/DOM trees unmount, zero window coordinates shift, and simulation ticks continue uninterrupted.

### 3.4 Resilient AI Portrait & Icon Resolution (`UI-SKN-005`)
`AssetUrlResolverPort::resolvePortrait(tags)` selects best-scoring tag match deterministically by character ID hash among commercial-verified AI portrait plates; missing or modded keys return verified vector placeholder paths immediately without broken image icons.

---

## 4. Isolated Cartridge Verification Strategy

1. **Zero Hardcoded Styles Check (`@req UI-SKN-001`):** Runs ESLint + static scan on `apps/ui-shell/src/` confirming zero literal hex colors or hardcoded font declarations outside `src/skins/` fixtures.
2. **9-Slice Aspect Snapshots (`@req UI-SKN-002`):** Playwright captures visual golden snapshots of `260x180` vs. `1100x700` windows confirming sharp corner pixels without stretch artifacts.
3. **Live Skin Swap Non-Destructiveness (`@req UI-SKN-003`, `@req UI-SKN-004`):** Opens 2 windows, emits `SwitchActiveSkin`, and asserts computed `--skin-bg-panel` CSS values and globe water shader uniforms change immediately while window bounds and `DiegeticSnapshot.tick` stay untouched.
4. **Asset Fallback Test (`@req UI-SKN-005`):** Vitest resolves unknown commodity/portrait tags and asserts valid non-empty fallback URLs and zero failed `<img>` loads.

---

## 5. Lift-and-Shift Replacement Guide

1. Supporting completely new skin capabilities (for example, dark CRT scanline shaders for sci-fi colonies or hammered bronze borders for Bronze Age scenarios) is done purely by adding directories under `skins/` or extending token bindings inside `apps/ui-shell/src/skins/`.
2. Zero view components or Rust crates require modifications.
