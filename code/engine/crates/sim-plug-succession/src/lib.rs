//! M7 Plug Cartridge: Mortal Leader & Character Lifecycle, Physical-Archive-Only Succession,
//! and Unrecorded Institutional Rapport Erasure.

#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::codec::{
    read_f32_bits, read_str, read_u8, read_u32_le, read_u64_le, write_f32_bits, write_str,
    write_u8, write_u32_le, write_u64_le,
};
use sim_spine::ids::{CellId, CharacterId, SettlementId, Tick};
use sim_spine::plugs::{SuccessionCharacterView, SuccessionPlugPort, SuccessionTransitionView};
use sim_spine::ports::{CanonicalSaveablePort, ModRegistryPort};

/// Concrete M7 succession plugin tracking character aging, mortality, and physical HQ archives.
#[derive(Clone, Debug, Default)]
pub struct SuccessionPlugin {
    active_rule_key: String,
    active_rule_display: String,
    continuity_fraction: f32,
    transition_disruption_hours: u32,
    hours_per_year: f32,
    roster: Vec<SuccessionCharacterView>,
    hq_physical_archives: Vec<String>,
    unrecorded_personal_rapport: BTreeMap<CharacterId, f32>,
    pending_transition: Option<SuccessionTransitionView>,
    disruption_until_tick: Tick,
    last_tick: Tick,
}

impl SuccessionPlugin {
    /// Initializes character mortal lifecycles and active succession protocol strictly from `ModRegistryPort`.
    #[must_use]
    pub fn from_registry(
        registry: &dyn ModRegistryPort,
        settlement_by_key: &BTreeMap<String, SettlementId>,
        hq_settlement: SettlementId,
    ) -> Self {
        let cal = registry.calendar_config();
        let hours_per_year =
            (f32::from(cal.hours_per_day.max(1) as u16) * f32::from(cal.days_per_year.max(1) as u16)).max(24.0);
        let rule = registry.succession_rules().first();
        let (rule_key, rule_display, cont, disrupt_h) = match rule {
            Some(r) => (
                r.key.clone(),
                r.display_name.clone(),
                r.continuity_fraction,
                r.transition_disruption_hours,
            ),
            None => (
                "default_successor".to_string(),
                "Deputy Succession".to_string(),
                0.85,
                72,
            ),
        };

        let mut roster = Vec::new();
        let mut rapport = BTreeMap::new();
        for (idx, ch) in registry.scenario_init().characters.iter().enumerate() {
            let cid = CharacterId((idx + 1) as u32);
            let sid = settlement_by_key
                .get(&ch.home_settlement_key)
                .copied()
                .unwrap_or(hq_settlement);
            let age = 32.0 + (idx as f32) * 7.5;
            roster.push(SuccessionCharacterView {
                character_id: cid,
                name: ch.display_name.clone(),
                role_title: ch.role_title.clone(),
                age_years: age,
                health_01: 0.94 - (idx as f32) * 0.04,
                is_alive: true,
                is_ruler: idx == 0,
                settlement: sid,
            });
            rapport.insert(cid, 0.78);
        }
        if roster.is_empty() {
            roster.push(SuccessionCharacterView {
                character_id: CharacterId(1),
                name: "Expedition Commander".to_string(),
                role_title: "Commander".to_string(),
                age_years: 38.0,
                health_01: 0.95,
                is_alive: true,
                is_ruler: true,
                settlement: hq_settlement,
            });
            rapport.insert(CharacterId(1), 0.75);
        }

        Self {
            active_rule_key: rule_key,
            active_rule_display: rule_display,
            continuity_fraction: cont,
            transition_disruption_hours: disrupt_h,
            hours_per_year,
            roster,
            hq_physical_archives: vec!["Founding Colony Survey Charter".to_string()],
            unrecorded_personal_rapport: rapport,
            pending_transition: None,
            disruption_until_tick: Tick(0),
            last_tick: Tick(0),
        }
    }

    /// Selects a succession rule defined in external mods (`ENG-CMD-007`).
    pub fn set_succession_rule(
        &mut self,
        rule_key: &str,
        registry: &dyn ModRegistryPort,
    ) -> Result<(), String> {
        let rule = registry
            .succession_rules()
            .iter()
            .find(|r| r.key == rule_key)
            .ok_or_else(|| format!("Unknown succession rule '{rule_key}'"))?;
        self.active_rule_key = rule.key.clone();
        self.active_rule_display = rule.display_name.clone();
        self.continuity_fraction = rule.continuity_fraction;
        self.transition_disruption_hours = rule.transition_disruption_hours;
        Ok(())
    }

    /// Appends a verified physical courier log or decree into HQ's durable archive (`ENG-CMD-007.1`).
    pub fn record_physical_hq_archive(&mut self, entry: String) {
        if !self.hq_physical_archives.contains(&entry) {
            self.hq_physical_archives.push(entry);
        }
    }

    /// Returns count of surviving physical archive entries.
    #[must_use]
    pub fn archive_entry_count(&self) -> usize {
        self.hq_physical_archives.len()
    }

    /// Returns remaining unrecorded personal rapport for `who` (`0.0` immediately after succession).
    #[must_use]
    pub fn personal_rapport(&self, who: CharacterId) -> f32 {
        self.unrecorded_personal_rapport.get(&who).copied().unwrap_or(0.0)
    }

    /// Sets character age or triggers mortality directly (used by aging ticks and scenario trauma).
    pub fn trigger_ruler_demise(&mut self, tick: Tick) -> bool {
        if let Some(ruler) = self.roster.iter_mut().find(|c| c.is_ruler && c.is_alive) {
            ruler.health_01 = 0.0;
            ruler.is_alive = false;
        }
        self.resolve_succession_if_needed(tick)
    }

    /// Advances character ages/health for `dt_hours` hours and resolves ruler succession upon demise (`ENG-POP-006`).
    pub fn step_aging(&mut self, tick: Tick, dt_hours: u32) -> bool {
        self.last_tick = tick;
        let dy = (dt_hours as f32) / self.hours_per_year.max(1.0);
        for c in &mut self.roster {
            if !c.is_alive {
                continue;
            }
            c.age_years += dy;
            if c.age_years >= 62.0 {
                let excess = c.age_years - 62.0;
                c.health_01 = (c.health_01 - dy * (0.04 + 0.012 * excess)).max(0.0);
                if c.health_01 <= 0.02 {
                    c.health_01 = 0.0;
                    c.is_alive = false;
                }
            }
        }
        self.resolve_succession_if_needed(tick)
    }

    /// Player acknowledges succession modal transition (`ENG-CMD-007.2`).
    pub fn acknowledge_succession(&mut self) {
        if let Some(tr) = &mut self.pending_transition {
            tr.awaiting_player_acknowledgement = false;
        }
    }

    fn resolve_succession_if_needed(&mut self, tick: Tick) -> bool {
        let deceased_idx = self.roster.iter().position(|c| c.is_ruler && !c.is_alive);
        let Some(d_idx) = deceased_idx else {
            return false;
        };
        let deceased_name = self.roster[d_idx].name.clone();
        self.roster[d_idx].is_ruler = false;

        let lost_rapport = self.unrecorded_personal_rapport.len();
        // Unrecorded informal loyalty/rapport vanishes completely with the deceased leader (`ENG-CMD-007.1`).
        self.unrecorded_personal_rapport.clear();

        let successor_idx = self
            .roster
            .iter()
            .position(|c| c.is_alive)
            .unwrap_or_else(|| {
                let next_id = CharacterId((self.roster.len() + 1) as u32);
                self.roster.push(SuccessionCharacterView {
                    character_id: next_id,
                    name: format!("Provisional Regent #{}", next_id.0),
                    role_title: "Acting Governor".to_string(),
                    age_years: 34.0,
                    health_01: 0.95,
                    is_alive: true,
                    is_ruler: false,
                    settlement: SettlementId(1),
                });
                self.roster.len() - 1
            });

        self.roster[successor_idx].is_ruler = true;
        let successor_name = self.roster[successor_idx].name.clone();
        let successor_id = self.roster[successor_idx].character_id;
        self.unrecorded_personal_rapport.insert(successor_id, 0.25);

        self.disruption_until_tick = Tick(
            tick.0
                .saturating_add(u64::from(self.transition_disruption_hours)),
        );
        self.pending_transition = Some(SuccessionTransitionView {
            rule_key: self.active_rule_key.clone(),
            rule_display_name: self.active_rule_display.clone(),
            deceased_ruler_name: deceased_name,
            successor_name,
            transition_tick: tick,
            disruption_until_tick: self.disruption_until_tick,
            inherited_archive_entries: self.hq_physical_archives.len(),
            lost_unrecorded_rapport_count: lost_rapport,
            awaiting_player_acknowledgement: true,
        });
        true
    }
}

impl SuccessionPlugPort for SuccessionPlugin {
    fn check_succession_transition(&mut self, tick: Tick, _hq_cell: CellId) -> bool {
        self.step_aging(tick, 1)
    }

    fn roster_characters(&self) -> &[SuccessionCharacterView] {
        &self.roster
    }

    fn pending_transition(&self) -> Option<&SuccessionTransitionView> {
        self.pending_transition.as_ref()
    }

    fn effective_command_multiplier(&self, tick: Tick) -> f32 {
        if tick.0 < self.disruption_until_tick.0 {
            self.continuity_fraction.clamp(0.45, 0.98)
        } else {
            1.0
        }
    }
}

impl CanonicalSaveablePort for SuccessionPlugin {
    fn cartridge_tag(&self) -> &'static str {
        "sim-plug-succession"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_str(out, &self.active_rule_key);
        write_str(out, &self.active_rule_display);
        write_f32_bits(out, self.continuity_fraction);
        write_u32_le(out, self.transition_disruption_hours);
        write_f32_bits(out, self.hours_per_year);
        write_u64_le(out, self.disruption_until_tick.0);
        write_u64_le(out, self.last_tick.0);

        write_u32_le(out, self.roster.len() as u32);
        for c in &self.roster {
            write_u32_le(out, c.character_id.0);
            write_str(out, &c.name);
            write_str(out, &c.role_title);
            write_f32_bits(out, c.age_years);
            write_f32_bits(out, c.health_01);
            write_u8(out, u8::from(c.is_alive));
            write_u8(out, u8::from(c.is_ruler));
            write_u32_le(out, c.settlement.0);
        }

        write_u32_le(out, self.hq_physical_archives.len() as u32);
        for entry in &self.hq_physical_archives {
            write_str(out, entry);
        }

        write_u32_le(out, self.unrecorded_personal_rapport.len() as u32);
        for (cid, &r) in &self.unrecorded_personal_rapport {
            write_u32_le(out, cid.0);
            write_f32_bits(out, r);
        }

        write_u8(out, u8::from(self.pending_transition.is_some()));
        if let Some(tr) = &self.pending_transition {
            write_str(out, &tr.rule_key);
            write_str(out, &tr.rule_display_name);
            write_str(out, &tr.deceased_ruler_name);
            write_str(out, &tr.successor_name);
            write_u64_le(out, tr.transition_tick.0);
            write_u64_le(out, tr.disruption_until_tick.0);
            write_u32_le(out, tr.inherited_archive_entries as u32);
            write_u32_le(out, tr.lost_unrecorded_rapport_count as u32);
            write_u8(out, u8::from(tr.awaiting_player_acknowledgement));
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.active_rule_key = read_str(input)?;
        self.active_rule_display = read_str(input)?;
        self.continuity_fraction = read_f32_bits(input)?;
        self.transition_disruption_hours = read_u32_le(input)?;
        self.hours_per_year = read_f32_bits(input)?;
        self.disruption_until_tick = Tick(read_u64_le(input)?);
        self.last_tick = Tick(read_u64_le(input)?);

        let rlen = read_u32_le(input)? as usize;
        self.roster.clear();
        for _ in 0..rlen {
            self.roster.push(SuccessionCharacterView {
                character_id: CharacterId(read_u32_le(input)?),
                name: read_str(input)?,
                role_title: read_str(input)?,
                age_years: read_f32_bits(input)?,
                health_01: read_f32_bits(input)?,
                is_alive: read_u8(input)? != 0,
                is_ruler: read_u8(input)? != 0,
                settlement: SettlementId(read_u32_le(input)?),
            });
        }

        let alen = read_u32_le(input)? as usize;
        self.hq_physical_archives.clear();
        for _ in 0..alen {
            self.hq_physical_archives.push(read_str(input)?);
        }

        let ulen = read_u32_le(input)? as usize;
        self.unrecorded_personal_rapport.clear();
        for _ in 0..ulen {
            self.unrecorded_personal_rapport
                .insert(CharacterId(read_u32_le(input)?), read_f32_bits(input)?);
        }

        let has_tr = read_u8(input)? != 0;
        self.pending_transition = if has_tr {
            Some(SuccessionTransitionView {
                rule_key: read_str(input)?,
                rule_display_name: read_str(input)?,
                deceased_ruler_name: read_str(input)?,
                successor_name: read_str(input)?,
                transition_tick: Tick(read_u64_le(input)?),
                disruption_until_tick: Tick(read_u64_le(input)?),
                inherited_archive_entries: read_u32_le(input)? as usize,
                lost_unrecorded_rapport_count: read_u32_le(input)? as usize,
                awaiting_player_acknowledgement: read_u8(input)? != 0,
            })
        } else {
            None
        };
        Ok(())
    }
}
