import {
  ArbitrationProposalDto,
  AuditDiscrepancyDto,
  ClockAndNotificationsDto,
  DelegationTreeDto,
  DiegeticDispatchDto,
  DiegeticSnapshot,
  FacilityRowDto,
  GlobeTopologyDto,
  GovernanceSnapshotDto,
  HqPresenceDto,
  KnownCellDto,
  KnownSettlementDto,
  KnowledgeSnapshotDto,
  NonStateEconomySnapshotDto,
  NotificationRuleDto,
  ObservedWeatherDto,
  PopulationCohortRowDto,
  ReportedLedgerRowDto,
  SuccessionSnapshotDto,
  SurveyedRoadEdgeDto,
  TimeSeriesDto,
  TrackedConvoyDto,
  WarfareSnapshotDto,
  isValidDiegeticSnapshot,
} from "./contracts";

export interface DiegeticStoreAccess {
  getSnapshot(): DiegeticSnapshot | null;
  subscribe(listener: (snapshot: DiegeticSnapshot) => void): () => void;
}

/** Immutable snapshot store populated exclusively from `player-view` `DiegeticSnapshot` payloads. */
export class DiegeticStore implements DiegeticStoreAccess {
  private currentSnapshot: DiegeticSnapshot | null = null;
  private readonly listeners = new Set<(snapshot: DiegeticSnapshot) => void>();

  public ingestSnapshot(candidate: DiegeticSnapshot): void {
    if (!isValidDiegeticSnapshot(candidate)) {
      throw new Error("Rejected invalid or air-gap-violating DiegeticSnapshot payload");
    }
    this.currentSnapshot = candidate;
    for (const listener of this.listeners) {
      listener(candidate);
    }
  }

  public getSnapshot(): DiegeticSnapshot | null {
    return this.currentSnapshot;
  }

  public subscribe(listener: (snapshot: DiegeticSnapshot) => void): () => void {
    this.listeners.add(listener);
    if (this.currentSnapshot !== null) {
      listener(this.currentSnapshot);
    }
    return () => {
      this.listeners.delete(listener);
    };
  }
}

export function selectGlobeTopology(store: DiegeticStoreAccess): GlobeTopologyDto | null {
  return store.getSnapshot()?.globeTopology ?? null;
}

export function selectKnownCells(store: DiegeticStoreAccess): readonly KnownCellDto[] {
  return store.getSnapshot()?.knownCells ?? [];
}

export function selectHqPresence(store: DiegeticStoreAccess): HqPresenceDto | null {
  return store.getSnapshot()?.hqPresence ?? null;
}

export function selectKnownSettlements(store: DiegeticStoreAccess): readonly KnownSettlementDto[] {
  return store.getSnapshot()?.knownSettlements ?? [];
}

export function selectSurveyedRoads(store: DiegeticStoreAccess): readonly SurveyedRoadEdgeDto[] {
  return store.getSnapshot()?.surveyedRoads ?? [];
}

export function selectTrackedConvoys(store: DiegeticStoreAccess): readonly TrackedConvoyDto[] {
  return store.getSnapshot()?.trackedConvoys ?? [];
}

export function selectObservedWeather(store: DiegeticStoreAccess): readonly ObservedWeatherDto[] {
  return store.getSnapshot()?.observedWeather ?? [];
}

export function selectLedgerRows(
  store: DiegeticStoreAccess,
  settlementId?: number,
): readonly ReportedLedgerRowDto[] {
  const rows = store.getSnapshot()?.ledgerRows ?? [];
  if (settlementId === undefined) return rows;
  return rows.filter((r) => r.settlementId === settlementId);
}

export function selectDelegationTree(store: DiegeticStoreAccess): DelegationTreeDto | null {
  return store.getSnapshot()?.delegationTree ?? null;
}

export function selectDispatchInbox(store: DiegeticStoreAccess): readonly DiegeticDispatchDto[] {
  return store.getSnapshot()?.dispatchInbox ?? [];
}

export function selectClockAndNotifications(
  store: DiegeticStoreAccess,
): ClockAndNotificationsDto | null {
  return store.getSnapshot()?.clockAndNotifications ?? null;
}

export function selectPopulationRows(
  store: DiegeticStoreAccess,
  settlementId?: number,
): readonly PopulationCohortRowDto[] {
  const rows = store.getSnapshot()?.populationRows ?? [];
  if (settlementId === undefined) return rows;
  return rows.filter((r) => r.settlementId === settlementId);
}

export function selectFacilityRows(
  store: DiegeticStoreAccess,
  settlementId?: number,
): readonly FacilityRowDto[] {
  const rows = store.getSnapshot()?.facilityRows ?? [];
  if (settlementId === undefined) return rows;
  return rows.filter((r) => r.settlementId === settlementId);
}

export function selectTimeSeries(store: DiegeticStoreAccess): readonly TimeSeriesDto[] {
  return store.getSnapshot()?.timeSeries ?? [];
}

export function selectOpenProposals(store: DiegeticStoreAccess): readonly ArbitrationProposalDto[] {
  return store.getSnapshot()?.openProposals ?? [];
}

export function selectNotificationRules(store: DiegeticStoreAccess): readonly NotificationRuleDto[] {
  return store.getSnapshot()?.notificationRules ?? [];
}

export function selectAuditDiscrepancies(store: DiegeticStoreAccess): readonly AuditDiscrepancyDto[] {
  return store.getSnapshot()?.auditDiscrepancies ?? [];
}

export function selectGovernance(store: DiegeticStoreAccess): GovernanceSnapshotDto | null {
  return store.getSnapshot()?.governance ?? null;
}

export function selectNonStateEconomy(store: DiegeticStoreAccess): NonStateEconomySnapshotDto | null {
  return store.getSnapshot()?.nonStateEconomy ?? null;
}

export function selectKnowledge(store: DiegeticStoreAccess): KnowledgeSnapshotDto | null {
  return store.getSnapshot()?.knowledge ?? null;
}

export function selectWarfare(store: DiegeticStoreAccess): WarfareSnapshotDto | null {
  return store.getSnapshot()?.warfare ?? null;
}

export function selectSuccession(store: DiegeticStoreAccess): SuccessionSnapshotDto | null {
  return store.getSnapshot()?.succession ?? null;
}
