//! Strict Epistemic Air-Gap & IPC DTO Boundary (`player-view`).
//! The **only** simulation crate allowed to emit IPC JSON payloads to `code/ui`.
#![deny(clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sim_spine::{
    codec::{
        read_f32_bits, read_str, read_u32_le, read_u64_le, read_u8, write_f32_bits, write_str,
        write_u32_le, write_u64_le, write_u8,
    },
    CanonicalSaveablePort, CellId, ModRegistryPort, PhysicalLedgerPort, PlanetGeometryPort,
    PopulationPort, SurfaceConditionsPort, Tick,
};

/// IPC-safe mod diagnostic payload compatible with `UI-SHL-004`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDiagnosticDto {
    pub severity: String,
    pub layer: String,
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub field_path: String,
    pub message: String,
}

/// Top-level error screen DTO sent when mod validation blocks world launch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModValidationFailureState {
    pub ok: bool,
    pub diagnostics: Vec<ModDiagnosticDto>,
}

/// Knowledge state of a planetary cell inside `DiegeticSnapshot`.
/// Note: `Unexplored` intentionally carries zero terrain, strata, mineral, or weather fields (`ENG-EPI-001`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tier", rename_all = "camelCase")]
pub enum KnownCellDto {
    Unexplored {
        #[serde(rename = "cellId")]
        cell_id: u32,
    },
    Surveyed {
        #[serde(rename = "cellId")]
        cell_id: u32,
        #[serde(rename = "observedTick")]
        observed_tick: u64,
        #[serde(rename = "isLiveHqSight")]
        is_live_hq_sight: bool,
        #[serde(rename = "elevationM")]
        elevation_m: f32,
        #[serde(rename = "confidence01")]
        confidence_01: f32,
        #[serde(rename = "provenanceLabel")]
        provenance_label: String,
    },
}

/// Physical HQ presence DTO.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HqPresenceDto {
    pub mode: String,
    pub cell_id: u32,
    pub settlement_name: String,
    pub sight_radius_cells: u8,
}

/// Globe topological metadata for buffer creation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobeTopologyDto {
    pub subdivision: u8,
    pub cell_count: u32,
}

/// Observed hourly weather strictly filtered to live HQ or arrived report cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedWeatherDto {
    pub cell_id: u32,
    pub observed_tick: u64,
    pub precip_mm_hr: f32,
    pub mud_index_01: f32,
    pub temp_c: f32,
    pub visibility_01: f32,
}

/// Reported commodity ledger row with explicit epistemic provenance bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedLedgerRowDto {
    pub settlement_id: u32,
    pub settlement_name: String,
    pub commodity_key: String,
    pub commodity_name: String,
    pub reported_mass_kg: f64,
    pub min_est_kg: f64,
    pub max_est_kg: f64,
    pub observed_tick: u64,
    pub provenance_label: String,
}

/// Subordinate node inside the delegation hierarchy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubordinateSummaryDto {
    pub character_id: u32,
    pub display_name: String,
    pub role_title: String,
    pub competence: f32,
}

/// Authority delegation overview.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationTreeDto {
    pub leader_title: String,
    pub capacity_attention: f32,
    pub used_attention: f32,
    pub subordinates: Vec<SubordinateSummaryDto>,
}

/// Tracked out-of-sight or live convoy marker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedConvoyDto {
    pub convoy_id: u64,
    pub label: String,
    pub status: String,
    pub projected_cell_id: u32,
}

/// Arrived physical dispatch entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiegeticDispatchDto {
    pub dispatch_id: u64,
    pub title: String,
    pub sent_tick: u64,
    pub arrived_tick: u64,
    pub origin_label: String,
}

/// Simulation clock and notification status DTO.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockAndNotificationsDto {
    pub tick: u64,
    pub calendar_text: String,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub active_alert_count: u32,
}

/// Sole authorized IPC snapshot crossing from Rust into Electron renderer (`ENG-EPI-001`, `UI-SHL-002`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiegeticSnapshot {
    pub schema_version: u32,
    pub scenario_key: String,
    pub scenario_name: String,
    pub globe_topology: GlobeTopologyDto,
    pub known_cells: Vec<KnownCellDto>,
    pub hq_presence: HqPresenceDto,
    pub tracked_convoys: Vec<TrackedConvoyDto>,
    pub observed_weather: Vec<ObservedWeatherDto>,
    pub ledger_rows: Vec<ReportedLedgerRowDto>,
    pub delegation_tree: DelegationTreeDto,
    pub dispatch_inbox: Vec<DiegeticDispatchDto>,
    pub clock_and_notifications: ClockAndNotificationsDto,
}

/// Validated player command envelope received across IPC from Electron.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValidatedPlayerCommand {
    SetPaused { paused: bool },
    StepSingleTick,
    SetSpeedMultiplier { multiplier: u32 },
    PingBridge { tag: String },
}

/// Typed IPC acknowledgement returned by backend command handlers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAck {
    pub accepted: bool,
    pub tick: u64,
    pub error_code: Option<String>,
    pub message: String,
}

/// Maintains the player's persistent epistemic archive and compiles air-gapped IPC snapshots.
#[derive(Default, Debug)]
pub struct PlayerKnowledgeArchive {
    surveyed_cells: BTreeMap<u32, KnownCellDto>,
}

impl PlayerKnowledgeArchive {
    /// Creates an empty cartographic knowledge archive prior to initial HQ observation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surveyed_cells: BTreeMap::new(),
        }
    }

    /// Formats a tick index cleanly according to scenario calendar rules.
    #[must_use]
    pub fn format_calendar_tick(registry: &dyn ModRegistryPort, tick: Tick) -> String {
        let cal = registry.calendar_config();
        let hours_per_day = u64::from(cal.hours_per_day.max(1));
        let days_per_year = u64::from(cal.days_per_year.max(1));
        let total_days = tick.get() / hours_per_day;
        let hour = tick.get() % hours_per_day;
        let year_offset = (total_days / days_per_year) as i32;
        let mut day_of_year = (total_days % days_per_year) as u32;

        let mut month_name = "Cycle";
        for (name, len) in cal.month_names.iter().zip(cal.month_lengths.iter()) {
            if day_of_year < *len {
                month_name = name.as_str();
                break;
            }
            day_of_year -= *len;
        }
        format!(
            "{} {}, {} {:02} ({:02}:00)",
            cal.year_prefix,
            cal.starting_year + year_offset,
            month_name,
            day_of_year + 1,
            hour
        )
    }

    /// Refreshes the player's local survey archive within the HQ optical sight radius and returns the live visible cell set.
    pub fn observe_hq_halo(
        &mut self,
        tick: Tick,
        planet: &dyn PlanetGeometryPort,
        ledger: &dyn PhysicalLedgerPort,
    ) -> BTreeSet<u32> {
        let hq_cell = ledger.hq_cell();
        let sight_radius = ledger.hq_sight_radius_cells();

        let mut live_cells = BTreeSet::new();
        let mut frontier = vec![(hq_cell, 0u8)];
        live_cells.insert(hq_cell.get());
        while let Some((cid, dist)) = frontier.pop() {
            if dist < sight_radius {
                for &nbr in planet.cell_neighbors(cid) {
                    if live_cells.insert(nbr.get()) {
                        frontier.push((nbr, dist + 1));
                    }
                }
            }
        }

        for (&cid_u32, entry) in &mut self.surveyed_cells {
            if !live_cells.contains(&cid_u32)
                && let KnownCellDto::Surveyed {
                    ref mut is_live_hq_sight,
                    ..
                } = *entry
            {
                *is_live_hq_sight = false;
            }
        }

        for &cid_u32 in &live_cells {
            let elev = planet.cell_elevation_m(CellId(cid_u32));
            self.surveyed_cells.insert(
                cid_u32,
                KnownCellDto::Surveyed {
                    cell_id: cid_u32,
                    observed_tick: tick.get(),
                    is_live_hq_sight: true,
                    elevation_m: elev,
                    confidence_01: 1.0,
                    provenance_label: "Direct Observation (HQ)".to_string(),
                },
            );
        }

        live_cells
    }

    /// Compiles a strictly air-gapped `DiegeticSnapshot` reading exclusively through Spine ports.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_snapshot(
        &mut self,
        tick: Tick,
        paused: bool,
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        ledger: &dyn PhysicalLedgerPort,
        population: &dyn PopulationPort,
    ) -> DiegeticSnapshot {
        let scenario = registry.scenario_init();
        let hq_cell = ledger.hq_cell();
        let sight_radius = ledger.hq_sight_radius_cells();
        let total_cells = planet.cell_count();

        let live_cells = self.observe_hq_halo(tick, planet, ledger);

        // Strict air-gap: union of sampled globe cells, live HQ sight halo, and previously surveyed cells.
        // Unvisited cells remain `Unexplored` with zero terrain, weather, or strata fields.
        let mut emitted_cells: BTreeSet<u32> = (0..total_cells.min(64)).collect();
        emitted_cells.extend(&live_cells);
        emitted_cells.extend(self.surveyed_cells.keys().copied());

        let mut known_cells = Vec::with_capacity(emitted_cells.len());
        let mut observed_weather = Vec::new();
        for cid_u32 in emitted_cells {
            if live_cells.contains(&cid_u32) {
                let st = weather.surface(CellId(cid_u32));
                observed_weather.push(ObservedWeatherDto {
                    cell_id: cid_u32,
                    observed_tick: tick.get(),
                    precip_mm_hr: st.precip_mm_hr,
                    mud_index_01: st.mud_index_01,
                    temp_c: st.temp_c,
                    visibility_01: st.visibility_01,
                });
            }
            if let Some(known) = self.surveyed_cells.get(&cid_u32) {
                let mut cell_dto = known.clone();
                if let KnownCellDto::Surveyed {
                    ref mut is_live_hq_sight,
                    ..
                } = cell_dto
                {
                    *is_live_hq_sight = live_cells.contains(&cid_u32);
                }
                known_cells.push(cell_dto);
            } else {
                known_cells.push(KnownCellDto::Unexplored { cell_id: cid_u32 });
            }
        }

        let mut ledger_rows = Vec::new();
        for settlement in ledger.settlements() {
            if !live_cells.contains(&settlement.cell_id.get()) {
                continue;
            }
            for entry in ledger.settlement_inventory_entries(settlement.id) {
                let (c_key, c_name) = registry
                    .commodity_def(entry.commodity)
                    .map(|c| (c.key.clone(), c.display_name.clone()))
                    .unwrap_or_default();
                ledger_rows.push(ReportedLedgerRowDto {
                    settlement_id: settlement.id.get(),
                    settlement_name: settlement.display_name.clone(),
                    commodity_key: c_key,
                    commodity_name: c_name,
                    reported_mass_kg: entry.mass_kg,
                    min_est_kg: entry.mass_kg,
                    max_est_kg: entry.mass_kg,
                    observed_tick: tick.get(),
                    provenance_label: "Direct Observation (HQ)".to_string(),
                });
            }
        }

        let hq_name = ledger
            .hq_settlement()
            .and_then(|sid| ledger.settlements().iter().find(|s| s.id == sid))
            .map(|s| s.display_name.clone())
            .unwrap_or_else(|| scenario.hq_settlement_key.clone());

        let subordinates = population
            .all_characters()
            .iter()
            .map(|ch| SubordinateSummaryDto {
                character_id: ch.id.get(),
                display_name: ch.display_name.clone(),
                role_title: ch.role_title.clone(),
                competence: ch.competence,
            })
            .collect();

        DiegeticSnapshot {
            schema_version: 1,
            scenario_key: scenario.scenario_key.clone(),
            scenario_name: scenario.display_name.clone(),
            globe_topology: GlobeTopologyDto {
                subdivision: scenario.goldberg_subdivision,
                cell_count: total_cells,
            },
            known_cells,
            hq_presence: HqPresenceDto {
                mode: "Stationary".to_string(),
                cell_id: hq_cell.get(),
                settlement_name: hq_name,
                sight_radius_cells: sight_radius,
            },
            tracked_convoys: Vec::new(),
            observed_weather,
            ledger_rows,
            delegation_tree: DelegationTreeDto {
                leader_title: "Colony Commander".to_string(),
                capacity_attention: 100.0,
                used_attention: 0.0,
                subordinates,
            },
            dispatch_inbox: Vec::new(),
            clock_and_notifications: ClockAndNotificationsDto {
                tick: tick.get(),
                calendar_text: Self::format_calendar_tick(registry, tick),
                paused,
                speed_multiplier: 1,
                active_alert_count: 0,
            },
        }
    }
}

impl CanonicalSaveablePort for PlayerKnowledgeArchive {
    fn cartridge_tag(&self) -> &'static str {
        "player-view"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u32_le(out, self.surveyed_cells.len() as u32);
        for (&cid, entry) in &self.surveyed_cells {
            write_u32_le(out, cid);
            match entry {
                KnownCellDto::Unexplored { cell_id } => {
                    write_u8(out, 0);
                    write_u32_le(out, *cell_id);
                }
                KnownCellDto::Surveyed {
                    cell_id,
                    observed_tick,
                    is_live_hq_sight,
                    elevation_m,
                    confidence_01,
                    provenance_label,
                } => {
                    write_u8(out, 1);
                    write_u32_le(out, *cell_id);
                    write_u64_le(out, *observed_tick);
                    write_u8(out, u8::from(*is_live_hq_sight));
                    write_f32_bits(out, *elevation_m);
                    write_f32_bits(out, *confidence_01);
                    write_str(out, provenance_label);
                }
            }
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        let count = read_u32_le(input)? as usize;
        self.surveyed_cells.clear();
        for _ in 0..count {
            let key = read_u32_le(input)?;
            let tag = read_u8(input)?;
            let dto = match tag {
                0 => KnownCellDto::Unexplored {
                    cell_id: read_u32_le(input)?,
                },
                1 => KnownCellDto::Surveyed {
                    cell_id: read_u32_le(input)?,
                    observed_tick: read_u64_le(input)?,
                    is_live_hq_sight: read_u8(input)? != 0,
                    elevation_m: read_f32_bits(input)?,
                    confidence_01: read_f32_bits(input)?,
                    provenance_label: read_str(input)?,
                },
                other => return Err(format!("invalid KnownCellDto tag {other}")),
            };
            self.surveyed_cells.insert(key, dto);
        }
        Ok(())
    }
}
