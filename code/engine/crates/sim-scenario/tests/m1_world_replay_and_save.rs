#![deny(clippy::disallowed_types)]

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use sim_clock::SaveCodecError;
use sim_data::load_mod_bundle;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{ModRegistryPort, SpinePlayerCommand, Tick};

fn colony_bundle() -> sim_data::CompiledModBundle {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mods_root = manifest_dir.join("../../../../mods/unexplained-colony");
    let stack = resolve_scenario_pack_stack(&[], Some(&mods_root));
    let refs: Vec<&std::path::Path> = stack.iter().map(PathBuf::as_path).collect();
    load_mod_bundle(&refs).expect("unexplained-colony stack must load cleanly")
}

// @req ENG-CLK-004
#[test]
fn multi_threaded_replay_produces_bit_identical_digests_across_thread_counts() {
    let bundle = colony_bundle();
    let seed = 4242_u64;
    let total_ticks = 2_160_u64;

    // Generate deterministic stream of 40 timestamped player commands.
    let mut cmd_rng = ChaCha8Rng::seed_from_u64(9001);
    let mut commands = Vec::with_capacity(40);
    for idx in 0..40 {
        let target_tick = Tick(1 + (cmd_rng.next_u64() % (total_ticks - 1)));
        let cmd = match idx % 4 {
            0 => SpinePlayerCommand::SetSpeedMultiplier(((cmd_rng.next_u64() % 4) + 1) as u32),
            1 => SpinePlayerCommand::SetPaused((idx / 4) % 2 == 0),
            2 => SpinePlayerCommand::AcknowledgeNotification(cmd_rng.next_u64() % 500),
            _ => SpinePlayerCommand::StubPing {
                tag: format!("cmd_{idx}"),
            },
        };
        commands.push((target_tick, cmd));
    }

    let mut digests = Vec::new();
    for threads in [1usize, 2, 4, 8] {
        let mut world = ScenarioWorld::from_registry(&bundle, seed);
        world
            .set_worker_threads(threads)
            .expect("worker pool init must succeed");
        for &(t, ref c) in &commands {
            world.enqueue_command(t, c.clone());
        }
        world.step_ticks(&bundle, total_ticks);
        world.verify_invariants().expect("invariants must hold");
        digests.push((threads, world.state_digest_hex()));
    }

    let baseline = &digests[0].1;
    for (threads, hex) in &digests {
        assert_eq!(
            hex, baseline,
            "thread count {threads} diverged from single-threaded digest"
        );
    }
}

// @req ENG-CLK-005
#[test]
fn canonical_save_and_reload_roundtrip_matches_uninterrupted_run_and_guards_mod_hash() {
    let bundle = colony_bundle();
    let seed = 1337_u64;

    let mut continuous = ScenarioWorld::from_registry(&bundle, seed);
    continuous.step_ticks(&bundle, 1_500);
    let expected_digest = continuous.state_digest_hex();

    let mut split_run = ScenarioWorld::from_registry(&bundle, seed);
    split_run.step_ticks(&bundle, 500);

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let save_path = std::env::temp_dir().join(format!("ng_m1_save_test_{unique}.ngsave"));
    split_run
        .save_to_path(&save_path, &bundle)
        .expect("saving canonical archive must succeed");

    // Reload into fresh ScenarioWorld instance initialized with a completely different seed.
    let mut restored = ScenarioWorld::from_registry(&bundle, 999_999);
    restored
        .load_from_path(&save_path, &bundle)
        .expect("reloading canonical archive must succeed");
    assert_eq!(restored.clock.current_tick, Tick(500));

    restored.step_ticks(&bundle, 1_000);
    assert_eq!(restored.clock.current_tick, Tick(1_500));
    assert_eq!(
        restored.state_digest_hex(),
        expected_digest,
        "post-reload simulation diverged from uninterrupted run"
    );

    // Verify incompatible mod hash rejection on load.
    let mut mismatch_hash = bundle.bundle_blake3_bytes();
    mismatch_hash[0] ^= 0xFF;
    let raw_archive = fs::read(&save_path).expect("read saved archive");
    let mut other_world = ScenarioWorld::from_registry(&bundle, 1);
    let err = sim_clock::load_snapshot_bytes(
        mismatch_hash,
        &raw_archive,
        &mut other_world.clock,
        &mut [
            &mut other_world.planet,
            &mut other_world.weather,
            &mut other_world.core,
            &mut other_world.archive,
        ],
    )
    .expect_err("mismatched mod hash must be rejected");
    assert_eq!(err, SaveCodecError::ModBundleHashMismatch);

    let _ = fs::remove_file(&save_path);
}
