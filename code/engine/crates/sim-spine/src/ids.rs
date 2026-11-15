//! Strongly-typed opaque identifiers shared across simulation cartridges.

macro_rules! define_spine_id {
    ($name:ident, $inner:ty, $doc:expr) => {
        #[doc = $doc]
        #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
        pub struct $name(pub $inner);

        impl $name {
            /// Returns the underlying integer index.
            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }
    };
}

define_spine_id!(Tick, u64, "Discrete simulation hour index starting at 0.");
define_spine_id!(CellId, u32, "Goldberg polyhedron hex or pentagon cell index.");
define_spine_id!(EdgeId, u32, "Directed or undirected cell-to-cell adjacency edge.");
define_spine_id!(PlateId, u16, "Tectonic plate identifier.");
define_spine_id!(WatershedId, u32, "Hydrological drainage basin identifier.");
define_spine_id!(SettlementId, u32, "Named settled site or outpost identifier.");
define_spine_id!(FacilityId, u32, "Productive facility, mine, farm, or workshop identifier.");
define_spine_id!(StockpileId, u32, "Spatial inventory yard, granary, warehouse, or cargo bay.");
define_spine_id!(BatchId, u64, "Distinct physical commodity batch stamped with purity and age.");
define_spine_id!(CommodityId, u16, "Interned external commodity definition index.");
define_spine_id!(ProcessId, u16, "Interned extraction, refining, farming, or building recipe index.");
define_spine_id!(CarrierClassId, u16, "Interned carrier class definition index.");
define_spine_id!(ConvoyId, u64, "Active physical moving transit entity identifier.");
define_spine_id!(InfraClassId, u16, "Interned road, bridge, canal, port, or rail class index.");
define_spine_id!(CohortId, u32, "Grouped demographic cohort per settlement and vocation.");
define_spine_id!(VocationId, u16, "Interned occupational skill domain index.");
define_spine_id!(CharacterId, u32, "Named Individual (leader, overseer, general, scholar, merchant).");
define_spine_id!(ActorId, u32, "Legal entity (state bureau, household, estate, merchant house, syndicate).");
define_spine_id!(MandateId, u64, "Issued strategic directive or delegated sub-mandate.");
define_spine_id!(ProposalId, u64, "Subordinate escalation proposal awaiting arbitration.");
define_spine_id!(DispatchId, u64, "Physical courier message or signal packet in transit or archived.");
define_spine_id!(TechComponentId, u16, "Modular empirical Know-How component identifier.");
define_spine_id!(TheoryDomainId, u16, "Theoretical Know-Why scientific domain identifier.");
define_spine_id!(FormationId, u32, "Military division, corps, or field detachment identifier.");
define_spine_id!(TheaterId, u16, "Operational military theater identifier.");
define_spine_id!(EventClassId, u16, "Interned notification and auto-pause event category index.");
define_spine_id!(ProceduralMaterialId, u16, "World-sampled procedural ore, flux, fuel, crop, fauna, or pathogen identifier.");
