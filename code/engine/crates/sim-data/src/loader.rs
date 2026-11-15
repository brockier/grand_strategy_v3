//! Deterministic five-layer mod loader, span-preserving AST merger, foreign-key validator,
//! Draft 2020-12 schema exporter, and `ModRegistryPort` implementation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use schemars::schema_for;
use serde::{Deserialize, Serialize};
use sim_spine::{
    ActorId, CalendarConfigView, CarrierClassId, CarrierDefView, CohortArchetypeDefView,
    CommodityDefView, CommodityId, EventClassId, InfraClassId, InfraDefView, ModRegistryPort,
    NaturalBoundsView, NotificationBehaviorKind, PlanetGenConfigView, ProcessDefView, ProcessId,
    ProcessQuantityView, ScenarioArchetypeKind, ScenarioCharacterSeed, ScenarioInitView,
    ScenarioSettlementSeed, VocationDefView, VocationId,
};

use crate::schema::{
    CalendarRecord, CarrierRecord, CharacterTraitRecord, CharactersDomain, CohortArchetypeRecord,
    CommoditiesAndKnowledgeDomain, CommodityRecord, DoctrineRecord, EventClassRecord,
    FormationTemplateRecord, GovernanceDomain, GovernanceModelRecord, InfraClassRecord,
    LocalizationStringRecord, LogisticsDomain, MergedModDocument,
    NotificationsAndLocDomain, PackManifest, PlanetGenDomain, PlanetGenerationRecord,
    PolicyLawRecord, PopulationDomain, ProcessRecord, ProcessesDomain,
    ScenarioAndCalendarDomain, ScenarioHeaderRecord, SkinRefDomain, SkinReferenceRecord,
    StartingCharacterRecord, StartingSettlementRecord, SuccessionRuleRecord, TechComponentRecord,
    TheoryDomainRecord, VocationRecord, WarfareDomain,
};

/// Diagnostic severity level emitted during mod loading and validation.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Structured, line-accurate diagnostic returned when loading or validating mod packs.
#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModDiagnostic {
    pub severity: DiagnosticSeverity,
    pub layer: String,
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub field_path: String,
    pub message: String,
}

#[derive(Clone, Debug)]
struct SourceSpan {
    layer: String,
    file_path: String,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug)]
struct StagedField {
    value: serde_json::Value,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct StagedRecord {
    record_span: SourceSpan,
    fields: BTreeMap<String, StagedField>,
    nested_spans: BTreeMap<String, SourceSpan>,
}

#[derive(Clone, Debug)]
struct DiscoveredPack {
    manifest: PackManifest,
    dir: PathBuf,
    files: Vec<PathBuf>,
}

/// Validated, immutable external data bundle implementing `ModRegistryPort`.
#[derive(Clone, Debug)]
pub struct CompiledModBundle {
    pub merged_doc: MergedModDocument,
    commodity_defs: Vec<CommodityDefView>,
    commodity_by_key: BTreeMap<String, CommodityId>,
    process_defs: Vec<ProcessDefView>,
    carrier_defs: Vec<CarrierDefView>,
    carrier_by_key: BTreeMap<String, CarrierClassId>,
    infra_defs: Vec<InfraDefView>,
    infra_by_key: BTreeMap<String, InfraClassId>,
    vocation_defs: Vec<VocationDefView>,
    archetype_defs: Vec<CohortArchetypeDefView>,
    event_by_key: BTreeMap<String, EventClassId>,
    notification_default_by_key: BTreeMap<String, NotificationBehaviorKind>,
    notification_list: Vec<(EventClassId, String, String, NotificationBehaviorKind)>,
    localization_by_key: BTreeMap<String, String>,
    calendar_view: CalendarConfigView,
    planet_gen_view: PlanetGenConfigView,
    scenario_view: ScenarioInitView,
    digest_bytes: [u8; 32],
    digest_hex: String,
}

impl CompiledModBundle {
    /// Returns the deterministic Blake3 hex digest computed over canonical merged mod JSON.
    #[must_use]
    pub fn blake3_digest(&self) -> &str {
        &self.digest_hex
    }
}

impl ModRegistryPort for CompiledModBundle {
    fn commodity_def(&self, id: CommodityId) -> Option<&CommodityDefView> {
        self.commodity_defs.get(id.get() as usize)
    }

    fn commodity_count(&self) -> usize {
        self.commodity_defs.len()
    }

    fn commodity_id_by_key(&self, key: &str) -> Option<CommodityId> {
        self.commodity_by_key.get(key).copied()
    }

    fn process_def(&self, id: ProcessId) -> Option<&ProcessDefView> {
        self.process_defs.get(id.get() as usize)
    }

    fn process_count(&self) -> usize {
        self.process_defs.len()
    }

    fn carrier_def(&self, id: CarrierClassId) -> Option<&CarrierDefView> {
        self.carrier_defs.get(id.get() as usize)
    }

    fn carrier_count(&self) -> usize {
        self.carrier_defs.len()
    }

    fn carrier_id_by_key(&self, key: &str) -> Option<CarrierClassId> {
        self.carrier_by_key.get(key).copied()
    }

    fn infra_def(&self, id: InfraClassId) -> Option<&InfraDefView> {
        self.infra_defs.get(id.get() as usize)
    }

    fn infra_count(&self) -> usize {
        self.infra_defs.len()
    }

    fn infra_id_by_key(&self, key: &str) -> Option<InfraClassId> {
        self.infra_by_key.get(key).copied()
    }

    fn vocation_def(&self, id: VocationId) -> Option<&VocationDefView> {
        self.vocation_defs.get(id.get() as usize)
    }

    fn vocation_count(&self) -> usize {
        self.vocation_defs.len()
    }

    fn cohort_archetype_count(&self) -> usize {
        self.archetype_defs.len()
    }

    fn cohort_archetype_def(&self, idx: usize) -> Option<&CohortArchetypeDefView> {
        self.archetype_defs.get(idx)
    }

    fn event_class_id(&self, key: &str) -> Option<EventClassId> {
        self.event_by_key.get(key).copied()
    }

    fn notification_default_behavior(&self, key: &str) -> Option<NotificationBehaviorKind> {
        self.notification_default_by_key.get(key).copied()
    }

    fn notification_classes(&self) -> Vec<(EventClassId, String, String, NotificationBehaviorKind)> {
        self.notification_list.clone()
    }

    fn localization_template(&self, key: &str) -> Option<&str> {
        self.localization_by_key.get(key).map(String::as_str)
    }

    fn calendar_config(&self) -> &CalendarConfigView {
        &self.calendar_view
    }

    fn planet_gen_config(&self) -> &PlanetGenConfigView {
        &self.planet_gen_view
    }

    fn bundle_blake3_bytes(&self) -> [u8; 32] {
        self.digest_bytes
    }

    fn scenario_init(&self) -> &ScenarioInitView {
        &self.scenario_view
    }
}

const SINGLETON_SECTIONS: &[&str] = &[
    "planet_generation",
    "calendar",
    "scenario",
    "skin_reference",
];

const ARRAY_SECTIONS: &[&str] = &[
    "commodities",
    "tech_components",
    "theory_domains",
    "processes",
    "carriers",
    "infra_classes",
    "cohort_archetypes",
    "vocations",
    "traits",
    "succession_rules",
    "governance_models",
    "policy_laws",
    "formation_templates",
    "doctrines",
    "starting_settlements",
    "starting_characters",
    "event_classes",
    "strings",
];

fn byte_offset_to_line_col(text: &str, byte_offset: usize) -> (usize, usize) {
    let clamped = byte_offset.min(text.len());
    let slice = &text[..clamped];
    let mut line = 1usize;
    let mut last_nl = 0usize;
    for (idx, b) in slice.bytes().enumerate() {
        if b == b'\n' {
            line += 1;
            last_nl = idx + 1;
        }
    }
    let col = slice[last_nl..].chars().count() + 1;
    (line, col)
}

fn collect_sorted_mod_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = BTreeSet::new();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("Failed reading directory {}: {e}", dir.display()))?;
        for entry_res in entries {
            let entry = entry_res.map_err(|e| format!("Failed iterating {}: {e}", dir.display()))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                if rel == Path::new("pack.toml") {
                    continue;
                }
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let lower = ext.to_ascii_lowercase();
                    if lower == "toml" || lower == "json" {
                        files.insert(path);
                    }
                }
            }
        }
    }
    Ok(files.into_iter().collect())
}

fn discover_packs(pack_dirs: &[&Path], diagnostics: &mut Vec<ModDiagnostic>) -> Vec<DiscoveredPack> {
    let mut discovered = Vec::new();
    for dir in pack_dirs {
        let manifest_path = dir.join("pack.toml");
        let display_path = manifest_path.display().to_string();
        let raw = match fs::read_to_string(&manifest_path) {
            Ok(text) => text,
            Err(err) => {
                diagnostics.push(ModDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    layer: "PackDiscovery".to_string(),
                    file_path: display_path,
                    line: 1,
                    column: 1,
                    field_path: "pack.toml".to_string(),
                    message: format!("Unable to read pack.toml: {err}"),
                });
                continue;
            }
        };

        let doc = match toml_edit::ImDocument::parse(raw.as_str()) {
            Ok(parsed) => parsed,
            Err(err) => {
                let (line, column) = err
                    .span()
                    .map(|r| byte_offset_to_line_col(&raw, r.start))
                    .unwrap_or((1, 1));
                diagnostics.push(ModDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    layer: "PackDiscovery".to_string(),
                    file_path: display_path,
                    line,
                    column,
                    field_path: "syntax".to_string(),
                    message: err.message().to_string(),
                });
                continue;
            }
        };

        let mut field_spans = BTreeMap::new();
        for (k, item) in doc.iter() {
            let offset = item
                .span()
                .or_else(|| doc.key(k).and_then(|key| key.span()))
                .map(|r| r.start)
                .unwrap_or(0);
            let (line, column) = byte_offset_to_line_col(&raw, offset);
            field_spans.insert(k.to_string(), (line, column));
        }

        let de = toml::Deserializer::new(&raw);
        match serde_path_to_error::deserialize::<_, PackManifest>(de) {
            Ok(manifest) => {
                match collect_sorted_mod_files(dir) {
                    Ok(files) => discovered.push(DiscoveredPack {
                        manifest,
                        dir: dir.to_path_buf(),
                        files,
                    }),
                    Err(err) => diagnostics.push(ModDiagnostic {
                        severity: DiagnosticSeverity::Error,
                        layer: format!("{:?}", manifest.layer),
                        file_path: dir.display().to_string(),
                        line: 1,
                        column: 1,
                        field_path: "directory".to_string(),
                        message: err,
                    }),
                }
            }
            Err(err) => {
                let fp = err.path().to_string();
                let root_key = fp.split('.').next().unwrap_or(&fp);
                let (line, column) = field_spans.get(root_key).copied().unwrap_or((1, 1));
                diagnostics.push(ModDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    layer: "PackManifest".to_string(),
                    file_path: display_path,
                    line,
                    column,
                    field_path: if fp.is_empty() { "pack".to_string() } else { fp },
                    message: err.inner().to_string(),
                });
            }
        }
    }

    discovered.sort_by(|a, b| {
        (
            a.manifest.layer as u8,
            a.manifest.load_priority,
            a.manifest.pack_id.as_str(),
            a.dir.to_string_lossy().as_ref(),
        )
            .cmp(&(
                b.manifest.layer as u8,
                b.manifest.load_priority,
                b.manifest.pack_id.as_str(),
                b.dir.to_string_lossy().as_ref(),
            ))
    });
    discovered
}

fn toml_item_to_json(
    item: &toml_edit::Item,
    raw: &str,
    layer: &str,
    file_path: &str,
    prefix: &str,
    nested_spans: &mut BTreeMap<String, SourceSpan>,
) -> serde_json::Value {
    match item {
        toml_edit::Item::Value(val) => toml_value_to_json(val, raw, layer, file_path, prefix, nested_spans),
        toml_edit::Item::Table(tbl) => {
            let mut map = serde_json::Map::new();
            let mut keys: Vec<&str> = tbl.iter().map(|(k, _)| k).collect();
            keys.sort_unstable();
            for k in keys {
                if let Some(child) = tbl.get(k) {
                    let child_path = if prefix.is_empty() {
                        k.to_string()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    if let Some(span) = child.span().or_else(|| tbl.key(k).and_then(|tk| tk.span())) {
                        let (line, column) = byte_offset_to_line_col(raw, span.start);
                        nested_spans.insert(
                            child_path.clone(),
                            SourceSpan {
                                layer: layer.to_string(),
                                file_path: file_path.to_string(),
                                line,
                                column,
                            },
                        );
                    }
                    map.insert(
                        k.to_string(),
                        toml_item_to_json(child, raw, layer, file_path, &child_path, nested_spans),
                    );
                }
            }
            serde_json::Value::Object(map)
        }
        toml_edit::Item::ArrayOfTables(aot) => {
            let mut arr = Vec::new();
            for (idx, tbl) in aot.iter().enumerate() {
                let mut map = serde_json::Map::new();
                let elem_path = format!("{prefix}[{idx}]");
                if let Some(span) = tbl.span() {
                    let (line, column) = byte_offset_to_line_col(raw, span.start);
                    nested_spans.insert(
                        elem_path.clone(),
                        SourceSpan {
                            layer: layer.to_string(),
                            file_path: file_path.to_string(),
                            line,
                            column,
                        },
                    );
                }
                let mut keys: Vec<&str> = tbl.iter().map(|(k, _)| k).collect();
                keys.sort_unstable();
                for k in keys {
                    if let Some(child) = tbl.get(k) {
                        let child_path = format!("{elem_path}.{k}");
                        if let Some(span) = child.span().or_else(|| tbl.key(k).and_then(|tk| tk.span())) {
                            let (line, column) = byte_offset_to_line_col(raw, span.start);
                            nested_spans.insert(
                                child_path.clone(),
                                SourceSpan {
                                    layer: layer.to_string(),
                                    file_path: file_path.to_string(),
                                    line,
                                    column,
                                },
                            );
                        }
                        map.insert(
                            k.to_string(),
                            toml_item_to_json(child, raw, layer, file_path, &child_path, nested_spans),
                        );
                    }
                }
                arr.push(serde_json::Value::Object(map));
            }
            serde_json::Value::Array(arr)
        }
        toml_edit::Item::None => serde_json::Value::Null,
    }
}

fn toml_value_to_json(
    val: &toml_edit::Value,
    raw: &str,
    layer: &str,
    file_path: &str,
    prefix: &str,
    nested_spans: &mut BTreeMap<String, SourceSpan>,
) -> serde_json::Value {
    match val {
        toml_edit::Value::String(s) => serde_json::Value::String(s.value().clone()),
        toml_edit::Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i.value())),
        toml_edit::Value::Float(f) => serde_json::Number::from_f64(*f.value())
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml_edit::Value::Boolean(b) => serde_json::Value::Bool(*b.value()),
        toml_edit::Value::Datetime(dt) => serde_json::Value::String(dt.value().to_string()),
        toml_edit::Value::Array(arr) => {
            let mut out = Vec::new();
            for (idx, item) in arr.iter().enumerate() {
                let elem_path = format!("{prefix}[{idx}]");
                if let Some(span) = item.span() {
                    let (line, column) = byte_offset_to_line_col(raw, span.start);
                    nested_spans.insert(
                        elem_path.clone(),
                        SourceSpan {
                            layer: layer.to_string(),
                            file_path: file_path.to_string(),
                            line,
                            column,
                        },
                    );
                }
                out.push(toml_value_to_json(item, raw, layer, file_path, &elem_path, nested_spans));
            }
            serde_json::Value::Array(out)
        }
        toml_edit::Value::InlineTable(itbl) => {
            let mut map = serde_json::Map::new();
            let mut keys: Vec<&str> = itbl.iter().map(|(k, _)| k).collect();
            keys.sort_unstable();
            for k in keys {
                if let Some(v) = itbl.get(k) {
                    let child_path = format!("{prefix}.{k}");
                    if let Some(span) = v.span().or_else(|| itbl.key(k).and_then(|tk| tk.span())) {
                        let (line, column) = byte_offset_to_line_col(raw, span.start);
                        nested_spans.insert(
                            child_path.clone(),
                            SourceSpan {
                                layer: layer.to_string(),
                                file_path: file_path.to_string(),
                                line,
                                column,
                            },
                        );
                    }
                    map.insert(
                        k.to_string(),
                        toml_value_to_json(v, raw, layer, file_path, &child_path, nested_spans),
                    );
                }
            }
            serde_json::Value::Object(map)
        }
    }
}

fn find_json_key_line_col(raw: &str, key: &str) -> (usize, usize) {
    let needle = format!("\"{key}\"");
    for (idx, line_text) in raw.lines().enumerate() {
        if let Some(byte_col) = line_text.find(&needle) {
            return (idx + 1, byte_col + 1);
        }
    }
    (1, 1)
}

#[allow(clippy::too_many_arguments)]
fn ingest_record_table(
    section: &str,
    tbl_fields: BTreeMap<String, StagedField>,
    nested_spans: BTreeMap<String, SourceSpan>,
    record_span: SourceSpan,
    is_singleton: bool,
    singleton_buckets: &mut BTreeMap<String, StagedRecord>,
    array_buckets: &mut BTreeMap<String, BTreeMap<String, StagedRecord>>,
    diagnostics: &mut Vec<ModDiagnostic>,
) {
    let is_patch = tbl_fields
        .get("_patch")
        .and_then(|f| f.value.as_bool())
        .unwrap_or(false);

    let mut clean_fields = tbl_fields;
    let patch_span = clean_fields.remove("_patch").map(|f| f.span);

    if is_singleton {
        if is_patch {
            if let Some(existing) = singleton_buckets.get_mut(section) {
                existing.record_span = record_span;
                for (k, v) in clean_fields {
                    existing.fields.insert(k, v);
                }
                for (k, v) in nested_spans {
                    existing.nested_spans.insert(k, v);
                }
            } else {
                let sp = patch_span.unwrap_or(record_span);
                diagnostics.push(ModDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    layer: sp.layer,
                    file_path: sp.file_path,
                    line: sp.line,
                    column: sp.column,
                    field_path: format!("{section}._patch"),
                    message: format!("Cannot apply _patch to missing singleton section '{section}'"),
                });
            }
        } else {
            singleton_buckets.insert(
                section.to_string(),
                StagedRecord {
                    record_span,
                    fields: clean_fields,
                    nested_spans,
                },
            );
        }
        return;
    }

    let id_val = clean_fields
        .get("id")
        .and_then(|f| f.value.as_str())
        .map(|s| s.to_string());
    let Some(record_id) = id_val else {
        diagnostics.push(ModDiagnostic {
            severity: DiagnosticSeverity::Error,
            layer: record_span.layer,
            file_path: record_span.file_path,
            line: record_span.line,
            column: record_span.column,
            field_path: format!("{section}.id"),
            message: format!("Record in section '{section}' is missing required string 'id'"),
        });
        return;
    };

    let bucket = array_buckets.entry(section.to_string()).or_default();
    if is_patch {
        if let Some(existing) = bucket.get_mut(&record_id) {
            for (k, v) in clean_fields {
                if k == "id" {
                    continue;
                }
                existing.fields.insert(k, v);
            }
            for (k, v) in nested_spans {
                existing.nested_spans.insert(k, v);
            }
        } else {
            let sp = patch_span.unwrap_or(record_span);
            diagnostics.push(ModDiagnostic {
                severity: DiagnosticSeverity::Error,
                layer: sp.layer,
                file_path: sp.file_path,
                line: sp.line,
                column: sp.column,
                field_path: format!("{section}.{record_id}._patch"),
                message: format!(
                    "Cannot apply _patch because target record '{record_id}' in '{section}' does not exist"
                ),
            });
        }
    } else {
        bucket.insert(
            record_id,
            StagedRecord {
                record_span,
                fields: clean_fields,
                nested_spans,
            },
        );
    }
}

fn extract_unknown_field_name(msg: &str) -> Option<&str> {
    let prefix = "unknown field `";
    let start = msg.find(prefix)? + prefix.len();
    let rest = &msg[start..];
    let end = rest.find('`')?;
    Some(&rest[..end])
}

fn resolve_span<'a>(record: &'a StagedRecord, field_path: &str, err_msg: Option<&str>) -> &'a SourceSpan {
    if let Some(sp) = record.nested_spans.get(field_path) {
        return sp;
    }
    let first_seg = field_path.split(['.', '[']).next().unwrap_or("");
    if let Some(field) = record.fields.get(first_seg) {
        return &field.span;
    }
    if let Some(msg) = err_msg
        && let Some(unknown) = extract_unknown_field_name(msg)
            && let Some(field) = record.fields.get(unknown) {
                return &field.span;
            }
    &record.record_span
}

fn deserialize_staged_record<T: for<'de> Deserialize<'de>>(
    section: &str,
    record: &StagedRecord,
    diagnostics: &mut Vec<ModDiagnostic>,
) -> Option<T> {
    let mut obj = serde_json::Map::new();
    for (k, v) in &record.fields {
        obj.insert(k.clone(), v.value.clone());
    }
    let val = serde_json::Value::Object(obj);
    match serde_path_to_error::deserialize::<_, T>(val) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            let rel_path = err.path().to_string();
            let err_text = err.inner().to_string();
            let span = resolve_span(record, &rel_path, Some(&err_text));
            let reported_path = if !rel_path.is_empty() && rel_path != "." {
                format!("{section}.{rel_path}")
            } else if let Some(unknown) = extract_unknown_field_name(&err_text) {
                format!("{section}.{unknown}")
            } else {
                section.to_string()
            };
            diagnostics.push(ModDiagnostic {
                severity: DiagnosticSeverity::Error,
                layer: span.layer.clone(),
                file_path: span.file_path.clone(),
                line: span.line,
                column: span.column,
                field_path: reported_path,
                message: err_text,
            });
            None
        }
    }
}

fn push_field_diag(
    diagnostics: &mut Vec<ModDiagnostic>,
    section: &str,
    record: &StagedRecord,
    field_path: &str,
    message: String,
) {
    let span = resolve_span(record, field_path, None);
    diagnostics.push(ModDiagnostic {
        severity: DiagnosticSeverity::Error,
        layer: span.layer.clone(),
        file_path: span.file_path.clone(),
        line: span.line,
        column: span.column,
        field_path: format!("{section}.{field_path}"),
        message,
    });
}

fn parse_packs_into_buckets(
    packs: &[DiscoveredPack],
    singleton_buckets: &mut BTreeMap<String, StagedRecord>,
    array_buckets: &mut BTreeMap<String, BTreeMap<String, StagedRecord>>,
    diagnostics: &mut Vec<ModDiagnostic>,
) {
    for pack in packs {
        let layer_label = format!("{:?}:{}", pack.manifest.layer, pack.manifest.pack_id);
        for file_path in &pack.files {
            let path_str = file_path.display().to_string();
            let raw = match fs::read_to_string(file_path) {
                Ok(text) => text,
                Err(err) => {
                    diagnostics.push(ModDiagnostic {
                        severity: DiagnosticSeverity::Error,
                        layer: layer_label.clone(),
                        file_path: path_str,
                        line: 1,
                        column: 1,
                        field_path: "io".to_string(),
                        message: format!("Failed to read mod file: {err}"),
                    });
                    continue;
                }
            };

            let ext = file_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            if ext == "toml" {
                let doc = match toml_edit::ImDocument::parse(raw.as_str()) {
                    Ok(d) => d,
                    Err(err) => {
                        let (line, column) = err
                            .span()
                            .map(|r| byte_offset_to_line_col(&raw, r.start))
                            .unwrap_or((1, 1));
                        diagnostics.push(ModDiagnostic {
                            severity: DiagnosticSeverity::Error,
                            layer: layer_label.clone(),
                            file_path: path_str,
                            line,
                            column,
                            field_path: "syntax".to_string(),
                            message: err.message().to_string(),
                        });
                        continue;
                    }
                };

                let mut top_keys: Vec<&str> = doc.iter().map(|(k, _)| k).collect();
                top_keys.sort_unstable();
                for section in top_keys {
                    let Some(item) = doc.get(section) else { continue };
                    let section_offset = item
                        .span()
                        .or_else(|| doc.key(section).and_then(|k| k.span()))
                        .map(|r| r.start)
                        .unwrap_or(0);
                    let (sec_line, sec_col) = byte_offset_to_line_col(&raw, section_offset);

                    if SINGLETON_SECTIONS.contains(&section) {
                        let Some(tbl) = item.as_table() else {
                            diagnostics.push(ModDiagnostic {
                                severity: DiagnosticSeverity::Error,
                                layer: layer_label.clone(),
                                file_path: path_str.clone(),
                                line: sec_line,
                                column: sec_col,
                                field_path: section.to_string(),
                                message: format!("Section '{section}' must be a TOML table"),
                            });
                            continue;
                        };
                        let (fields, nested_spans) =
                            extract_toml_table_fields(tbl, &raw, &layer_label, &path_str);
                        ingest_record_table(
                            section,
                            fields,
                            nested_spans,
                            SourceSpan {
                                layer: layer_label.clone(),
                                file_path: path_str.clone(),
                                line: sec_line,
                                column: sec_col,
                            },
                            true,
                            singleton_buckets,
                            array_buckets,
                            diagnostics,
                        );
                    } else if ARRAY_SECTIONS.contains(&section) {
                        let Some(aot) = item.as_array_of_tables() else {
                            diagnostics.push(ModDiagnostic {
                                severity: DiagnosticSeverity::Error,
                                layer: layer_label.clone(),
                                file_path: path_str.clone(),
                                line: sec_line,
                                column: sec_col,
                                field_path: section.to_string(),
                                message: format!("Section '{section}' must be an array of tables [[{section}]]"),
                            });
                            continue;
                        };
                        for tbl in aot.iter() {
                            let tbl_offset = tbl.span().map(|r| r.start).unwrap_or(section_offset);
                            let (t_line, t_col) = byte_offset_to_line_col(&raw, tbl_offset);
                            let (fields, nested_spans) =
                                extract_toml_table_fields(tbl, &raw, &layer_label, &path_str);
                            ingest_record_table(
                                section,
                                fields,
                                nested_spans,
                                SourceSpan {
                                    layer: layer_label.clone(),
                                    file_path: path_str.clone(),
                                    line: t_line,
                                    column: t_col,
                                },
                                false,
                                singleton_buckets,
                                array_buckets,
                                diagnostics,
                            );
                        }
                    } else {
                        diagnostics.push(ModDiagnostic {
                            severity: DiagnosticSeverity::Error,
                            layer: layer_label.clone(),
                            file_path: path_str.clone(),
                            line: sec_line,
                            column: sec_col,
                            field_path: section.to_string(),
                            message: format!("Unknown top-level mod section '{section}'"),
                        });
                    }
                }
            } else if ext == "json" {
                let root_val: serde_json::Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(err) => {
                        diagnostics.push(ModDiagnostic {
                            severity: DiagnosticSeverity::Error,
                            layer: layer_label.clone(),
                            file_path: path_str,
                            line: err.line().max(1),
                            column: err.column().max(1),
                            field_path: "syntax".to_string(),
                            message: err.to_string(),
                        });
                        continue;
                    }
                };
                let Some(root_obj) = root_val.as_object() else {
                    diagnostics.push(ModDiagnostic {
                        severity: DiagnosticSeverity::Error,
                        layer: layer_label.clone(),
                        file_path: path_str,
                        line: 1,
                        column: 1,
                        field_path: "$".to_string(),
                        message: "Top-level JSON mod file must be a JSON object".to_string(),
                    });
                    continue;
                };
                let mut keys: Vec<&String> = root_obj.keys().collect();
                keys.sort();
                for section in keys {
                    let (sec_line, sec_col) = find_json_key_line_col(&raw, section);
                    let val = &root_obj[section];
                    if SINGLETON_SECTIONS.contains(&section.as_str()) {
                        let Some(obj) = val.as_object() else {
                            diagnostics.push(ModDiagnostic {
                                severity: DiagnosticSeverity::Error,
                                layer: layer_label.clone(),
                                file_path: path_str.clone(),
                                line: sec_line,
                                column: sec_col,
                                field_path: section.clone(),
                                message: format!("JSON section '{section}' must be an object"),
                            });
                            continue;
                        };
                        let fields = extract_json_object_fields(obj, &raw, &layer_label, &path_str);
                        ingest_record_table(
                            section,
                            fields,
                            BTreeMap::new(),
                            SourceSpan {
                                layer: layer_label.clone(),
                                file_path: path_str.clone(),
                                line: sec_line,
                                column: sec_col,
                            },
                            true,
                            singleton_buckets,
                            array_buckets,
                            diagnostics,
                        );
                    } else if ARRAY_SECTIONS.contains(&section.as_str()) {
                        let Some(arr) = val.as_array() else {
                            diagnostics.push(ModDiagnostic {
                                severity: DiagnosticSeverity::Error,
                                layer: layer_label.clone(),
                                file_path: path_str.clone(),
                                line: sec_line,
                                column: sec_col,
                                field_path: section.clone(),
                                message: format!("JSON section '{section}' must be an array of objects"),
                            });
                            continue;
                        };
                        for elem in arr {
                            let Some(obj) = elem.as_object() else {
                                diagnostics.push(ModDiagnostic {
                                    severity: DiagnosticSeverity::Error,
                                    layer: layer_label.clone(),
                                    file_path: path_str.clone(),
                                    line: sec_line,
                                    column: sec_col,
                                    field_path: section.clone(),
                                    message: format!("Elements of '{section}' must be JSON objects"),
                                });
                                continue;
                            };
                            let fields =
                                extract_json_object_fields(obj, &raw, &layer_label, &path_str);
                            ingest_record_table(
                                section,
                                fields,
                                BTreeMap::new(),
                                SourceSpan {
                                    layer: layer_label.clone(),
                                    file_path: path_str.clone(),
                                    line: sec_line,
                                    column: sec_col,
                                },
                                false,
                                singleton_buckets,
                                array_buckets,
                                diagnostics,
                            );
                        }
                    } else {
                        diagnostics.push(ModDiagnostic {
                            severity: DiagnosticSeverity::Error,
                            layer: layer_label.clone(),
                            file_path: path_str.clone(),
                            line: sec_line,
                            column: sec_col,
                            field_path: section.clone(),
                            message: format!("Unknown top-level mod section '{section}'"),
                        });
                    }
                }
            }
        }
    }
}

fn extract_toml_table_fields(
    tbl: &toml_edit::Table,
    raw: &str,
    layer: &str,
    file_path: &str,
) -> (BTreeMap<String, StagedField>, BTreeMap<String, SourceSpan>) {
    let mut fields = BTreeMap::new();
    let mut nested_spans = BTreeMap::new();
    let mut keys: Vec<&str> = tbl.iter().map(|(k, _)| k).collect();
    keys.sort_unstable();
    for k in keys {
        let Some(child) = tbl.get(k) else { continue };
        let offset = tbl
            .key(k)
            .and_then(|tk| tk.span())
            .or_else(|| child.span())
            .map(|r| r.start)
            .unwrap_or(0);
        let (line, column) = byte_offset_to_line_col(raw, offset);
        let json_val = toml_item_to_json(child, raw, layer, file_path, k, &mut nested_spans);
        let span = SourceSpan {
            layer: layer.to_string(),
            file_path: file_path.to_string(),
            line,
            column,
        };
        nested_spans.insert(k.to_string(), span.clone());
        fields.insert(
            k.to_string(),
            StagedField {
                value: json_val,
                span,
            },
        );
    }
    (fields, nested_spans)
}

fn extract_json_object_fields(
    obj: &serde_json::Map<String, serde_json::Value>,
    raw: &str,
    layer: &str,
    file_path: &str,
) -> BTreeMap<String, StagedField> {
    let mut fields = BTreeMap::new();
    for (k, v) in obj {
        let (line, column) = find_json_key_line_col(raw, k);
        fields.insert(
            k.clone(),
            StagedField {
                value: v.clone(),
                span: SourceSpan {
                    layer: layer.to_string(),
                    file_path: file_path.to_string(),
                    line,
                    column,
                },
            },
        );
    }
    fields
}

/// Loads, merges in deterministic five-layer order, validates schemas, constraints,
/// and cross-domain foreign keys, returning either a `CompiledModBundle` or full structured diagnostics.
pub fn load_mod_bundle(pack_dirs: &[&Path]) -> Result<CompiledModBundle, Vec<ModDiagnostic>> {
    let mut diagnostics = Vec::new();
    let packs = discover_packs(pack_dirs, &mut diagnostics);
    if packs.is_empty() && !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut singleton_buckets: BTreeMap<String, StagedRecord> = BTreeMap::new();
    let mut array_buckets: BTreeMap<String, BTreeMap<String, StagedRecord>> = BTreeMap::new();
    parse_packs_into_buckets(
        &packs,
        &mut singleton_buckets,
        &mut array_buckets,
        &mut diagnostics,
    );

    let parse_singleton = |section: &str, diags: &mut Vec<ModDiagnostic>| -> Option<&StagedRecord> {
        match singleton_buckets.get(section) {
            Some(rec) => Some(rec),
            None => {
                diags.push(ModDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    layer: "Bundle".to_string(),
                    file_path: section.to_string(),
                    line: 1,
                    column: 1,
                    field_path: section.to_string(),
                    message: format!("Required singleton domain section '{section}' is missing"),
                });
                None
            }
        }
    };

    let planet_generation = parse_singleton("planet_generation", &mut diagnostics)
        .and_then(|rec| deserialize_staged_record::<PlanetGenerationRecord>("planet_generation", rec, &mut diagnostics));
    let calendar = parse_singleton("calendar", &mut diagnostics)
        .and_then(|rec| deserialize_staged_record::<CalendarRecord>("calendar", rec, &mut diagnostics));
    let scenario = parse_singleton("scenario", &mut diagnostics)
        .and_then(|rec| deserialize_staged_record::<ScenarioHeaderRecord>("scenario", rec, &mut diagnostics));
    let skin_reference = parse_singleton("skin_reference", &mut diagnostics)
        .and_then(|rec| deserialize_staged_record::<SkinReferenceRecord>("skin_reference", rec, &mut diagnostics));

    macro_rules! parse_array_domain {
        ($section:expr, $ty:ty) => {{
            let mut items = Vec::new();
            let mut recs = Vec::new();
            if let Some(bucket) = array_buckets.get($section) {
                for (id_key, rec) in bucket {
                    if let Some(parsed) = deserialize_staged_record::<$ty>($section, rec, &mut diagnostics) {
                        items.push(parsed);
                        recs.push((id_key.clone(), rec));
                    }
                }
            }
            (items, recs)
        }};
    }

    let (commodities, commodity_recs) = parse_array_domain!("commodities", CommodityRecord);
    let (tech_components, _) = parse_array_domain!("tech_components", TechComponentRecord);
    let (theory_domains, _) = parse_array_domain!("theory_domains", TheoryDomainRecord);
    let (processes, process_recs) = parse_array_domain!("processes", ProcessRecord);
    let (carriers, carrier_recs) = parse_array_domain!("carriers", CarrierRecord);
    let (infra_classes, infra_recs) = parse_array_domain!("infra_classes", InfraClassRecord);
    let (cohort_archetypes, cohort_recs) = parse_array_domain!("cohort_archetypes", CohortArchetypeRecord);
    let (vocations, vocation_recs) = parse_array_domain!("vocations", VocationRecord);
    let (traits, _) = parse_array_domain!("traits", CharacterTraitRecord);
    let (succession_rules, _) = parse_array_domain!("succession_rules", SuccessionRuleRecord);
    let (governance_models, _) = parse_array_domain!("governance_models", GovernanceModelRecord);
    let (policy_laws, _) = parse_array_domain!("policy_laws", PolicyLawRecord);
    let (formation_templates, _) = parse_array_domain!("formation_templates", FormationTemplateRecord);
    let (doctrines, _) = parse_array_domain!("doctrines", DoctrineRecord);
    let (starting_settlements, settlement_recs) =
        parse_array_domain!("starting_settlements", StartingSettlementRecord);
    let (starting_characters, character_recs) =
        parse_array_domain!("starting_characters", StartingCharacterRecord);
    let (event_classes, event_recs) = parse_array_domain!("event_classes", EventClassRecord);
    let (strings, _) = parse_array_domain!("strings", LocalizationStringRecord);

    // Check required non-empty domains (ENG-MOD-006)
    for (section, len) in [
        ("commodities", commodities.len()),
        ("processes", processes.len()),
        ("carriers", carriers.len()),
        ("infra_classes", infra_classes.len()),
        ("cohort_archetypes", cohort_archetypes.len()),
        ("vocations", vocations.len()),
        ("traits", traits.len()),
        ("succession_rules", succession_rules.len()),
        ("governance_models", governance_models.len()),
        ("policy_laws", policy_laws.len()),
        ("formation_templates", formation_templates.len()),
        ("doctrines", doctrines.len()),
        ("event_classes", event_classes.len()),
        ("strings", strings.len()),
        ("starting_settlements", starting_settlements.len()),
    ] {
        let bucket_has_records = array_buckets
            .get(section)
            .is_some_and(|bucket| !bucket.is_empty());
        if len == 0 && !bucket_has_records {
            diagnostics.push(ModDiagnostic {
                severity: DiagnosticSeverity::Error,
                layer: "Bundle".to_string(),
                file_path: section.to_string(),
                line: 1,
                column: 1,
                field_path: section.to_string(),
                message: format!("Required domain table '{section}' is missing or empty"),
            });
        }
    }

    // Numeric range validation (ENG-MOD-003)
    if let (Some(pg), Some(rec)) = (&planet_generation, singleton_buckets.get("planet_generation")) {
        if pg.land_fraction_target <= 0.0 || pg.land_fraction_target >= 1.0 {
            push_field_diag(
                &mut diagnostics,
                "planet_generation",
                rec,
                "land_fraction_target",
                "land_fraction_target must lie strictly in (0.0, 1.0)".to_string(),
            );
        }
        if pg.tectonic_plate_count == 0 {
            push_field_diag(
                &mut diagnostics,
                "planet_generation",
                rec,
                "tectonic_plate_count",
                "tectonic_plate_count must be >= 1".to_string(),
            );
        }
    }

    if let (Some(cal), Some(rec)) = (&calendar, singleton_buckets.get("calendar")) {
        if cal.hours_per_day == 0 {
            push_field_diag(
                &mut diagnostics,
                "calendar",
                rec,
                "hours_per_day",
                "hours_per_day must be >= 1".to_string(),
            );
        }
        if cal.month_names.len() != cal.month_lengths.len() {
            push_field_diag(
                &mut diagnostics,
                "calendar",
                rec,
                "month_lengths",
                "month_names and month_lengths must have identical length".to_string(),
            );
        } else {
            let sum: u32 = cal.month_lengths.iter().sum();
            if sum != cal.days_per_year {
                push_field_diag(
                    &mut diagnostics,
                    "calendar",
                    rec,
                    "days_per_year",
                    format!("days_per_year ({}) must equal sum(month_lengths) ({sum})", cal.days_per_year),
                );
            }
        }
    }

    for (c, (_, rec)) in commodities.iter().zip(commodity_recs.iter()) {
        if c.mass_per_unit_kg <= 0.0 {
            push_field_diag(
                &mut diagnostics,
                "commodities",
                rec,
                "mass_per_unit_kg",
                format!("Commodity '{}' mass_per_unit_kg must be > 0.0", c.id),
            );
        }
        if c.volume_per_unit_m3 <= 0.0 {
            push_field_diag(
                &mut diagnostics,
                "commodities",
                rec,
                "volume_per_unit_m3",
                format!("Commodity '{}' volume_per_unit_m3 must be > 0.0", c.id),
            );
        }
        if c.spoilage_rate_per_hour < 0.0 || c.spoilage_rate_per_hour > 1.0 {
            push_field_diag(
                &mut diagnostics,
                "commodities",
                rec,
                "spoilage_rate_per_hour",
                format!("Commodity '{}' spoilage_rate_per_hour must lie in [0.0, 1.0]", c.id),
            );
        }
        if c.tool_wear_per_hour < 0.0 {
            push_field_diag(
                &mut diagnostics,
                "commodities",
                rec,
                "tool_wear_per_hour",
                format!("Commodity '{}' tool_wear_per_hour must be >= 0.0", c.id),
            );
        }
    }

    for (p, (_, rec)) in processes.iter().zip(process_recs.iter()) {
        if p.cycle_hours == 0 {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "cycle_hours",
                format!("Process '{}' cycle_hours must be >= 1", p.id),
            );
        }
        if p.workforce_hours_required <= 0.0 {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "workforce_hours_required",
                format!("Process '{}' workforce_hours_required must be > 0.0", p.id),
            );
        }
        if p.weather_halt_mud_threshold < 0.0 || p.weather_halt_mud_threshold > 1.0 {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "weather_halt_mud_threshold",
                format!("Process '{}' weather_halt_mud_threshold must lie in [0.0, 1.0]", p.id),
            );
        }
    }

    for (ch, (_, rec)) in starting_characters.iter().zip(character_recs.iter()) {
        if !(0.0..=1.0).contains(&ch.precision_01) {
            push_field_diag(
                &mut diagnostics,
                "starting_characters",
                rec,
                "precision_01",
                format!("Character '{}' precision_01 must lie in [0.0, 1.0]", ch.id),
            );
        }
        if !(0.0..=1.0).contains(&ch.competence) {
            push_field_diag(
                &mut diagnostics,
                "starting_characters",
                rec,
                "competence",
                format!("Character '{}' competence must lie in [0.0, 1.0]", ch.id),
            );
        }
    }

    for (cohort, (_, rec)) in cohort_archetypes.iter().zip(cohort_recs.iter()) {
        if cohort.age_min_years > cohort.age_max_years {
            push_field_diag(
                &mut diagnostics,
                "cohort_archetypes",
                rec,
                "age_max_years",
                format!("Cohort archetype '{}' age_max_years must be >= age_min_years", cohort.id),
            );
        }
    }

    for (infra, (_, rec)) in infra_classes.iter().zip(infra_recs.iter()) {
        if infra.speed_multiplier <= 0.0 {
            push_field_diag(
                &mut diagnostics,
                "infra_classes",
                rec,
                "speed_multiplier",
                format!("Infra '{}' speed_multiplier must be > 0.0", infra.id),
            );
        }
    }

    for (voc, (_, rec)) in vocations.iter().zip(vocation_recs.iter()) {
        if voc.base_skill_gain_per_day < 0.0 {
            push_field_diag(
                &mut diagnostics,
                "vocations",
                rec,
                "base_skill_gain_per_day",
                format!("Vocation '{}' base_skill_gain_per_day must be >= 0.0", voc.id),
            );
        }
    }

    // Cross-domain foreign key validation (ENG-MOD-005)
    let commodity_keys: BTreeSet<&str> = commodities.iter().map(|c| c.id.as_str()).collect();
    let vocation_keys: BTreeSet<&str> = vocations.iter().map(|v| v.id.as_str()).collect();
    let process_keys: BTreeSet<&str> = processes.iter().map(|p| p.id.as_str()).collect();
    let infra_keys: BTreeSet<&str> = infra_classes.iter().map(|i| i.id.as_str()).collect();
    let archetype_keys: BTreeSet<&str> = cohort_archetypes.iter().map(|a| a.id.as_str()).collect();
    let trait_keys: BTreeSet<&str> = traits.iter().map(|t| t.id.as_str()).collect();
    let settlement_keys: BTreeSet<&str> = starting_settlements.iter().map(|s| s.id.as_str()).collect();
    let gov_keys: BTreeSet<&str> = governance_models.iter().map(|g| g.id.as_str()).collect();
    let succ_keys: BTreeSet<&str> = succession_rules.iter().map(|s| s.id.as_str()).collect();
    let policy_keys: BTreeSet<&str> = policy_laws.iter().map(|p| p.id.as_str()).collect();
    let string_keys: BTreeSet<&str> = strings.iter().map(|s| s.id.as_str()).collect();

    for (p, (_, rec)) in processes.iter().zip(process_recs.iter()) {
        if !vocation_keys.contains(p.required_vocation.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "required_vocation",
                format!("Process '{}' references undefined vocation '{}'", p.id, p.required_vocation),
            );
        }
        if let Some(tool) = &p.tool_commodity
            && !commodity_keys.contains(tool.as_str())
        {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "tool_commodity",
                format!("Process '{}' references undefined tool commodity '{}'", p.id, tool),
            );
        }
        if let Some(fb) = &p.fallback_tool_commodity
            && !commodity_keys.contains(fb.as_str())
        {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "fallback_tool_commodity",
                format!(
                    "Process '{}' references undefined fallback tool commodity '{}'",
                    p.id, fb
                ),
            );
        }
        if let Some(waste) = &p.waste_commodity
            && !commodity_keys.contains(waste.as_str())
        {
            push_field_diag(
                &mut diagnostics,
                "processes",
                rec,
                "waste_commodity",
                format!(
                    "Process '{}' references undefined waste commodity '{}'",
                    p.id, waste
                ),
            );
        }
        for (idx, inp) in p.inputs.iter().enumerate() {
            if !commodity_keys.contains(inp.commodity.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "processes",
                    rec,
                    &format!("inputs[{idx}].commodity"),
                    format!("Process '{}' input references undefined commodity '{}'", p.id, inp.commodity),
                );
            }
        }
        for (idx, out) in p.outputs.iter().enumerate() {
            if !commodity_keys.contains(out.commodity.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "processes",
                    rec,
                    &format!("outputs[{idx}].commodity"),
                    format!("Process '{}' output references undefined commodity '{}'", p.id, out.commodity),
                );
            }
        }
    }

    for (car, (_, rec)) in carriers.iter().zip(carrier_recs.iter()) {
        if !commodity_keys.contains(car.fodder_or_fuel_commodity.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "carriers",
                rec,
                "fodder_or_fuel_commodity",
                format!(
                    "Carrier '{}' references undefined fuel commodity '{}'",
                    car.id, car.fodder_or_fuel_commodity
                ),
            );
        }
        for (idx, inf) in car.allowed_infra_classes.iter().enumerate() {
            if !infra_keys.contains(inf.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "carriers",
                    rec,
                    &format!("allowed_infra_classes[{idx}]"),
                    format!(
                        "Carrier '{}' references undefined infra class '{}'",
                        car.id, inf
                    ),
                );
            }
        }
        if let Some(spare) = &car.spare_parts_commodity
            && !commodity_keys.contains(spare.as_str())
        {
            push_field_diag(
                &mut diagnostics,
                "carriers",
                rec,
                "spare_parts_commodity",
                format!(
                    "Carrier '{}' references undefined spare parts commodity '{}'",
                    car.id, spare
                ),
            );
        }
    }

    for (st, (_, rec)) in starting_settlements.iter().zip(settlement_recs.iter()) {
        for (idx, sp) in st.stockpiles.iter().enumerate() {
            if !commodity_keys.contains(sp.commodity.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "starting_settlements",
                    rec,
                    &format!("stockpiles[{idx}].commodity"),
                    format!(
                        "Settlement '{}' stockpile references undefined commodity '{}'",
                        st.id, sp.commodity
                    ),
                );
            }
        }
        for (idx, ch) in st.cohorts.iter().enumerate() {
            if !archetype_keys.contains(ch.archetype.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "starting_settlements",
                    rec,
                    &format!("cohorts[{idx}].archetype"),
                    format!(
                        "Settlement '{}' cohort references undefined archetype '{}'",
                        st.id, ch.archetype
                    ),
                );
            }
            if !vocation_keys.contains(ch.vocation.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "starting_settlements",
                    rec,
                    &format!("cohorts[{idx}].vocation"),
                    format!(
                        "Settlement '{}' cohort references undefined vocation '{}'",
                        st.id, ch.vocation
                    ),
                );
            }
        }
        for (idx, fac) in st.initial_facilities.iter().enumerate() {
            if !process_keys.contains(fac.process.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "starting_settlements",
                    rec,
                    &format!("initial_facilities[{idx}].process"),
                    format!(
                        "Settlement '{}' facility references undefined process '{}'",
                        st.id, fac.process
                    ),
                );
            }
        }
    }

    for (ch, (_, rec)) in starting_characters.iter().zip(character_recs.iter()) {
        if !settlement_keys.contains(ch.home_settlement.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "starting_characters",
                rec,
                "home_settlement",
                format!(
                    "Character '{}' references undefined settlement '{}'",
                    ch.id, ch.home_settlement
                ),
            );
        }
        for (idx, tr) in ch.traits.iter().enumerate() {
            if !trait_keys.contains(tr.as_str()) {
                push_field_diag(
                    &mut diagnostics,
                    "starting_characters",
                    rec,
                    &format!("traits[{idx}]"),
                    format!("Character '{}' references undefined trait '{}'", ch.id, tr),
                );
            }
        }
    }

    if let (Some(sc), Some(rec)) = (&scenario, singleton_buckets.get("scenario")) {
        if !gov_keys.contains(sc.governance_model.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "scenario",
                rec,
                "governance_model",
                format!("Scenario '{}' references undefined governance_model '{}'", sc.id, sc.governance_model),
            );
        }
        if !succ_keys.contains(sc.succession_rule.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "scenario",
                rec,
                "succession_rule",
                format!("Scenario '{}' references undefined succession_rule '{}'", sc.id, sc.succession_rule),
            );
        }
        if !policy_keys.contains(sc.active_policy.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "scenario",
                rec,
                "active_policy",
                format!("Scenario '{}' references undefined active_policy '{}'", sc.id, sc.active_policy),
            );
        }
        if !settlement_keys.contains(sc.hq_settlement_id.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "scenario",
                rec,
                "hq_settlement_id",
                format!("Scenario '{}' references undefined hq_settlement_id '{}'", sc.id, sc.hq_settlement_id),
            );
        }
    }

    for (ev, (_, rec)) in event_classes.iter().zip(event_recs.iter()) {
        if !string_keys.contains(ev.template_key.as_str()) {
            push_field_diag(
                &mut diagnostics,
                "event_classes",
                rec,
                "template_key",
                format!(
                    "Event class '{}' references undefined localization key '{}'",
                    ev.id, ev.template_key
                ),
            );
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let planet_rec = planet_generation.expect("validated present");
    let calendar_rec = calendar.expect("validated present");
    let scenario_rec = scenario.expect("validated present");
    let skin_rec = skin_reference.expect("validated present");

    let merged_doc = MergedModDocument {
        planet: PlanetGenDomain {
            planet_generation: Some(planet_rec.clone()),
        },
        commodities_and_knowledge: CommoditiesAndKnowledgeDomain {
            commodities: commodities.clone(),
            tech_components,
            theory_domains,
        },
        processes: ProcessesDomain {
            processes: processes.clone(),
        },
        logistics: LogisticsDomain {
            carriers: carriers.clone(),
            infra_classes: infra_classes.clone(),
        },
        population: PopulationDomain {
            cohort_archetypes: cohort_archetypes.clone(),
            vocations: vocations.clone(),
        },
        characters: CharactersDomain {
            traits,
            succession_rules,
        },
        governance: GovernanceDomain {
            governance_models: governance_models.clone(),
            policy_laws,
        },
        warfare: WarfareDomain {
            formation_templates,
            doctrines,
        },
        scenario_and_calendar: ScenarioAndCalendarDomain {
            calendar: Some(calendar_rec.clone()),
            scenario: Some(scenario_rec.clone()),
            starting_settlements: starting_settlements.clone(),
            starting_characters: starting_characters.clone(),
        },
        notifications_and_loc: NotificationsAndLocDomain {
            event_classes: event_classes.clone(),
            strings: strings.clone(),
        },
        skin_ref: SkinRefDomain {
            skin_reference: Some(skin_rec),
        },
    };

    let canonical_bytes = serde_json::to_vec(&merged_doc).unwrap_or_default();
    let digest_hash = blake3::hash(&canonical_bytes);
    let digest_bytes = *digest_hash.as_bytes();
    let digest_hex = digest_hash.to_hex().to_string();

    let mut commodity_defs = Vec::with_capacity(commodities.len());
    let mut commodity_by_key = BTreeMap::new();
    for (idx, c) in commodities.iter().enumerate() {
        let cid = CommodityId(idx as u16);
        commodity_by_key.insert(c.id.clone(), cid);
        commodity_defs.push(CommodityDefView {
            id: cid,
            key: c.id.clone(),
            display_name: c.display_name.clone(),
            category: c.category.clone(),
            mass_per_unit_kg: c.mass_per_unit_kg,
            volume_per_unit_m3: c.volume_per_unit_m3,
            spoilage_rate_per_hour: c.spoilage_rate_per_hour,
            energy_mj_per_kg: c.energy_mj_per_kg,
            structural_strength: c.structural_strength,
            is_tool: c.is_tool,
            tool_wear_per_hour: c.tool_wear_per_hour,
            caloric_mcal_per_kg: c.caloric_mcal_per_kg,
            potable_liters_per_kg: c.potable_liters_per_kg,
            heating_mj_per_kg: c.heating_mj_per_kg,
            is_clothing: c.is_clothing,
            tool_hardness_rating: c.tool_hardness_rating,
            is_slag_or_waste: c.is_slag_or_waste,
        });
    }

    let mut vocation_defs = Vec::with_capacity(vocations.len());
    let mut vocation_by_key = BTreeMap::new();
    for (idx, v) in vocations.iter().enumerate() {
        let vid = VocationId(idx as u16);
        vocation_by_key.insert(v.id.clone(), vid);
        vocation_defs.push(VocationDefView {
            id: vid,
            key: v.id.clone(),
            display_name: v.display_name.clone(),
            base_skill_gain_per_day: v.base_skill_gain_per_day,
            skill_decay_per_day: v.skill_decay_per_day,
        });
    }

    let mut archetype_defs = Vec::with_capacity(cohort_archetypes.len());
    let mut archetype_by_key = BTreeMap::new();
    for (idx, a) in cohort_archetypes.iter().enumerate() {
        archetype_by_key.insert(a.id.clone(), idx);
        archetype_defs.push(CohortArchetypeDefView {
            index: idx as u16,
            key: a.id.clone(),
            display_name: a.display_name.clone(),
            daily_grain_need_kg: a.daily_grain_need_kg,
            daily_water_need_kg: a.daily_water_need_kg,
            daily_fuel_need_kg_below_10c: a.daily_fuel_need_kg_below_10c,
            daily_clothing_wear_kg: a.daily_clothing_wear_kg,
            base_fertility_annual: a.base_fertility_annual,
            base_mortality_annual: a.base_mortality_annual,
            initial_literacy: a.initial_literacy,
            dependents_share_01: a.dependents_share_01,
            elders_share_01: a.elders_share_01,
        });
    }

    let mut process_defs = Vec::with_capacity(processes.len());
    let mut process_by_key = BTreeMap::new();
    for (idx, p) in processes.iter().enumerate() {
        let pid = ProcessId(idx as u16);
        process_by_key.insert(p.id.clone(), pid);
        process_defs.push(ProcessDefView {
            id: pid,
            key: p.id.clone(),
            display_name: p.display_name.clone(),
            required_vocation: vocation_by_key[p.required_vocation.as_str()],
            cycle_hours: p.cycle_hours,
            workforce_hours_required: p.workforce_hours_required,
            tool_commodity: p.tool_commodity.as_ref().map(|k| commodity_by_key[k.as_str()]),
            fallback_tool_commodity: p
                .fallback_tool_commodity
                .as_ref()
                .map(|k| commodity_by_key[k.as_str()]),
            waste_commodity: p.waste_commodity.as_ref().map(|k| commodity_by_key[k.as_str()]),
            weather_halt_mud_threshold: p.weather_halt_mud_threshold,
            is_outdoor_construction: p.is_outdoor_construction,
            is_underground_mine: p.is_underground_mine,
            is_crop_cultivation: p.is_crop_cultivation,
            is_mine_drainage: p.is_mine_drainage,
            min_temp_c: p.min_temp_c,
            max_temp_c: p.max_temp_c,
            optimal_moisture_01: p.optimal_moisture_01,
            tool_wear_intensity: p.tool_wear_intensity,
            inputs: p
                .inputs
                .iter()
                .map(|i| ProcessQuantityView {
                    commodity: commodity_by_key[i.commodity.as_str()],
                    amount_kg: i.amount_kg,
                })
                .collect(),
            outputs: p
                .outputs
                .iter()
                .map(|o| ProcessQuantityView {
                    commodity: commodity_by_key[o.commodity.as_str()],
                    amount_kg: o.amount_kg,
                })
                .collect(),
        });
    }

    let mut infra_defs = Vec::with_capacity(infra_classes.len());
    let mut infra_by_key = BTreeMap::new();
    for (idx, inf) in infra_classes.iter().enumerate() {
        let iid = InfraClassId(idx as u16);
        infra_by_key.insert(inf.id.clone(), iid);
        infra_defs.push(InfraDefView {
            id: iid,
            key: inf.id.clone(),
            display_name: inf.display_name.clone(),
            throughput_tonnes_per_hour: inf.throughput_tonnes_per_hour,
            speed_multiplier: inf.speed_multiplier,
            mud_shield_factor: inf.mud_shield_factor,
        });
    }

    let mut carrier_defs = Vec::with_capacity(carriers.len());
    let mut carrier_by_key = BTreeMap::new();
    for (idx, car) in carriers.iter().enumerate() {
        let cid = CarrierClassId(idx as u16);
        carrier_by_key.insert(car.id.clone(), cid);
        carrier_defs.push(CarrierDefView {
            id: cid,
            key: car.id.clone(),
            display_name: car.display_name.clone(),
            max_payload_kg: car.max_payload_kg,
            max_payload_m3: car.max_payload_m3,
            base_speed_km_hr: car.base_speed_km_hr,
            max_grade_rad: car.max_grade_rad,
            mud_penalty_factor: car.mud_penalty_factor,
            crew_required: car.crew_required,
            fodder_or_fuel_commodity: commodity_by_key[car.fodder_or_fuel_commodity.as_str()],
            fuel_burn_kg_per_hour: car.fuel_burn_kg_per_hour,
            allowed_infra_classes: car
                .allowed_infra_classes
                .iter()
                .map(|k| infra_by_key[k.as_str()])
                .collect(),
            is_watercraft: car.is_watercraft,
            uses_wind_sail: car.uses_wind_sail,
            max_ice_thickness_m: car.max_ice_thickness_m,
            spare_parts_commodity: car
                .spare_parts_commodity
                .as_ref()
                .map(|k| commodity_by_key[k.as_str()]),
            spare_parts_wear_per_km: car.spare_parts_wear_per_km,
        });
    }

    let mut event_by_key = BTreeMap::new();
    let mut notification_default_by_key = BTreeMap::new();
    let mut notification_list = Vec::with_capacity(event_classes.len());
    for (idx, ev) in event_classes.iter().enumerate() {
        let eid = EventClassId(idx as u16);
        let tier = NotificationBehaviorKind::from_mod_str(&ev.default_tier);
        event_by_key.insert(ev.id.clone(), eid);
        notification_default_by_key.insert(ev.id.clone(), tier);
        notification_list.push((eid, ev.id.clone(), ev.display_name.clone(), tier));
    }

    let localization_by_key: BTreeMap<String, String> = strings
        .into_iter()
        .map(|s| (s.id, s.text))
        .collect();

    let gov_model = governance_models
        .iter()
        .find(|g| g.id == scenario_rec.governance_model);
    let (gov_base_attention, gov_clerical_overhead, gov_hop_latency) = match gov_model {
        Some(g) => (
            g.base_attention_capacity,
            g.clerical_overhead_per_mandate,
            g.hierarchy_hop_latency_hours,
        ),
        None => (100.0, 8.0, 4),
    };

    let calendar_view = CalendarConfigView {
        hours_per_day: calendar_rec.hours_per_day,
        days_per_year: calendar_rec.days_per_year,
        year_prefix: calendar_rec.year_prefix,
        starting_year: calendar_rec.starting_year,
        month_names: calendar_rec.month_names,
        month_lengths: calendar_rec.month_lengths,
    };

    let planet_gen_view = PlanetGenConfigView {
        id: planet_rec.id.clone(),
        goldberg_subdivision: planet_rec.goldberg_subdivision,
        tectonic_plate_count: planet_rec.tectonic_plate_count,
        land_fraction_target: planet_rec.land_fraction_target,
        axial_tilt_deg: planet_rec.axial_tilt_deg,
        solar_constant_w_m2: planet_rec.solar_constant_w_m2,
        rotation_period_hours: planet_rec.rotation_period_hours,
        mean_radius_km: planet_rec.mean_radius_km,
        base_sea_level_m: planet_rec.base_sea_level_m,
        max_orogenic_uplift_m: planet_rec.max_orogenic_uplift_m,
        planetary_density_kg_m3: planet_rec.planetary_density_kg_m3,
        natural_bounds: NaturalBoundsView {
            ore_count: planet_rec.natural_bounds.ore_count,
            flux_count: planet_rec.natural_bounds.flux_count,
            fuel_count: planet_rec.natural_bounds.fuel_count,
            crop_count: planet_rec.natural_bounds.crop_count,
            fauna_count: planet_rec.natural_bounds.fauna_count,
            pathogen_count: planet_rec.natural_bounds.pathogen_count,
            ore_melting_k_range: planet_rec.natural_bounds.ore_melting_k_range,
            ore_purity_range: planet_rec.natural_bounds.ore_purity_range,
            fuel_enthalpy_mj_kg_range: planet_rec.natural_bounds.fuel_enthalpy_mj_kg_range,
            crop_opt_temp_c_range: planet_rec.natural_bounds.crop_opt_temp_c_range,
        },
    };

    let scenario_view = ScenarioInitView {
        scenario_key: scenario_rec.id,
        display_name: scenario_rec.display_name,
        archetype: ScenarioArchetypeKind::from_profile_str(&scenario_rec.starting_profile),
        hq_settlement_key: scenario_rec.hq_settlement_id,
        hq_sight_radius_cells: scenario_rec.hq_sight_radius_cells,
        unexplored_surrounding_cells: scenario_rec.unexplored_surrounding_cells,
        goldberg_subdivision: planet_rec.goldberg_subdivision,
        governance_base_attention: gov_base_attention,
        clerical_overhead_per_mandate: gov_clerical_overhead,
        hierarchy_hop_latency_hours: gov_hop_latency,
        settlements: starting_settlements
            .iter()
            .map(|st| ScenarioSettlementSeed {
                key: st.id.clone(),
                display_name: st.display_name.clone(),
                cell_index: st.cell_index,
                initial_population: st.initial_population,
                stockpiles: st
                    .stockpiles
                    .iter()
                    .map(|sp| (commodity_by_key[sp.commodity.as_str()], sp.mass_kg, sp.purity))
                    .collect(),
                stockpile_roofs_01: st.stockpiles.iter().map(|sp| sp.roof_protection_01).collect(),
                cohorts: st
                    .cohorts
                    .iter()
                    .map(|ch| (vocation_by_key[ch.vocation.as_str()], ch.count))
                    .collect(),
                cohort_archetype_indices: st
                    .cohorts
                    .iter()
                    .map(|ch| archetype_by_key[ch.archetype.as_str()])
                    .collect(),
                initial_facilities: st
                    .initial_facilities
                    .iter()
                    .map(|fac| (process_by_key[fac.process.as_str()], fac.workers))
                    .collect(),
                initial_facility_owners: st
                    .initial_facilities
                    .iter()
                    .map(|fac| ActorId(fac.owner_actor_id))
                    .collect(),
                initial_shaft_water_01: st.initial_shaft_water_01,
            })
            .collect(),
        characters: starting_characters
            .iter()
            .map(|ch| ScenarioCharacterSeed {
                key: ch.id.clone(),
                display_name: ch.display_name.clone(),
                role_title: ch.role_title.clone(),
                home_settlement_key: ch.home_settlement.clone(),
                competence: ch.competence,
                precision_01: ch.precision_01,
                honesty_vs_sycophancy: ch.honesty_vs_sycophancy,
                caution_vs_alarmism: ch.caution_vs_alarmism,
                initiative_01: ch.initiative_01,
                adaptability_01: ch.adaptability_01,
                ambition_01: ch.ambition_01,
                loyalty_01: ch.loyalty_01,
                greed_skimming_01: ch.greed_skimming_01,
            })
            .collect(),
    };

    Ok(CompiledModBundle {
        merged_doc,
        commodity_defs,
        commodity_by_key,
        process_defs,
        carrier_defs,
        carrier_by_key,
        infra_defs,
        infra_by_key,
        vocation_defs,
        archetype_defs,
        event_by_key,
        notification_default_by_key,
        notification_list,
        localization_by_key,
        calendar_view,
        planet_gen_view,
        scenario_view,
        digest_bytes,
        digest_hex,
    })
}

/// Exports deterministic Draft 2020-12 JSON Schema files for all 11 externalized mod domains
/// plus the `pack.toml` manifest header into `out_dir`.
pub fn export_json_schemas(out_dir: &Path) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(out_dir)
        .map_err(|e| format!("Failed creating schema output directory {}: {e}", out_dir.display()))?;

    let schemas: [(&str, schemars::Schema); 12] = [
        ("00_pack_manifest.schema.json", schema_for!(PackManifest)),
        ("01_planet_generation.schema.json", schema_for!(PlanetGenDomain)),
        ("02_commodities_and_knowledge.schema.json", schema_for!(CommoditiesAndKnowledgeDomain)),
        ("03_processes.schema.json", schema_for!(ProcessesDomain)),
        ("04_logistics.schema.json", schema_for!(LogisticsDomain)),
        ("05_population.schema.json", schema_for!(PopulationDomain)),
        ("06_characters_and_succession.schema.json", schema_for!(CharactersDomain)),
        ("07_governance_and_laws.schema.json", schema_for!(GovernanceDomain)),
        ("08_warfare_and_doctrines.schema.json", schema_for!(WarfareDomain)),
        ("09_scenario_and_calendar.schema.json", schema_for!(ScenarioAndCalendarDomain)),
        ("10_notifications_and_loc.schema.json", schema_for!(NotificationsAndLocDomain)),
        ("11_skin_reference.schema.json", schema_for!(SkinRefDomain)),
    ];

    let mut written = Vec::with_capacity(schemas.len());
    for (filename, schema) in schemas {
        let mut pretty = serde_json::to_string_pretty(&schema)
            .map_err(|e| format!("Failed serializing schema {filename}: {e}"))?;
        pretty.push('\n');
        let target = out_dir.join(filename);
        fs::write(&target, pretty.as_bytes())
            .map_err(|e| format!("Failed writing schema {}: {e}", target.display()))?;
        written.push(target);
    }
    Ok(written)
}
