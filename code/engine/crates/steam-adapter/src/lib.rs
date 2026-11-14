//! Isolated optional Steamworks integration adapter (`ENG-LIC-002`).
//! Core simulation crates (`sim-spine`, `sim-data`, `sim-core`, `player-view`) never depend on this crate.

use sim_spine::{PlatformError, PlatformIntegrationPort};

/// Default DRM-free adapter requiring zero proprietary store SDK files.
#[derive(Default, Debug, Clone, Copy)]
pub struct NoopPlatformAdapter;

impl PlatformIntegrationPort for NoopPlatformAdapter {
    fn ensure_launched_via_store(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn unlock_milestone_flag(&self, _key: &str) {}

    fn tick_callbacks(&mut self) {}
}

/// Feature-gated adapter activated strictly in desktop store builds with `--features steam`.
#[cfg(feature = "steam")]
#[derive(Default, Debug)]
pub struct SteamPlatformAdapter {
    pub app_id: u32,
}

#[cfg(feature = "steam")]
impl PlatformIntegrationPort for SteamPlatformAdapter {
    fn ensure_launched_via_store(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn unlock_milestone_flag(&self, _key: &str) {}

    fn tick_callbacks(&mut self) {}
}
