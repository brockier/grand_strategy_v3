//! Authoritative Simulation Spine exporting strongly-typed opaque IDs, narrow Port traits,
//! deterministic TickPhase scheduling, and pre-wired M5-M7 extension plug hooks.

pub mod ids;
pub mod plugs;
pub mod ports;

pub use ids::*;
pub use plugs::*;
pub use ports::*;
