# Module Design: Mod Schema Pipeline & Five-Layer Data Loader

**Module ID:** `ENG-MOD-02`  
**Code Location:** `code/engine/crates/sim-data/`  
**Milestone & Design Depth:** `M0–M3: Full Detailed Design`  
**Satisfies Requirements:** `ENG-MOD-001`, `ENG-MOD-002`, `ENG-MOD-003`, `ENG-MOD-004`, `ENG-MOD-005`, `ENG-MOD-006` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - External filesystem paths (`schemas/`, `mods/`, `skins/`, user load-order manifest)
  - CLI command triggers for JSON Schema export
- **Provided Spine Ports (Outputs):**
  - `ModRegistryPort` concrete read-only implementation (`CompiledModBundle`)
  - Generated Draft 2020-12 JSON Schema files via `schemars`
  - Structured line/column `ModLoadError` diagnostic payload for CLI and UI error screens

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Eleven Externalized Domain Registries

`CompiledModBundle` owns dense `Vec<T>` tables indexed by interned `u16` Spine IDs mapped from string keys (`BTreeMap<String, u16>`):

| Domain Index | Mod Subdirectory / File Group | Interned Spine Key |
| :--- | :--- | :--- |
| `1` | `planet/generation.toml` | `PlanetGenConfig` & `ProceduralMaterialSpace` |
| `2` | `commodities/*.toml`, `knowledge/primitives.toml` | `CommodityId`, `TechComponentId`, `TheoryDomainId` |
| `3` | `processes/*.toml` | `ProcessId` |
| `4` | `logistics/carriers.toml`, `logistics/infra.toml` | `CarrierClassId`, `InfraClassId` |
| `5` | `population/cohorts.toml`, `population/vocations.toml` | `VocationId`, `CohortArchetypeId` |
| `6` | `characters/traits.toml`, `characters/succession.toml` | `TraitId`, `SuccessionRuleId` |
| `7` | `governance/institutions.toml`, `governance/laws.toml` | `GovernanceModelId`, `PolicyLawId` |
| `8` | `warfare/formations.toml`, `warfare/doctrines.toml` | `FormationTemplateId`, `DoctrineId` |
| `9` | `scenario.toml`, `calendar.toml`, `initial_state/*.toml` | `ScenarioDefinition`, `CalendarConfig` |
| `10` | `notifications/events.toml`, `localization/*.toml` | `EventClassId`, `DispatchTemplateId` |
| `11` | `skin_manifest.toml` (skin reference metadata) | `SkinManifestMeta` |

### 2.2 Two-Stage AST & Diagnostic Structure

```rust
pub enum ModLayerKind {
    CoreSchemas = 0,
    EraPack = 1,
    ScenarioPack = 2,
    UserMod = 3,
    UiSkin = 4,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModDiagnostic {
    pub severity: DiagnosticSeverity, // Error | Warning
    pub layer: String,
    pub file_path: String,
    pub line: usize,       // 1-based line number preserved from toml::Spanned / serde_path_to_error
    pub column: usize,     // 1-based column number
    pub field_path: String,
    pub message: String,
}
```

Internal staging buffers use `toml::Spanned<toml::Value>` and `serde_json` line-tracked spans so line numbers survive multi-layer dictionary merging before deserialization into concrete typed structs.

---

## 3. Algorithms, Workflows & Validation Pipeline

### 3.1 Deterministic Five-Layer Merge (`ENG-MOD-004`)
1. Sort candidate packs strictly by `(ModLayerKind as u8, manifest.load_priority, pack_id_lexicographic)`. Within any pack directory, collect `.toml` and `.json` files sorted lexicographically by normalized relative UTF-8 path (eliminating OS filesystem `readdir` order variance).
2. Parse each file into a span-annotated intermediate map keyed by `(domain_table, record_string_id)`.
3. Record merge semantics per entry:
   - Default mode (`replace` or new ID): insert or overwrite `record_string_id` while retaining the winning source file span map for diagnostics.
   - Explicit partial patch (`_patch = true`): merge top-level keys deterministically into the existing map entry, rejecting patches whose target `record_string_id` does not yet exist.
4. Deserialize each merged record via `serde_path_to_error` into strongly typed `sim-data` structs, converting type/constraint mismatches directly into `ModDiagnostic` entries using the preserved `(file_path, line, column)`.

### 3.2 Cross-Domain Foreign-Key Validation (`ENG-MOD-005`)
After typed deserialization completes and string IDs intern into dense `u16` IDs:
1. Build valid key sets for all 11 domains.
2. Execute full referential integrity passes:
   - Every `ProcessDef` input/output/catalyst/tool commodity resolves in `CommodityId` table and references a valid `VocationId`.
   - Every `CarrierDef` fuel/fodder commodity resolves in `CommodityId` table and references valid `InfraClassId` allowances.
   - Every `ScenarioDefinition` starting settlement stockpile item, cohort vocation, character trait, and initial facility process resolves against active registries.
   - Every `NotificationRule` references a valid `EventClassId` and dispatch template key.
3. Accumulate *all* diagnostics in one pass rather than aborting on the first error so mod authors receive a complete error list at once.

### 3.3 Draft 2020-12 JSON Schema Export (`ENG-MOD-002`)
1. Every top-level manifest struct derives `schemars::JsonSchema` alongside `serde::Deserialize` and `serde::Serialize`.
2. Calling `export_json_schemas(out_dir)` generates deterministic sorted JSON Schemas for all 11 domains so VS Code and external mod editors validate `.json`/`.toml` packs automatically.

---

## 4. Isolated Cartridge Verification Strategy

1. **Zero Engine Hardcoding (`@req ENG-MOD-001`):** Integration test supplies a synthetic minimal alien mod pack (`crystal_shards`, `hover_sled`, `thermal_extractor`) with zero standard Earth names and runs `sim-data` + `sim-core` initialization cleanly.
2. **Schema Round-Trip (`@req ENG-MOD-002`):** Compares freshly exported `schemars` outputs against committed `schemas/*.schema.json` for exact byte equality.
3. **Diagnostic Span Precision (`@req ENG-MOD-003`):** Feeds malformed syntax, unknown keys (`#[serde(deny_unknown_fields)]`), negative mass limits, and wrong field types; asserts exact `file_path`, `line`, `column`, and zero panics.
4. **Directory Permutation Invariance (`@req ENG-MOD-004`):** Copies test mod directories into randomized inode orders and asserts bit-identical `CompiledModBundle` Blake3 digests.
5. **Foreign-Key Rejection (`@req ENG-MOD-005`):** Injects dangling `commodity_id` and `vocation_id` strings inside process and scenario fixtures and verifies clean multi-error diagnostic return before tick `0`.
6. **Eleven-Domain Completeness (`@req ENG-MOD-006`):** Verifies loading `mods/unexplained-colony/` populates every one of the 11 domain registries and verifies removing any single required domain triggers a clear missing-domain diagnostic.

---

## 5. Lift-and-Shift Replacement Guide

1. To switch mod authoring format (for example, adding YAML, Ron, or SQLite compiled mod caches), keep `CompiledModBundle` implementing `ModRegistryPort` and replace only the AST parser/merger inside `code/engine/crates/sim-data/`.
2. Zero simulation cartridges know whether records arrived from `TOML`, `JSON`, or binary mod archives because they consume strictly `ModRegistryPort` slices and `u16` Spine IDs.
3. Zero changes to `requirements/*.md` or sibling crates are required.
