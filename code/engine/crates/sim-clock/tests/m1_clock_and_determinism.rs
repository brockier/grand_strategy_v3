//! Milestone 1 unit and property verification tests for `sim-clock` (`ENG-CLK-001`..`006`).

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use rand_core::RngCore;
use sim_clock::{
    ClockRunMode, DAILY_PERIOD, SaveCodecError, SimulationClock, StaggeredBucketRegistry,
    TickTelemetryRing, WEEKLY_PERIOD, build_worker_pool, compute_canonical_digest,
    execute_parallel_sorted, load_snapshot_bytes, phase_rng, project_calendar,
    save_snapshot_bytes,
};
use sim_spine::codec::{read_u64_le, write_u64_le};
use sim_spine::{
    AutoPauseSignal, CalendarConfigView, CanonicalSaveablePort, ClockControlCommand, EventClassId,
    PhaseTimingSample, Tick, TickPhase, TickTelemetryPort,
};

fn standard_365_calendar() -> CalendarConfigView {
    CalendarConfigView {
        hours_per_day: 24,
        days_per_year: 365,
        year_prefix: "Year".to_string(),
        starting_year: 1,
        month_names: vec![
            "M01".into(),
            "M02".into(),
            "M03".into(),
            "M04".into(),
            "M05".into(),
            "M06".into(),
            "M07".into(),
            "M08".into(),
            "M09".into(),
            "M10".into(),
            "M11".into(),
            "M12".into(),
        ],
        month_lengths: vec![31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    }
}

// @req ENG-CLK-001
#[test]
fn stepping_8760_ticks_advances_365_day_calendar_by_one_year_with_solar_wraparound() {
    let cal = standard_365_calendar();
    let start = project_calendar(&cal, Tick(0));
    assert_eq!(start.year, 1);
    assert_eq!(start.day_of_year, 0);
    assert_eq!(start.hour_of_day, 0);
    assert_eq!(start.solar_longitude_rad, 0.0);
    assert_eq!(start.diurnal_angle_rad, 0.0);

    let last_hour = project_calendar(&cal, Tick(8_759));
    assert_eq!(last_hour.year, 1);
    assert_eq!(last_hour.day_of_year, 364);
    assert_eq!(last_hour.hour_of_day, 23);
    assert!(last_hour.solar_longitude_rad > TAU * 0.99);
    assert!(last_hour.diurnal_angle_rad > TAU * 0.95);

    let wrapped = project_calendar(&cal, Tick(8_760));
    assert_eq!(wrapped.year, 2);
    assert_eq!(wrapped.day_of_year, 0);
    assert_eq!(wrapped.day_of_month, 1);
    assert_eq!(wrapped.hour_of_day, 0);
    assert_eq!(wrapped.solar_longitude_rad, 0.0);
    assert_eq!(wrapped.diurnal_angle_rad, 0.0);
}

// @req ENG-CLK-002
#[test]
fn clock_state_machine_supports_pause_step_speed_and_exact_autopause() {
    let mut clock = SimulationClock::new(42);
    let mut state_counter = 0u64;

    clock.apply_control_command(ClockControlCommand::Pause);
    let executed_while_paused = clock.advance_frame(100, |_| {
        state_counter += 1;
        None
    });
    assert_eq!(executed_while_paused, 0);
    assert_eq!(state_counter, 0);
    assert_eq!(clock.current_tick, Tick(0));

    clock.apply_control_command(ClockControlCommand::StepSingleHour);
    let executed_step = clock.advance_frame(50, |_| {
        state_counter += 1;
        None
    });
    assert_eq!(executed_step, 1);
    assert_eq!(state_counter, 1);
    assert_eq!(clock.current_tick, Tick(1));
    assert_eq!(clock.mode, ClockRunMode::Paused);

    let trigger_ev = EventClassId(7);
    clock.set_event_auto_pause(trigger_ev, true);
    clock.apply_control_command(ClockControlCommand::SetSpeedMultiplier(10));
    let executed_before_pause = clock.advance_frame(1, |tick| {
        state_counter += 1;
        if tick == Tick(5) {
            Some(AutoPauseSignal {
                reason_event: trigger_ev,
                tick,
            })
        } else {
            None
        }
    });
    assert_eq!(executed_before_pause, 4);
    assert_eq!(state_counter, 5);
    assert_eq!(clock.current_tick, Tick(5));
    assert_eq!(
        clock.mode,
        ClockRunMode::AutoPaused {
            reason_event: trigger_ev,
            tick: Tick(5)
        }
    );

    // Subsequent frame updates emit zero ticks until explicitly resumed.
    assert_eq!(clock.advance_frame(10, |_| None), 0);
    assert_eq!(clock.current_tick, Tick(5));
}

// @req ENG-CLK-003
#[test]
fn staggered_buckets_execute_every_entity_once_per_period_with_bounded_variance() {
    let n = 1_000usize;
    let mut reg = StaggeredBucketRegistry::new();
    for i in 0..n {
        // Deliberately sparse non-sequential entity IDs to stress slot distribution.
        reg.register_entity((i as u64) * 37 + 13);
    }

    let mut daily_hits: BTreeMap<u64, u32> = BTreeMap::new();
    let mut daily_sizes = Vec::new();
    for t in 0..DAILY_PERIOD {
        let mut count_this_tick = 0usize;
        for i in 0..n {
            let eid = (i as u64) * 37 + 13;
            if reg.is_due(eid, Tick(t), DAILY_PERIOD) {
                *daily_hits.entry(eid).or_insert(0) += 1;
                count_this_tick += 1;
            }
        }
        daily_sizes.push(count_this_tick);
    }

    assert_eq!(daily_hits.len(), n);
    assert!(daily_hits.values().all(|&c| c == 1));
    let min_daily = n / (DAILY_PERIOD as usize);
    let max_daily = n.div_ceil(DAILY_PERIOD as usize) + 1;
    assert!(daily_sizes.iter().all(|&c| c >= min_daily && c <= max_daily));

    let mut weekly_hits: BTreeMap<u64, u32> = BTreeMap::new();
    for t in 0..WEEKLY_PERIOD {
        for i in 0..n {
            let eid = (i as u64) * 37 + 13;
            if reg.is_due(eid, Tick(t), WEEKLY_PERIOD) {
                *weekly_hits.entry(eid).or_insert(0) += 1;
            }
        }
    }
    assert_eq!(weekly_hits.len(), n);
    assert!(weekly_hits.values().all(|&c| c == 1));
}

#[derive(Default)]
struct DummyCartridge {
    tag: &'static str,
    acc: u64,
    values: Vec<u64>,
}

impl CanonicalSaveablePort for DummyCartridge {
    fn cartridge_tag(&self) -> &'static str {
        self.tag
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u64_le(out, self.acc);
        write_u64_le(out, self.values.len() as u64);
        for &v in &self.values {
            write_u64_le(out, v);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.acc = read_u64_le(input)?;
        let len = read_u64_le(input)? as usize;
        self.values.clear();
        for _ in 0..len {
            self.values.push(read_u64_le(input)?);
        }
        Ok(())
    }
}

fn run_synthetic_multithread_workload(threads: usize, ticks: u64) -> [u8; 32] {
    let pool = build_worker_pool(threads).unwrap();
    let mut clock = SimulationClock::new(987654321);
    let mut cart = DummyCartridge {
        tag: "dummy-work",
        acc: 0,
        values: (0..256).collect(),
    };
    let entity_ids: Vec<u64> = (10..522).collect();

    for step in 1..=ticks {
        clock.current_tick = Tick(step);
        let deltas = execute_parallel_sorted(Some(&pool), &entity_ids, |&eid| {
            let mut rng = phase_rng(
                clock.world_seed,
                Tick(step),
                TickPhase::DailyBucketExtractionProductionConstruction,
                eid,
            );
            let r = rng.next_u64() % 97;
            vec![(eid, 0, (eid % 256, r))]
        });
        for (slot, val) in deltas {
            cart.values[slot as usize] = cart.values[slot as usize].wrapping_add(val);
            cart.acc = cart.acc.wrapping_add(val);
        }
    }
    compute_canonical_digest(&clock, &[&cart])
}

// @req ENG-CLK-004
#[test]
fn parallel_counter_rng_yields_identical_digests_across_thread_counts() {
    let h1 = run_synthetic_multithread_workload(1, 240);
    let h2 = run_synthetic_multithread_workload(2, 240);
    let h4 = run_synthetic_multithread_workload(4, 240);
    let h8 = run_synthetic_multithread_workload(8, 240);
    assert_eq!(h1, h2);
    assert_eq!(h1, h4);
    assert_eq!(h1, h8);
}

// @req ENG-CLK-005
#[test]
fn save_and_load_codec_preserves_exact_digest_and_rejects_mismatched_mods_or_versions() {
    let mod_hash = *blake3::hash(b"m1-mod-v1").as_bytes();
    let mut clock_a = SimulationClock::new(1337);
    let mut cart_a = DummyCartridge {
        tag: "dummy-save",
        acc: 55,
        values: vec![1, 2, 3, 4],
    };
    for t in 1..=25 {
        clock_a.current_tick = Tick(t);
        cart_a.acc = cart_a.acc.wrapping_mul(3).wrapping_add(t);
        cart_a.values[0] = cart_a.acc;
    }
    let bytes = save_snapshot_bytes(mod_hash, &clock_a, &[&cart_a]);

    let mut clock_b = SimulationClock::new(0);
    let mut cart_b = DummyCartridge {
        tag: "dummy-save",
        acc: 0,
        values: Vec::new(),
    };
    let loaded_digest = load_snapshot_bytes(mod_hash, &bytes, &mut clock_b, &mut [&mut cart_b])
        .expect("valid archive must decode");
    assert_eq!(loaded_digest, compute_canonical_digest(&clock_a, &[&cart_a]));
    assert_eq!(clock_a, clock_b);
    assert_eq!(cart_a.values, cart_b.values);

    // Negative check: mismatched mod hash rejected cleanly.
    let bad_mod_hash = *blake3::hash(b"different-mod").as_bytes();
    let err_mod = load_snapshot_bytes(bad_mod_hash, &bytes, &mut clock_b, &mut [&mut cart_b]);
    assert_eq!(err_mod, Err(SaveCodecError::ModBundleHashMismatch));

    // Negative check: tampered format version rejected cleanly.
    let mut bad_ver = bytes.clone();
    bad_ver[4..8].copy_from_slice(&99u32.to_le_bytes());
    let err_ver = load_snapshot_bytes(mod_hash, &bad_ver, &mut clock_b, &mut [&mut cart_b]);
    assert_eq!(err_ver, Err(SaveCodecError::UnsupportedFormatVersion(99)));
}

// @req ENG-CLK-006
#[test]
fn telemetry_ring_records_subsystems_without_mutating_simulation_digests() {
    let clock = SimulationClock::new(77);
    let cart = DummyCartridge {
        tag: "dummy-tele",
        acc: 9,
        values: vec![42],
    };
    let digest_before = compute_canonical_digest(&clock, &[&cart]);

    let mut ring = TickTelemetryRing::new();
    for t in 1..=5 {
        ring.record_sample(PhaseTimingSample {
            tick: Tick(t),
            phase_ns: [100, 2_500, 300, 800, 600, 1_900, 400, 500],
            total_ns: 7_100,
            bucket_entities_processed: [1, 642, 0, 12, 8, 24, 16, 64],
        });
    }

    let summary = ring.telemetry_summary();
    assert_eq!(summary.total_ticks_profiled, 5);
    let json = ring.to_json_value();
    assert!(json["weatherMs"].as_f64().unwrap() > 0.0);
    assert!(json["subsystems"]["player_view"]["avgUsPerTick"].as_f64().unwrap() > 0.0);
    let human = ring.to_human_readable_report();
    assert!(human.contains("weather"));
    assert!(human.contains("transport"));
    assert!(human.contains("production"));
    assert!(human.contains("population"));
    assert!(human.contains("player_view"));
    assert_eq!(compute_canonical_digest(&clock, &[&cart]), digest_before);
}
