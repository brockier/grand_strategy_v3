//! External data-driven schema validation, five-layer mod merging, and Draft 2020-12 JSON Schema export.
#![deny(clippy::disallowed_types)]

pub mod loader;
pub mod schema;

pub use loader::{CompiledModBundle, DiagnosticSeverity, ModDiagnostic, export_json_schemas, load_mod_bundle};
pub use schema::ModLayerKind;
