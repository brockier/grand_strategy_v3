//! Workspace compliance, licensing, provenance, isolation, and requirement traceability auditing.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// Parsed requirement status per `requirements/*.md` Section 1.2.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ReqStatus {
    Active,
    Draft,
    Superseded,
    Removed,
}

/// Requirement record parsed from `requirements/engine_requirements.md` or `requirements/ui_requirements.md`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedRequirement {
    pub id: String,
    pub status: ReqStatus,
    pub milestone: u8,
    pub verification_layer: String,
    pub lineage: String,
}

/// Requirement citation discovered inside automated tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestCitation {
    pub req_id: String,
    pub file_path: PathBuf,
    pub line_number: usize,
}

#[derive(Debug, Deserialize)]
struct DenyConfig {
    licenses: DenyLicenses,
}

#[derive(Debug, Deserialize)]
struct DenyLicenses {
    unlicensed: String,
    copyleft: String,
    allow: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    license: Option<String>,
    license_file: Option<String>,
    repository: Option<String>,
    authors: Vec<String>,
    manifest_path: String,
    dependencies: Vec<CargoDep>,
}

#[derive(Debug, Deserialize)]
struct CargoDep {
    name: String,
    #[serde(default)]
    kind: Option<String>,
}

/// AI Asset Provenance Manifest (`tools/asset-pipeline/manifests/provenance.toml`).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetProvenanceManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub assets: Vec<AssetProvenanceRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SteamAiCategory {
    VisualTexture,
    Portrait,
    Icon,
    AudioFoley,
    MusicStem,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetProvenanceRecord {
    pub asset_rel_path: String,
    pub generator_model: String,
    pub generator_version: String,
    pub license_tier: String,
    pub commercial_rights_verified: bool,
    pub human_post_processing: String,
    pub steam_disclosure_category: String,
}

#[derive(Clone, Debug, Deserialize)]
struct BaselineAssetsManifest {
    #[allow(dead_code)]
    schema_version: u32,
    #[serde(default)]
    fonts: Vec<BaselineAssetEntry>,
    #[serde(default)]
    baseline_assets: Vec<BaselineAssetEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct BaselineAssetEntry {
    rel_path: String,
    spdx_license: String,
    license_file: String,
    #[allow(dead_code)]
    copyright_notice: String,
}

#[derive(Clone, Debug, Serialize)]
struct SteamAiDisclosureReport {
    schema_version: u32,
    total_verified_assets: usize,
    categories: BTreeMap<String, usize>,
    generator_models: Vec<String>,
    assets: Vec<AssetProvenanceRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SpdxTok {
    LParen,
    RParen,
    And,
    Or,
    With,
    Ident(String),
}

fn tokenize_spdx(expr: &str) -> Vec<SpdxTok> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, toks: &mut Vec<SpdxTok>| {
        if cur.is_empty() {
            return;
        }
        let s = std::mem::take(cur);
        match s.as_str() {
            "AND" => toks.push(SpdxTok::And),
            "OR" => toks.push(SpdxTok::Or),
            "WITH" => toks.push(SpdxTok::With),
            _ => toks.push(SpdxTok::Ident(s)),
        }
    };
    for ch in expr.chars() {
        match ch {
            '(' => {
                flush(&mut cur, &mut toks);
                toks.push(SpdxTok::LParen);
            }
            ')' => {
                flush(&mut cur, &mut toks);
                toks.push(SpdxTok::RParen);
            }
            '/' => {
                // Normalize legacy slash separators occasionally found in crate metadata
                flush(&mut cur, &mut toks);
                toks.push(SpdxTok::Or);
            }
            c if c.is_whitespace() => flush(&mut cur, &mut toks),
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut toks);
    toks
}

fn eval_spdx_or(toks: &[SpdxTok], pos: &mut usize, allow: &BTreeSet<String>) -> Result<bool, String> {
    let mut lhs = eval_spdx_and(toks, pos, allow)?;
    while matches!(toks.get(*pos), Some(SpdxTok::Or)) {
        *pos += 1;
        let rhs = eval_spdx_and(toks, pos, allow)?;
        lhs = lhs || rhs;
    }
    Ok(lhs)
}

fn eval_spdx_and(toks: &[SpdxTok], pos: &mut usize, allow: &BTreeSet<String>) -> Result<bool, String> {
    let mut lhs = eval_spdx_atom(toks, pos, allow)?;
    while matches!(toks.get(*pos), Some(SpdxTok::And)) {
        *pos += 1;
        let rhs = eval_spdx_atom(toks, pos, allow)?;
        lhs = lhs && rhs;
    }
    Ok(lhs)
}

fn eval_spdx_atom(toks: &[SpdxTok], pos: &mut usize, allow: &BTreeSet<String>) -> Result<bool, String> {
    match toks.get(*pos) {
        Some(SpdxTok::LParen) => {
            *pos += 1;
            let inner = eval_spdx_or(toks, pos, allow)?;
            if !matches!(toks.get(*pos), Some(SpdxTok::RParen)) {
                return Err("Unclosed '(' in SPDX expression".to_string());
            }
            *pos += 1;
            Ok(inner)
        }
        Some(SpdxTok::Ident(id)) => {
            *pos += 1;
            let mut ok = allow.contains(id.as_str());
            if matches!(toks.get(*pos), Some(SpdxTok::With)) {
                *pos += 1;
                match toks.get(*pos) {
                    Some(SpdxTok::Ident(exc)) => {
                        *pos += 1;
                        // Standard linking exceptions preserve the parent permissive base license.
                        if exc != "LLVM-exception" && !allow.contains(exc.as_str()) {
                            ok = false;
                        }
                    }
                    _ => return Err("Expected exception identifier after WITH".to_string()),
                }
            }
            Ok(ok)
        }
        _ => Err("Unexpected token in SPDX expression".to_string()),
    }
}

/// Evaluates whether an SPDX license expression is satisfied by the allowlist.
pub fn spdx_satisfied_by_allowlist(expr: &str, allow: &BTreeSet<String>) -> Result<bool, String> {
    let toks = tokenize_spdx(expr);
    if toks.is_empty() {
        return Ok(false);
    }
    let mut pos = 0usize;
    let res = eval_spdx_or(&toks, &mut pos, allow)?;
    if pos != toks.len() {
        return Err(format!("Trailing SPDX tokens in '{expr}'"));
    }
    Ok(res)
}

/// Runs `cargo metadata` or parses supplied JSON fixture and validates every non-workspace crate
/// against `deny.toml` permissive SPDX allowlist rules (`ENG-LIC-001`).
pub fn check_licenses(
    engine_dir: &Path,
    deny_toml_path: &Path,
    metadata_json_override: Option<&str>,
) -> Result<usize, Vec<String>> {
    let deny_raw = fs::read_to_string(deny_toml_path)
        .map_err(|e| vec![format!("Failed reading {}: {e}", deny_toml_path.display())])?;
    let deny: DenyConfig = toml::from_str(&deny_raw)
        .map_err(|e| vec![format!("Invalid {}: {e}", deny_toml_path.display())])?;
    if deny.licenses.unlicensed != "deny" || deny.licenses.copyleft != "deny" {
        return Err(vec![
            "deny.toml must set unlicensed = \"deny\" and copyleft = \"deny\"".to_string(),
        ]);
    }
    let allow: BTreeSet<String> = deny.licenses.allow.into_iter().collect();

    let meta_json = match metadata_json_override {
        Some(raw) => raw.to_string(),
        None => {
            let out = Command::new("cargo")
                .args(["metadata", "--format-version", "1", "--locked"])
                .current_dir(engine_dir)
                .output()
                .map_err(|e| vec![format!("Failed launching cargo metadata: {e}")])?;
            if !out.status.success() {
                return Err(vec![format!(
                    "cargo metadata failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                )]);
            }
            String::from_utf8_lossy(&out.stdout).to_string()
        }
    };

    let meta: CargoMetadata = serde_json::from_str(&meta_json)
        .map_err(|e| vec![format!("Failed parsing cargo metadata JSON: {e}")])?;
    let workspace_set: BTreeSet<&str> = meta.workspace_members.iter().map(String::as_str).collect();
    let mut violations = Vec::new();
    let mut checked = 0usize;

    for pkg in &meta.packages {
        if workspace_set.contains(pkg.id.as_str()) {
            continue;
        }
        checked += 1;
        match &pkg.license {
            Some(expr) => match spdx_satisfied_by_allowlist(expr, &allow) {
                Ok(true) => {}
                Ok(false) => violations.push(format!(
                    "Crate {}@{} has disallowed license '{}'",
                    pkg.name, pkg.version, expr
                )),
                Err(parse_err) => violations.push(format!(
                    "Crate {}@{} malformed SPDX '{}': {}",
                    pkg.name, pkg.version, expr, parse_err
                )),
            },
            None => violations.push(format!(
                "Crate {}@{} is missing SPDX 'license' field (license_file={:?})",
                pkg.name, pkg.version, pkg.license_file
            )),
        }
    }

    if violations.is_empty() {
        Ok(checked)
    } else {
        Err(violations)
    }
}

/// Verifies Cargo Spine + Cartridge DAG isolation and ensures core simulation crates have
/// zero dependencies or symbol references touching `steam-adapter` or Steamworks (`ENG-LIC-002`).
pub fn check_isolation(
    engine_dir: &Path,
    metadata_no_deps_override: Option<&str>,
) -> Result<(), Vec<String>> {
    let meta_json = match metadata_no_deps_override {
        Some(raw) => raw.to_string(),
        None => {
            let out = Command::new("cargo")
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .current_dir(engine_dir)
                .output()
                .map_err(|e| vec![format!("Failed running cargo metadata --no-deps: {e}")])?;
            if !out.status.success() {
                return Err(vec![format!(
                    "cargo metadata --no-deps failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                )]);
            }
            String::from_utf8_lossy(&out.stdout).to_string()
        }
    };

    let meta: CargoMetadata = serde_json::from_str(&meta_json)
        .map_err(|e| vec![format!("Failed parsing cargo metadata JSON: {e}")])?;
    let ws_names: BTreeSet<&str> = meta.packages.iter().map(|p| p.name.as_str()).collect();
    let composition_roots: BTreeSet<&str> = ["sim-cli", "sim-scenario", "desktop-host", "xtask"]
        .into_iter()
        .collect();

    let mut errors = Vec::new();
    for pkg in &meta.packages {
        let name = pkg.name.as_str();
        let normal_ws_deps: Vec<&str> = pkg
            .dependencies
            .iter()
            .filter(|d| d.kind.is_none() && (ws_names.contains(d.name.as_str()) || d.name.contains("steam")))
            .map(|d| d.name.as_str())
            .collect();

        if name == "sim-spine" && !normal_ws_deps.is_empty() {
            errors.push(format!(
                "Spine crate 'sim-spine' must have zero workspace deps, found: {:?}",
                normal_ws_deps
            ));
        } else if !composition_roots.contains(name) && name != "steam-adapter" {
            for dep in &normal_ws_deps {
                if *dep != "sim-spine" && *dep != "sim-data" {
                    errors.push(format!(
                        "Cartridge '{}' violates Spine isolation by depending on '{}'",
                        name, dep
                    ));
                }
            }
        }

        if [
            "sim-spine",
            "sim-data",
            "sim-clock",
            "sim-planet",
            "sim-weather",
            "sim-economy",
            "sim-logistics",
            "sim-pop",
            "sim-command",
            "sim-core",
            "player-view",
            "sim-scenario",
            "sim-cli",
        ]
        .contains(&name)
        {
            for dep in &pkg.dependencies {
                if dep.name.contains("steam") {
                    errors.push(format!(
                        "Core crate '{}' must never depend on Steamworks crate '{}'",
                        name, dep.name
                    ));
                }
            }
        }
    }

    // Source scan guarding against accidental Steamworks symbol leakage inside core simulation crates.
    for core_crate in [
        "sim-spine",
        "sim-data",
        "sim-clock",
        "sim-planet",
        "sim-weather",
        "sim-economy",
        "sim-logistics",
        "sim-pop",
        "sim-command",
        "sim-core",
        "player-view",
        "sim-scenario",
    ] {
        let src_dir = engine_dir.join("crates").join(core_crate).join("src");
        if src_dir.exists() {
            let mut files = Vec::new();
            collect_files_by_ext(&src_dir, &["rs"], &mut files);
            for f in files {
                if let Ok(text) = fs::read_to_string(&f)
                    && text.to_ascii_lowercase().contains("steamworks") {
                        errors.push(format!(
                            "Forbidden 'steamworks' symbol reference in core file {}",
                            f.display()
                        ));
                    }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates AI asset provenance manifests (`ENG-LIC-003`) and writes `steam_ai_disclosure.json`.
pub fn verify_assets(
    repo_root: &Path,
    provenance_manifest_path: &Path,
    out_disclosure_path: &Path,
) -> Result<usize, Vec<String>> {
    let raw = fs::read_to_string(provenance_manifest_path)
        .map_err(|e| vec![format!("Failed reading {}: {e}", provenance_manifest_path.display())])?;
    let manifest: AssetProvenanceManifest = toml::from_str(&raw)
        .map_err(|e| vec![format!("Invalid {}: {e}", provenance_manifest_path.display())])?;

    let mut errors = Vec::new();
    let mut ai_by_path: BTreeMap<String, AssetProvenanceRecord> = BTreeMap::new();
    let valid_categories: BTreeSet<&str> = [
        "VisualTexture",
        "Portrait",
        "Icon",
        "AudioFoley",
        "MusicStem",
    ]
    .into_iter()
    .collect();

    for rec in manifest.assets {
        if rec.generator_model.trim().is_empty() {
            errors.push(format!("{}: generator_model is empty", rec.asset_rel_path));
        }
        if rec.generator_version.trim().is_empty() {
            errors.push(format!("{}: generator_version is empty", rec.asset_rel_path));
        }
        if rec.license_tier.trim().is_empty() {
            errors.push(format!("{}: license_tier is empty", rec.asset_rel_path));
        }
        if !rec.commercial_rights_verified {
            errors.push(format!(
                "{}: commercial_rights_verified must be true for distribution",
                rec.asset_rel_path
            ));
        }
        if rec.human_post_processing.trim().is_empty() {
            errors.push(format!("{}: human_post_processing is empty", rec.asset_rel_path));
        }
        if !valid_categories.contains(rec.steam_disclosure_category.as_str()) {
            errors.push(format!(
                "{}: invalid steam_disclosure_category '{}'",
                rec.asset_rel_path, rec.steam_disclosure_category
            ));
        }
        let disk_path = repo_root.join(&rec.asset_rel_path);
        if !disk_path.is_file() {
            errors.push(format!(
                "Manifest references missing asset file {}",
                rec.asset_rel_path
            ));
        }
        ai_by_path.insert(rec.asset_rel_path.clone(), rec);
    }

    // Collect explicit non-AI open-licensed baseline assets under skins/*/licenses/baseline_assets.toml
    let mut open_licensed_rel_paths: BTreeSet<String> = BTreeSet::new();
    let allowed_open_spdx: BTreeSet<&str> = ["OFL-1.1", "Apache-2.0", "MIT", "CC0-1.0"]
        .into_iter()
        .collect();
    let skins_dir = repo_root.join("skins");
    if skins_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&skins_dir) {
            for entry in entries.flatten() {
                let skin_path = entry.path();
                if !skin_path.is_dir() {
                    continue;
                }
                let baseline_file = skin_path.join("licenses").join("baseline_assets.toml");
                if baseline_file.is_file() {
                    let b_raw = fs::read_to_string(&baseline_file).map_err(|e| {
                        vec![format!("Failed reading {}: {e}", baseline_file.display())]
                    })?;
                    let parsed: BaselineAssetsManifest = toml::from_str(&b_raw).map_err(|e| {
                        vec![format!("Invalid {}: {e}", baseline_file.display())]
                    })?;
                    for item in parsed.fonts.into_iter().chain(parsed.baseline_assets) {
                        if !allowed_open_spdx.contains(item.spdx_license.as_str()) {
                            errors.push(format!(
                                "{}: disallowed open-asset license '{}'",
                                item.rel_path, item.spdx_license
                            ));
                        }
                        if !skin_path.join(&item.license_file).is_file() {
                            errors.push(format!(
                                "{}: missing license file {}",
                                item.rel_path, item.license_file
                            ));
                        }
                        let full = skin_path.join(&item.rel_path);
                        if let Ok(rel) = full.strip_prefix(repo_root) {
                            open_licensed_rel_paths.insert(rel.to_string_lossy().replace('\\', "/"));
                        }
                    }
                }
            }
        }

    // Check every visual/audio/font binary shipped under skins/ and mods/
    let asset_exts = [
        "png", "jpg", "jpeg", "webp", "ktx2", "ogg", "wav", "mp3", "flac", "ttf", "woff2",
    ];
    for dir_name in ["skins", "mods"] {
        let dir_path = repo_root.join(dir_name);
        if !dir_path.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        collect_files_by_ext(&dir_path, &asset_exts, &mut files);
        for file in files {
            let rel = file
                .strip_prefix(repo_root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            if !ai_by_path.contains_key(&rel) && !open_licensed_rel_paths.contains(&rel) {
                errors.push(format!(
                    "Shipped binary asset '{rel}' has neither AI commercial provenance nor non-AI open license entry"
                ));
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut categories = BTreeMap::new();
    let mut models = BTreeSet::new();
    let mut records = Vec::new();
    for rec in ai_by_path.into_values() {
        *categories
            .entry(rec.steam_disclosure_category.clone())
            .or_insert(0usize) += 1;
        models.insert(format!("{}@{}", rec.generator_model, rec.generator_version));
        records.push(rec);
    }

    let report = SteamAiDisclosureReport {
        schema_version: 1,
        total_verified_assets: records.len(),
        categories,
        generator_models: models.into_iter().collect(),
        assets: records,
    };

    if let Some(parent) = out_disclosure_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| vec![format!("Failed creating {}: {e}", parent.display())])?;
    }
    let mut json = serde_json::to_string_pretty(&report)
        .map_err(|e| vec![format!("Failed serializing disclosure report: {e}")])?;
    json.push('\n');
    fs::write(out_disclosure_path, json.as_bytes())
        .map_err(|e| vec![format!("Failed writing {}: {e}", out_disclosure_path.display())])?;
    Ok(report.total_verified_assets)
}

/// Aggregates licenses and license text for 100% of external crates in `cargo metadata` (`ENG-LIC-004`).
pub fn bundle_notices(
    engine_dir: &Path,
    out_notices_path: &Path,
    metadata_json_override: Option<&str>,
) -> Result<usize, Vec<String>> {
    let meta_json = match metadata_json_override {
        Some(raw) => raw.to_string(),
        None => {
            let out = Command::new("cargo")
                .args(["metadata", "--format-version", "1", "--locked"])
                .current_dir(engine_dir)
                .output()
                .map_err(|e| vec![format!("Failed launching cargo metadata: {e}")])?;
            if !out.status.success() {
                return Err(vec![format!(
                    "cargo metadata failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                )]);
            }
            String::from_utf8_lossy(&out.stdout).to_string()
        }
    };

    let meta: CargoMetadata = serde_json::from_str(&meta_json)
        .map_err(|e| vec![format!("Failed parsing cargo metadata: {e}")])?;
    let ws: BTreeSet<&str> = meta.workspace_members.iter().map(String::as_str).collect();

    let mut ext: Vec<&CargoPackage> = meta
        .packages
        .iter()
        .filter(|p| !ws.contains(p.id.as_str()))
        .collect();
    ext.sort_by(|a, b| (a.name.as_str(), a.version.as_str()).cmp(&(b.name.as_str(), b.version.as_str())));

    let mut buf = String::new();
    buf.push_str("THIRD-PARTY SOFTWARE NOTICES AND INFORMATION\n");
    buf.push_str("============================================\n\n");

    for pkg in &ext {
        let spdx = pkg.license.as_deref().unwrap_or("UNSPECIFIED");
        buf.push_str(&format!("--- {} v{} [{}] ---\n", pkg.name, pkg.version, spdx));
        if !pkg.authors.is_empty() {
            buf.push_str(&format!("Authors: {}\n", pkg.authors.join(", ")));
        }
        if let Some(repo) = &pkg.repository {
            buf.push_str(&format!("Repository: {repo}\n"));
        }

        let crate_dir = Path::new(&pkg.manifest_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let mut license_texts = Vec::new();
        if let Ok(rd) = fs::read_dir(crate_dir) {
            let mut candidate_names = BTreeSet::new();
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file()
                    && let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                        let up = fname.to_ascii_uppercase();
                        if up.starts_with("LICENSE") || up.starts_with("LICENCE") || up.starts_with("COPYING") {
                            candidate_names.insert(p);
                        }
                    }
            }
            for lp in candidate_names {
                if let Ok(text) = fs::read_to_string(&lp) {
                    license_texts.push(format!("[File: {}]\n{}", lp.file_name().unwrap().to_string_lossy(), text.trim()));
                }
            }
        }
        if let Some(lf) = &pkg.license_file {
            let explicit = crate_dir.join(lf);
            if explicit.is_file()
                && let Ok(text) = fs::read_to_string(&explicit) {
                    license_texts.push(format!("[Declared license-file: {lf}]\n{}", text.trim()));
                }
        }

        if license_texts.is_empty() {
            buf.push_str(&format!("SPDX License Notice: {}\n\n", spdx));
        } else {
            for lt in license_texts {
                buf.push_str(&lt);
                buf.push_str("\n\n");
            }
        }
    }

    if let Some(parent) = out_notices_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| vec![format!("Failed creating {}: {e}", parent.display())])?;
    }
    fs::write(out_notices_path, buf.as_bytes())
        .map_err(|e| vec![format!("Failed writing {}: {e}", out_notices_path.display())])?;

    Ok(ext.len())
}

/// Parses milestone integer tag (`M0` -> `0`, `M7` -> `7`).
pub fn parse_milestone_num(tag: &str) -> Result<u8, String> {
    let trimmed = tag.trim();
    if let Some(num_str) = trimmed.strip_prefix('M') {
        num_str
            .parse::<u8>()
            .map_err(|_| format!("Invalid milestone tag '{tag}'"))
    } else {
        Err(format!("Milestone tag must start with 'M', got '{tag}'"))
    }
}

/// Parses every 7-field requirement block in a specification markdown file.
pub fn parse_requirements_markdown(
    content: &str,
    source_name: &str,
) -> Result<Vec<ParsedRequirement>, Vec<String>> {
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    let mut current_id: Option<String> = None;
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut current_line = 0usize;

    let flush_block = |heading_id: Option<String>,
                       heading_line: usize,
                       fields: &mut BTreeMap<String, String>,
                       parsed: &mut Vec<ParsedRequirement>,
                       errors: &mut Vec<String>| {
        let Some(hid) = heading_id else { return };
        let req_keys = [
            "ID",
            "Status",
            "Milestone",
            "Verification Layer",
            "Lineage",
            "Requirement",
            "Acceptance Criteria",
        ];
        for key in req_keys {
            if !fields.contains_key(key) {
                errors.push(format!(
                    "{source_name}:{heading_line}: Requirement '{hid}' is missing required field '- **{key}:**'"
                ));
            }
        }
        if let (Some(id), Some(status_s), Some(ms_s), Some(vl), Some(lin)) = (
            fields.get("ID"),
            fields.get("Status"),
            fields.get("Milestone"),
            fields.get("Verification Layer"),
            fields.get("Lineage"),
        ) {
            if id != &hid {
                errors.push(format!(
                    "{source_name}:{heading_line}: Heading ID '{hid}' does not match '- **ID:** `{id}`'"
                ));
            }
            let status = match status_s.as_str() {
                "Active" => Some(ReqStatus::Active),
                "Draft" => Some(ReqStatus::Draft),
                "Superseded" => Some(ReqStatus::Superseded),
                "Removed" => Some(ReqStatus::Removed),
                other => {
                    errors.push(format!(
                        "{source_name}:{heading_line}: Unknown Status '{other}' on '{id}'"
                    ));
                    None
                }
            };
            let ms = match parse_milestone_num(ms_s) {
                Ok(n) => Some(n),
                Err(e) => {
                    errors.push(format!("{source_name}:{heading_line}: {e}"));
                    None
                }
            };
            if let (Some(status), Some(milestone)) = (status, ms) {
                parsed.push(ParsedRequirement {
                    id: id.clone(),
                    status,
                    milestone,
                    verification_layer: vl.clone(),
                    lineage: lin.clone(),
                });
            }
        }
        fields.clear();
    };

    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        if let Some(rest) = line.strip_prefix("### ")
            && let Some(colon) = rest.find(':') {
                let candidate = rest[..colon].trim();
                if candidate.starts_with("ENG-") || candidate.starts_with("UI-") {
                    flush_block(
                        current_id.take(),
                        current_line,
                        &mut fields,
                        &mut parsed,
                        &mut errors,
                    );
                    current_id = Some(candidate.to_string());
                    current_line = line_no;
                    continue;
                }
            }

        if current_id.is_some()
            && let Some(after) = line.strip_prefix("- **")
                && let Some(sep) = after.find(":**") {
                    let key = after[..sep].trim().to_string();
                    let raw_val = after[sep + 3..].trim();
                    let cleaned = raw_val
                        .strip_prefix('`')
                        .and_then(|s| s.strip_suffix('`'))
                        .unwrap_or(raw_val)
                        .to_string();
                    fields.insert(key, cleaned);
                }
    }
    flush_block(
        current_id.take(),
        current_line,
        &mut fields,
        &mut parsed,
        &mut errors,
    );

    if errors.is_empty() {
        Ok(parsed)
    } else {
        Err(errors)
    }
}

fn collect_files_by_ext(root: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for p in children {
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str())
                    && ["target", "node_modules", "dist", "dist-electron", "release", ".git"]
                        .contains(&name)
                    {
                        continue;
                    }
                stack.push(p);
            } else if p.is_file()
                && let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let lower = ext.to_ascii_lowercase();
                    if exts.contains(&lower.as_str()) {
                        out.push(p);
                    }
                }
        }
    }
}

/// Scans Rust and TypeScript test source trees for `// @req <ID>` annotations.
pub fn scan_test_citations(search_roots: &[&Path]) -> Vec<TestCitation> {
    let mut files = Vec::new();
    for root in search_roots {
        if root.exists() {
            collect_files_by_ext(root, &["rs", "ts", "tsx", "mjs"], &mut files);
        }
    }
    files.sort();
    let mut citations = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("// @req ")
                && let Some(token) = rest.split_whitespace().next() {
                    citations.push(TestCitation {
                        req_id: token.trim().to_string(),
                        file_path: file.clone(),
                        line_number: idx + 1,
                    });
                }
        }
    }
    citations
}

/// Validates requirement markdown integrity and automated `@req` test traceability (`ENG-SCN-005`).
pub fn check_requirements_traceability(
    req_docs: &[(&str, &str)],
    search_roots: &[&Path],
    up_to_milestone: u8,
) -> Result<(usize, usize), Vec<String>> {
    let mut errors = Vec::new();
    let mut by_id: BTreeMap<String, ParsedRequirement> = BTreeMap::new();

    for (doc_name, doc_content) in req_docs {
        match parse_requirements_markdown(doc_content, doc_name) {
            Ok(items) => {
                for item in items {
                    if by_id.contains_key(&item.id) {
                        errors.push(format!("Duplicate requirement ID '{}'", item.id));
                    } else {
                        by_id.insert(item.id.clone(), item);
                    }
                }
            }
            Err(mut errs) => errors.append(&mut errs),
        }
    }

    let citations = scan_test_citations(search_roots);
    let mut cited_counts: BTreeMap<String, usize> = BTreeMap::new();
    for cit in &citations {
        match by_id.get(&cit.req_id) {
            None => errors.push(format!(
                "{}:{}: Orphan test citation '@req {}' references undefined requirement ID",
                cit.file_path.display(),
                cit.line_number,
                cit.req_id
            )),
            Some(req) if req.status == ReqStatus::Removed || req.status == ReqStatus::Superseded => {
                errors.push(format!(
                    "{}:{}: Stale test citation '@req {}' targets {:?} requirement",
                    cit.file_path.display(),
                    cit.line_number,
                    cit.req_id,
                    req.status
                ))
            }
            Some(_) => {
                *cited_counts.entry(cit.req_id.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut active_checked = 0usize;
    for req in by_id.values() {
        if req.status == ReqStatus::Active && req.milestone <= up_to_milestone {
            active_checked += 1;
            if !cited_counts.contains_key(&req.id) {
                errors.push(format!(
                    "Uncovered Active M{} requirement '{}' has zero bound '@req' tests",
                    req.milestone, req.id
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok((active_checked, citations.len()))
    } else {
        Err(errors)
    }
}
