# Module Design: Licensing, Provenance & Compliance Tooling

**Module ID:** `ENG-MOD-01`  
**Code Location:** `code/engine/tools/xtask/` and `code/engine/crates/steam-adapter/`  
**Milestone & Design Depth:** `M0: Full Detailed Design`  
**Satisfies Requirements:** `ENG-LIC-001`, `ENG-LIC-002`, `ENG-LIC-003`, `ENG-LIC-004`, `ENG-SCN-005` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - Workspace `Cargo.toml` / `Cargo.lock` metadata graph (`cargo metadata --format-version 1`)
  - Markdown requirements documents (`requirements/engine_requirements.md`, `requirements/ui_requirements.md`)
  - Asset provenance manifests (`tools/asset-pipeline/manifests/*.toml`)
  - `PlatformEvent` hook emitted optionally by composition root (`TickPhase::CompileDiegeticPlayerViewAndNotifications`)
- **Provided Spine Ports (Outputs):**
  - `PlatformIntegrationPort` trait in `sim-spine` (`NoopPlatformAdapter` default; `SteamPlatformAdapter` behind `--features steam`)
  - Deterministic CI audit exit codes (`0` pass, non-zero violation details)
  - Generated distributable artifacts: `THIRD_PARTY_NOTICES.txt` and `steam_ai_disclosure.json`

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 `deny.toml` SPDX License Policy State

```toml
[licenses]
unlicensed = "deny"
copyleft = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "Unicode-3.0",
    "CC0-1.0",
]
confidence-threshold = 0.9
```

### 2.2 AI Provenance Manifest Schema (`AssetProvenanceRecord`)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetProvenanceManifest {
    pub schema_version: u32,
    pub assets: Vec<AssetProvenanceRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetProvenanceRecord {
    pub asset_rel_path: String,
    pub generator_model: String,
    pub generator_version: String,
    pub license_tier: String,
    pub commercial_rights_verified: bool,
    pub human_post_processing: String,
    pub steam_disclosure_category: SteamAiCategory, // VisualTexture | Portrait | Icon | AudioFoley | MusicStem
}
```

### 2.3 Traceability Auditor Model (`ParsedRequirement` & `TestCitation`)

```rust
pub struct ParsedRequirement {
    pub id: String,               // Verbatim ID token: ENG-EPI-001 or UI-WIN-002
    pub status: ReqStatus,        // Active | Draft | Superseded | Removed
    pub milestone: MilestoneTag,  // M0..M7
    pub lineage: String,
}

pub struct TestCitation {
    pub req_id: String,
    pub file_path: PathBuf,
    pub line_number: usize,
}
```

### 2.4 Optional Steamworks Boundary (`code/engine/crates/steam-adapter`)

```rust
pub trait PlatformIntegrationPort: Send + Sync {
    fn ensure_launched_via_store(&self) -> Result<(), PlatformError>;
    fn unlock_milestone_flag(&self, key: &str);
    fn tick_callbacks(&mut self);
}
```

`code/engine/crates/sim-core`, `code/engine/crates/sim-data`, `code/engine/crates/player-view`, and `code/engine/crates/sim-cli` never depend on `code/engine/crates/steam-adapter`. Only the desktop host wrapper optionally enables `--features steam`.

---

## 3. Algorithms, Workflows & Verification Execution

### 3.1 `cargo xtask bundle-notices` (`ENG-LIC-004`)
1. Executes `cargo metadata --format-version 1 --locked`.
2. Filters out workspace-member crates and collects every external crate (`name`, `version`, `license`, `authors`, `repository`, `manifest_path`).
3. Locates `LICENSE*` / `COPYING*` files adjacent to each crate's `Cargo.toml`, failing immediately if any resolved dependency lacks a license file or valid SPDX expression.
4. Sorts entries deterministically by `(crate_name, version)` and writes `THIRD_PARTY_NOTICES.txt`.

### 3.2 `cargo xtask verify-assets` (`ENG-LIC-003`)
1. Walks all non-dev files under `skins/` and `mods/` asset directories.
2. Requires every binary graphical or audio file (`.png`, `.webp`, `.ktx2`, `.ogg`, `.wav`) to match either an explicit non-AI open-license entry (`UI-LIC-002`) or an `AssetProvenanceRecord` with `commercial_rights_verified == true`.
3. Rejects empty `generator_model`, unverified commercial status, or uncatalogued orphan binary files with exit code `1`.
4. Aggregates distinct models, categories, and human post-processing summaries into `dist/steam_ai_disclosure.json` ready for Valve's Steamworks AI Content Survey.

### 3.3 `cargo xtask check-reqs` (`ENG-SCN-005`)
1. Parses every `### <ID>: ...` block and its `- **ID:**`, `- **Status:**`, `- **Milestone:**`, and `- **Lineage:**` fields from `requirements/engine_requirements.md` and `requirements/ui_requirements.md`, failing if duplicate IDs or malformed blocks exist.
2. Scans all `.rs`, `.ts`, and `.tsx` test files under `code/engine/crates/` and `code/ui/` for `// @req <ID>` comments or `[<ID>]` test names.
3. Enforces four invariants:
   - Every test citation targets an existing requirement ID.
   - Zero test citations target a `Removed` or `Superseded` requirement ID.
   - Every `Active` requirement with `milestone <= --up-to-milestone` has at least one bound automated test citation.
   - Outputs coverage summary JSON + pass/fail exit code.

### 3.4 `cargo xtask check-isolation` (Spine Guard)
1. Reads `cargo metadata --no-deps --format-version 1`.
2. Asserts zero workspace crates outside composition roots (`sim-scenario`, `sim-cli`, `desktop-host`) depend on more than `{sim-spine, sim-data}`.

---

## 4. Isolated Cartridge Verification Strategy

1. **License Audit (`@req ENG-LIC-001`):** CI executes `cargo deny check licenses` against intentional clean and intentionally poisoned fixture lockfiles containing a copyleft crate, verifying pass on clean and rejection on poisoned.
2. **Steam Boundary Check (`@req ENG-LIC-002`):** Runs `cargo test --workspace` with no Steam SDK libraries installed and asserts `cargo tree -p sim-core -p sim-data -p player-view` emits zero `steamworks` edges.
3. **Asset Provenance Tests (`@req ENG-LIC-003`):** Feeds synthetic manifests missing `commercial_rights_verified = true` to `verify-assets` and checks failure diagnostics and valid `steam_ai_disclosure.json` emission.
4. **Notice Bundler Tests (`@req ENG-LIC-004`):** Verifies deterministic output byte equality and 100 percent lockfile crate inclusion in `THIRD_PARTY_NOTICES.txt`.
5. **Traceability Auditor Tests (`@req ENG-SCN-005`):** Runs `check-reqs` against synthetic markdown and Rust/TS test fixtures containing valid, missing, duplicate, `Superseded`, and `Removed` citations.

---

## 5. Lift-and-Shift Replacement Guide

1. To switch distribution storefronts (for instance, replacing Steamworks with GOG Galaxy or an itch.io DRM-free wrapper), discard `code/engine/crates/steam-adapter/` and write a new implementation of `PlatformIntegrationPort` with zero changes to `sim-spine` or any simulation cartridge.
2. To replace `cargo-deny` or custom `xtask` scripts with an external SBOM/compliance suite, keep the exact CLI flags (`bundle-notices`, `verify-assets`, `check-reqs`, `check-isolation`) and exit-code semantics; no engine crate reads `code/engine/tools/xtask/`.
3. Zero changes to `requirements/*.md` are required.
