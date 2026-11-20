/**
 * Authoritative UI Spine contracts mirroring `player-view` IPC DTOs (`UI-SPINE-00`, `UI-SHL-002`).
 * Pure TypeScript definitions and runtime schema validators with zero sibling or Electron/Three imports.
 */

export type RgbTuple = readonly [number, number, number];

export interface ModDiagnosticDto {
  readonly severity: string;
  readonly layer: string;
  readonly filePath: string;
  readonly line: number;
  readonly column: number;
  readonly fieldPath: string;
  readonly message: string;
}

export interface ModValidationFailureState {
  readonly ok: false;
  readonly diagnostics: readonly ModDiagnosticDto[];
}

export interface DepositEstimateDto {
  readonly materialId: number;
  readonly materialLabel: string;
  readonly categoryKey: string;
  readonly minTonnes: number;
  readonly pointEstTonnes: number;
  readonly maxTonnes: number;
  readonly purityEst01: number;
  readonly confidence01: number;
  readonly surveyorSkill01: number;
  readonly instrumentTier: number;
  readonly observedTick: number;
  readonly provenanceLabel: string;
  readonly accessibilityTier?: number;
  readonly isOffshore?: boolean;
  readonly isAlluvialPlacer?: boolean;
}

export interface DamStatusDto {
  readonly cellId: number;
  readonly ruleCurve: string;
  readonly reservoirStorageM3: number;
  readonly reservoirCapacityM3: number;
  readonly reservoirFill01: number;
  readonly reservoirHeadM: number;
  readonly spillwayReleaseM3S: number;
  readonly liveHydroKw: number;
  readonly sedimentSilt01: number;
  readonly condition01: number;
  readonly breached: boolean;
}

export type GlobeProjectionMode = "globe" | "equal_area" | "mercator";

export interface GlobeMapLabelAnchor {
  readonly id: string;
  readonly kind: "settlement" | "formation";
  readonly entityId: number;
  readonly text: string;
  readonly subtitle: string;
  readonly unit: readonly [number, number, number];
  readonly isPrimary: boolean;
}

export type KnownCellDto =
  | {
      readonly tier: "unexplored";
      readonly cellId: number;
    }
  | {
      readonly tier: "rumored";
      readonly cellId: number;
      readonly observedTick: number;
      readonly elevationM: number;
      readonly confidence01: number;
      readonly provenanceLabel: string;
      readonly biomeKey?: string;
      readonly isOcean?: boolean;
    }
  | {
      readonly tier: "stale";
      readonly cellId: number;
      readonly observedTick: number;
      readonly lastLiveTick?: number;
      readonly elevationM: number;
      readonly confidence01: number;
      readonly provenanceLabel: string;
      readonly biomeKey?: string;
      readonly isOcean?: boolean;
      readonly riverDownstreamCellId?: number | null;
      readonly riverFlowNorm01?: number;
      readonly depositEstimates?: readonly DepositEstimateDto[];
      readonly meanAnnualTempC?: number;
      readonly annualPrecipMm?: number;
      readonly aridityIndex01?: number;
      readonly soilFertility01?: number;
      readonly rockStrataCode?: number;
      readonly drainageAreaKm2?: number;
      readonly strahlerOrder?: number;
      readonly isLake?: boolean;
      readonly riverChannelDownstreamCellId?: number | null;
      readonly riverDischargeM3S?: number;
      readonly riverVelocityMps?: number;
      readonly hydroPotentialKw?: number;
      readonly geologyProvince?: string;
      readonly roughnessM?: number;
      readonly basinDepthM?: number;
      readonly faultInstability01?: number;
      readonly volcanicHazard01?: number;
      readonly soilDrainage01?: number;
      readonly soilClay01?: number;
      readonly marineUpwelling01?: number;
      readonly shelfNutrient01?: number;
      readonly lakeKind?: string;
      readonly lakeSalinityPerMille?: number;
      readonly isEstuary?: boolean;
      readonly riverNavigability?: string;
      readonly isSpringOasis?: boolean;
      readonly groundwaterM3?: number;
      readonly damStatus?: DamStatusDto | null;
    }
  | {
      readonly tier: "surveyed";
      readonly cellId: number;
      readonly observedTick: number;
      readonly isLiveHqSight: boolean;
      readonly elevationM: number;
      readonly confidence01: number;
      readonly provenanceLabel: string;
      readonly biomeKey?: string;
      readonly isOcean?: boolean;
      readonly riverDownstreamCellId?: number | null;
      readonly riverFlowNorm01?: number;
      readonly depositEstimates?: readonly DepositEstimateDto[];
      readonly meanAnnualTempC?: number;
      readonly annualPrecipMm?: number;
      readonly aridityIndex01?: number;
      readonly soilFertility01?: number;
      readonly rockStrataCode?: number;
      readonly drainageAreaKm2?: number;
      readonly strahlerOrder?: number;
      readonly isLake?: boolean;
      readonly riverChannelDownstreamCellId?: number | null;
      readonly riverDischargeM3S?: number;
      readonly riverVelocityMps?: number;
      readonly hydroPotentialKw?: number;
      readonly geologyProvince?: string;
      readonly roughnessM?: number;
      readonly basinDepthM?: number;
      readonly faultInstability01?: number;
      readonly volcanicHazard01?: number;
      readonly soilDrainage01?: number;
      readonly soilClay01?: number;
      readonly marineUpwelling01?: number;
      readonly shelfNutrient01?: number;
      readonly lakeKind?: string;
      readonly lakeSalinityPerMille?: number;
      readonly isEstuary?: boolean;
      readonly riverNavigability?: string;
      readonly isSpringOasis?: boolean;
      readonly groundwaterM3?: number;
      readonly damStatus?: DamStatusDto | null;
    };

export interface KnownSettlementDto {
  readonly settlementId: number;
  readonly settlementName: string;
  readonly cellId: number;
  readonly isHq: boolean;
}

export interface SurveyedRoadEdgeDto {
  readonly edgeId: number;
  readonly fromCellId: number;
  readonly toCellId: number;
  readonly infraKey: string;
  readonly washedOut?: boolean;
  readonly riverEdge?: boolean;
  readonly bridgeCapKg?: number | null;
  readonly roadPurpose?: string;
  readonly purposeLabel?: string;
  readonly cumulativePassages?: number;
  readonly cumulativeTonnesPassed?: number;
}

export interface CommLatencyContourDto {
  readonly cellId: number;
  readonly estimatedCourierHours: number;
}

export interface HqRelocationPreviewDto {
  readonly targetCellId: number;
  readonly label: string;
  readonly travelHours: number;
  readonly projectedHavenLatencyHours: number;
}

export interface HqPresenceDto {
  readonly mode: string;
  readonly cellId: number;
  readonly settlementName: string;
  readonly sightRadiusCells: number;
  readonly inTransit?: boolean;
  readonly destinationCellId?: number | null;
  readonly etaTick?: number | null;
  readonly communicationContours?: readonly CommLatencyContourDto[];
  readonly relocationPreviews?: readonly HqRelocationPreviewDto[];
}

export interface GlobeTopologyDto {
  readonly subdivision: number;
  readonly cellCount: number;
  readonly triangles?: readonly (readonly [number, number, number])[];
}

export interface ObservedWeatherDto {
  readonly cellId: number;
  readonly observedTick: number;
  readonly precipMmHr: number;
  readonly mudIndex01: number;
  readonly tempC: number;
  readonly visibility01: number;
  readonly floodStage01?: number;
  readonly snowDepthM?: number;
  readonly iceThicknessM?: number;
  readonly windUMps?: number;
  readonly windVMps?: number;
}

export interface ReportedLedgerRowDto {
  readonly settlementId: number;
  readonly settlementName: string;
  readonly commodityKey: string;
  readonly commodityName: string;
  readonly reportedMassKg: number;
  readonly minEstKg: number;
  readonly maxEstKg: number;
  readonly observedTick: number;
  readonly provenanceLabel: string;
  readonly category?: string;
  readonly confidence01?: number;
  readonly reporterName?: string;
}

export interface PopulationCohortRowDto {
  readonly cohortId: number;
  readonly settlementId: number;
  readonly settlementName: string;
  readonly vocationKey: string;
  readonly vocationName: string;
  readonly reportedHeadcount: number;
  readonly minHeadcount: number;
  readonly maxHeadcount: number;
  readonly meanSkill01: number;
  readonly literacy01: number;
  readonly nutrition01: number;
  readonly unrestStage: string;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
}

export interface FacilityRowDto {
  readonly facilityId: number;
  readonly settlementId: number;
  readonly settlementName: string;
  readonly processKey: string;
  readonly processName: string;
  readonly assignedWorkers: number;
  readonly toolCondition01: number;
  readonly weatherHalted: boolean;
  readonly shaftWater01: number;
  readonly observedTick: number;
  readonly provenanceLabel: string;
}

export interface TimeSeriesPointDto {
  readonly tick: number;
  readonly value: number;
  readonly minBound: number;
  readonly maxBound: number;
  readonly isConfirmed: boolean;
}

export interface TimeSeriesDto {
  readonly seriesKey: string;
  readonly label: string;
  readonly unit: string;
  readonly points: readonly TimeSeriesPointDto[];
}

export interface MandateSummaryDto {
  readonly mandateId: number;
  readonly assigneeId: number;
  readonly targetSettlementId: number;
  readonly targetCellId: number;
  readonly objectiveKind: string;
  readonly authorityTier: string;
  readonly summary: string;
  readonly priorityWeight: number;
  readonly status: string;
  readonly realizedEfficiency: number;
  readonly progress01: number;
  readonly subtasks: readonly string[];
}

export interface SubordinateSummaryDto {
  readonly characterId: number;
  readonly displayName: string;
  readonly roleTitle: string;
  readonly competence: number;
  readonly homeSettlementId?: number;
  readonly homeCellId?: number;
  readonly precision01?: number;
  readonly loyaltyLabel?: string;
  readonly allocatedAttention?: number;
  readonly assignedMandates?: readonly MandateSummaryDto[];
}

export interface DelegationTreeDto {
  readonly leaderTitle: string;
  readonly capacityAttention: number;
  readonly usedAttention: number;
  readonly subordinates: readonly SubordinateSummaryDto[];
  readonly clericalOverhead?: number;
  readonly overloadRatio?: number;
  readonly focusedMandateId?: number | null;
  readonly focusedAttentionShare?: number;
  readonly activeMandates?: readonly MandateSummaryDto[];
}

export interface TrackedConvoyDto {
  readonly convoyId: number;
  readonly label: string;
  readonly status: string;
  readonly projectedCellId: number;
  readonly channel?: string;
  readonly originCellId?: number;
  readonly destinationCellId?: number;
  readonly originUnitXyz?: readonly [number, number, number];
  readonly currentUnitXyz?: readonly [number, number, number];
  readonly destinationUnitXyz?: readonly [number, number, number];
  readonly routeWaypointsXyz?: readonly (readonly [number, number, number])[];
  readonly distanceTravelledKm?: number;
  readonly totalRouteKm?: number;
  readonly routeCells?: readonly number[];
  readonly departureTick?: number;
  readonly expectedArrivalTick?: number;
  readonly confidence01?: number;
  readonly provenanceLabel?: string;
}

export interface DispatchTokenDto {
  readonly text: string;
  readonly entityKind: string | null;
  readonly entityId: number | null;
  readonly targetCellId: number | null;
}

export interface DiegeticDispatchDto {
  readonly dispatchId: number;
  readonly title: string;
  readonly sentTick: number;
  readonly arrivedTick: number;
  readonly originLabel: string;
  readonly senderId?: number;
  readonly senderName?: string;
  readonly originCellId?: number;
  readonly eventClassKey?: string;
  readonly severity?: string;
  readonly confidence01?: number;
  readonly bodyTokens?: readonly DispatchTokenDto[];
}

export interface ArbitrationOptionDto {
  readonly index: number;
  readonly label: string;
  readonly description: string;
  readonly attentionCost: number;
}

export interface ArbitrationProposalDto {
  readonly proposalId: number;
  readonly settlementId: number;
  readonly originCellId: number;
  readonly raisedById: number;
  readonly raisedByName: string;
  readonly title: string;
  readonly summary: string;
  readonly createdTick: number;
  readonly deadlineTick: number;
  readonly options: readonly ArbitrationOptionDto[];
}

export interface NotificationRuleDto {
  readonly eventClassKey: string;
  readonly displayName: string;
  readonly behavior: string;
}

export interface AuditDiscrepancyDto {
  readonly settlementId: number;
  readonly characterId: number;
  readonly characterName: string;
  readonly commodityKey: string;
  readonly claimedMassKg: number;
  readonly verifiedMassKg: number;
  readonly discrepancyKg: number;
  readonly detectedTick: number;
}

export interface ClockAndNotificationsDto {
  readonly tick: number;
  readonly calendarText: string;
  readonly paused: boolean;
  readonly speedMultiplier: number;
  readonly activeAlertCount: number;
  readonly autoPauseReason?: string | null;
}

export interface GovernanceModelOptionDto {
  readonly modelKey: string;
  readonly displayName: string;
  readonly baseAttentionCapacity: number;
  readonly clericalOverheadPerMandate: number;
  readonly hierarchyHopLatencyHours: number;
}

export interface PolicyLawOptionDto {
  readonly policyKey: string;
  readonly displayName: string;
  readonly taxOrTributeRate01: number;
  readonly patentDurationYears: number;
}

export interface GovernanceSnapshotDto {
  readonly activeModelKey: string;
  readonly activeModelName: string;
  readonly previousModelKey: string | null;
  readonly adaptationProgress01: number;
  readonly activePolicyKey: string;
  readonly activePolicyName: string;
  readonly taxOrTributeRate01: number;
  readonly patentDurationYears: number;
  readonly skimmingLeakageFraction: number;
  readonly effectiveAttentionCapacity: number;
  readonly clericalOverheadPerMandate: number;
  readonly hierarchyHopLatencyHours: number;
  readonly charteredCarriersTotal: number;
  readonly bureaucraticCaptureEvents: number;
  readonly availableModels: readonly GovernanceModelOptionDto[];
  readonly availablePolicies: readonly PolicyLawOptionDto[];
}

export interface NonStateActorDto {
  readonly actorId: number;
  readonly name: string;
  readonly kind: string;
  readonly homeSettlementId: number;
  readonly settlementName: string;
  readonly workingCapitalCoins: number;
  readonly heldCommodityKey: string | null;
  readonly heldMassKg: number;
  readonly hoardedMassKg: number;
  readonly privateCarriersOwned: number;
  readonly charteredToStateCarriers: number;
  readonly compliance01: number;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
}

export interface InformalMarketDto {
  readonly settlementId: number;
  readonly settlementName: string;
  readonly commodityKey: string;
  readonly commodityName: string;
  readonly clearingPriceIndex: number;
  readonly blackMarketPremium01: number;
  readonly blackMarketLeakageKg: number;
  readonly hoardedReserveKg: number;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
}

export interface EdgeCongestionDto {
  readonly edgeId: number;
  readonly fromCellId: number;
  readonly toCellId: number;
  readonly hourlyFlowTonnes: number;
  readonly capacityTonnes: number;
  readonly congestionMultiplier: number;
}

export interface NonStateEconomySnapshotDto {
  readonly actors: readonly NonStateActorDto[];
  readonly informalMarkets: readonly InformalMarketDto[];
  readonly congestedEdges: readonly EdgeCongestionDto[];
}

export interface KnowledgeComponentScoreDto {
  readonly componentId: number;
  readonly componentKey: string;
  readonly displayName: string;
  readonly knowHow01: number;
}

export interface TheoryDomainScoreDto {
  readonly domainId: number;
  readonly domainKey: string;
  readonly displayName: string;
  readonly knowWhy01: number;
}

export interface KnowledgeCarrierDto {
  readonly settlementId: number;
  readonly settlementName: string;
  readonly componentKey: string;
  readonly carrierKind: string;
  readonly retentionStrength01: number;
  readonly holderLabel: string;
}

export interface SettlementKnowledgeDto {
  readonly settlementId: number;
  readonly settlementName: string;
  readonly innovationModeKey: string;
  readonly bottleneckPressure01: number;
  readonly leisureSurplus01: number;
  readonly knowHow: readonly KnowledgeComponentScoreDto[];
  readonly knowWhy: readonly TheoryDomainScoreDto[];
  readonly activePatentHolderActorId: number | null;
  readonly patentExpiryTick: number | null;
  readonly regressionLosses: number;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
}

export interface KnowledgeSnapshotDto {
  readonly settlements: readonly SettlementKnowledgeDto[];
  readonly carriers: readonly KnowledgeCarrierDto[];
  readonly availableModes: readonly string[];
}

export interface DoctrineOptionDto {
  readonly doctrineKey: string;
  readonly displayName: string;
  readonly initiativeBonus: number;
  readonly strictObedienceWeight: number;
  readonly rainPenaltyMitigation: number;
}

export interface TheaterDirectiveDto {
  readonly theaterId: number;
  readonly commanderId: number;
  readonly commanderName: string;
  readonly objectiveCellId: number;
  readonly supplyPriority01: number;
  readonly issuedTick: number;
  readonly autonomousWithdrawals: number;
}

export interface MilitaryFormationDto {
  readonly formationId: number;
  readonly label: string;
  readonly templateKey: string;
  readonly theaterId: number;
  readonly commanderId: number;
  readonly commanderName: string;
  readonly homeSettlementId: number;
  readonly currentCellId: number;
  readonly currentUnitXyz: readonly [number, number, number];
  readonly targetCellId: number;
  readonly targetUnitXyz: readonly [number, number, number];
  readonly personnelCount: number;
  readonly equipmentMassKg: number;
  readonly munitionsDays: number;
  readonly rationsDays: number;
  readonly cohesion01: number;
  readonly doctrineKey: string;
  readonly noRetreatOrder: boolean;
  readonly encircled: boolean;
  readonly insubordinateWithdrawal: boolean;
  readonly staleOrderTicks: number;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
}

export interface WarfareSnapshotDto {
  readonly theaters: readonly TheaterDirectiveDto[];
  readonly formations: readonly MilitaryFormationDto[];
  readonly availableDoctrines: readonly DoctrineOptionDto[];
}

export interface SuccessionCharacterDto {
  readonly characterId: number;
  readonly name: string;
  readonly roleTitle: string;
  readonly ageYears: number;
  readonly health01: number;
  readonly isAlive: boolean;
  readonly isRuler: boolean;
  readonly settlementId: number;
  readonly settlementName: string;
}

export interface SuccessionTransitionDto {
  readonly ruleKey: string;
  readonly ruleDisplayName: string;
  readonly deceasedRulerName: string;
  readonly successorName: string;
  readonly transitionTick: number;
  readonly disruptionUntilTick: number;
  readonly inheritedArchiveEntries: readonly string[];
  readonly lostUnrecordedRapportCount: number;
  readonly awaitingPlayerAcknowledgement: boolean;
}

export interface SuccessionSnapshotDto {
  readonly roster: readonly SuccessionCharacterDto[];
  readonly pendingTransition: SuccessionTransitionDto | null;
  readonly effectiveCommandMultiplier: number;
}

export interface DiegeticSnapshot {
  readonly schemaVersion: number;
  readonly scenarioKey: string;
  readonly scenarioName: string;
  readonly globeTopology: GlobeTopologyDto;
  readonly knownCells: readonly KnownCellDto[];
  readonly hqPresence: HqPresenceDto;
  readonly knownSettlements?: readonly KnownSettlementDto[];
  readonly surveyedRoads?: readonly SurveyedRoadEdgeDto[];
  readonly trackedConvoys: readonly TrackedConvoyDto[];
  readonly observedWeather: readonly ObservedWeatherDto[];
  readonly ledgerRows: readonly ReportedLedgerRowDto[];
  readonly delegationTree: DelegationTreeDto;
  readonly dispatchInbox: readonly DiegeticDispatchDto[];
  readonly clockAndNotifications: ClockAndNotificationsDto;
  readonly populationRows?: readonly PopulationCohortRowDto[];
  readonly facilityRows?: readonly FacilityRowDto[];
  readonly timeSeries?: readonly TimeSeriesDto[];
  readonly openProposals?: readonly ArbitrationProposalDto[];
  readonly notificationRules?: readonly NotificationRuleDto[];
  readonly auditDiscrepancies?: readonly AuditDiscrepancyDto[];
  readonly governance?: GovernanceSnapshotDto | null;
  readonly nonStateEconomy?: NonStateEconomySnapshotDto | null;
  readonly knowledge?: KnowledgeSnapshotDto | null;
  readonly warfare?: WarfareSnapshotDto | null;
  readonly succession?: SuccessionSnapshotDto | null;
}

export type ValidatedPlayerCommand =
  | { readonly kind: "setPaused"; readonly paused: boolean }
  | { readonly kind: "stepSingleTick" }
  | { readonly kind: "setSpeedMultiplier"; readonly multiplier: number }
  | { readonly kind: "pingBridge"; readonly tag: string }
  | {
      readonly kind: "issueMandate";
      readonly assignee: number;
      readonly objectiveKind: string;
      readonly targetCell: number;
      readonly priorityWeight: number;
      readonly authorityTier: string;
    }
  | {
      readonly kind: "resolveArbitrationProposal";
      readonly proposalId: number;
      readonly chosenOptionIndex: number;
    }
  | {
      readonly kind: "setPersonalAttentionZoom";
      readonly targetMandateId: number | null;
      readonly share01: number;
    }
  | {
      readonly kind: "relocateHq";
      readonly destinationCell: number;
    }
  | {
      readonly kind: "updateNotificationRule";
      readonly eventClassKey: string;
      readonly behavior: string;
    }
  | { readonly kind: "switchGovernanceModel"; readonly modelKey: string }
  | {
      readonly kind: "enactEconomicPolicy";
      readonly policyKey: string;
      readonly rateOverride01?: number | null;
    }
  | {
      readonly kind: "charterPrivateCarriers";
      readonly targetSettlement: number;
      readonly count: number;
    }
  | { readonly kind: "setInnovationMode"; readonly settlement: number; readonly modeKey: string }
  | {
      readonly kind: "issueTheaterDirective";
      readonly theaterId: number;
      readonly commander: number;
      readonly objectiveCell: number;
      readonly supplyPriority01: number;
    }
  | {
      readonly kind: "issueOperationalBattlePlan";
      readonly formationId: number;
      readonly targetCell: number;
      readonly doctrineKey: string;
      readonly noRetreat: boolean;
    }
  | { readonly kind: "acknowledgeSuccession" }
  | {
      readonly kind: "constructDam";
      readonly targetCell: number;
      readonly ruleCurve?: string;
    }
  | {
      readonly kind: "setDamRuleCurve";
      readonly targetCell: number;
      readonly ruleCurve: string;
    };

export interface CommandAck {
  readonly accepted: boolean;
  readonly tick: number;
  readonly errorCode: string | null;
  readonly message: string;
}

export interface SkinDescriptor {
  readonly skinId: string;
  readonly displayName: string;
  readonly stylesheetPath: string;
}

export interface GlobePalette {
  readonly oceanShallowRgb: RgbTuple;
  readonly oceanDeepRgb: RgbTuple;
  readonly landLowRgb: RgbTuple;
  readonly landMidRgb: RgbTuple;
  readonly landHighRgb: RgbTuple;
  readonly peakSnowRgb: RgbTuple;
  readonly riverWaterRgb: RgbTuple;
  readonly roadOchreRgb: RgbTuple;
  readonly roadStoneRgb?: RgbTuple;
  readonly settlementMarkerRgb: RgbTuple;
  readonly contourInkRgb: RgbTuple;
  readonly compassRoseRgb: RgbTuple;
  readonly unexploredVellumRgb: RgbTuple;
  readonly parchmentGrainTextureUrl: string;
  readonly hatchPatternTextureUrl: string;
  readonly terrainBiomeTextureUrl: string;
}

export interface AudioManifestDto {
  readonly schemaVersion: number;
  readonly skinId: string;
  readonly ambientSurface: {
    readonly fadeStartAltitude: number;
    readonly fadeEndAltitude: number;
    readonly windLoopUrl: string;
    readonly rainLoopUrl: string;
    readonly settlementLoopUrl: string;
  };
  readonly ambientMapStudy: {
    readonly fadeStartAltitude: number;
    readonly fadeEndAltitude: number;
    readonly studyLoopUrl: string;
  };
  readonly music: {
    readonly crossfadeMs: number;
    readonly stemUrls: readonly string[];
  };
  readonly uiFoley: {
    readonly cooldownMs: number;
    readonly cueUrls: Readonly<Record<string, string>>;
  };
}

export interface PortraitAssetEntry {
  readonly id: string;
  readonly url: string;
  readonly ageBracket: string;
  readonly vocation: string;
  readonly eraTag: string;
  readonly healthBand: string;
}

export interface IconAssetEntry {
  readonly key: string;
  readonly category: string;
  readonly url: string;
}

export interface DispatchPlateEntry {
  readonly key: string;
  readonly url: string;
}

export interface AssetManifestDto {
  readonly schemaVersion: number;
  readonly fallbacks: {
    readonly portraitUrl: string;
    readonly iconUrl: string;
    readonly dispatchPlateUrl: string;
  };
  readonly portraits: readonly PortraitAssetEntry[];
  readonly icons: readonly IconAssetEntry[];
  readonly dispatchPlates: readonly DispatchPlateEntry[];
}

export interface SkinManifestBundle {
  readonly skinId: string;
  readonly displayName: string;
  readonly stylesheetHref: string;
  readonly cssText: string;
  readonly globePalette: GlobePalette;
  readonly audioManifest?: AudioManifestDto;
  readonly assetManifest?: AssetManifestDto;
}

export interface AudioMixerSettings {
  readonly masterVolume: number;
  readonly musicVolume: number;
  readonly ambienceVolume: number;
  readonly foleyVolume: number;
  readonly masterMuted: boolean;
  readonly musicMuted: boolean;
  readonly ambienceMuted: boolean;
  readonly foleyMuted: boolean;
}

export interface UserPreferencesDto {
  readonly activeSkinId?: string;
  readonly audioMixer: AudioMixerSettings;
  readonly activeLayoutJson?: string;
  readonly namedLayoutPresets: Readonly<Record<string, string>>;
  readonly notificationRules?: Readonly<Record<string, string>>;
}

export type GlobeRayHit =
  | {
      readonly hitKind: "convoy";
      readonly cellId: number;
      readonly convoyId: number;
      readonly label: string;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    }
  | {
      readonly hitKind: "settlement";
      readonly cellId: number;
      readonly settlementId: number;
      readonly label: string;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    }
  | {
      readonly hitKind: "formation";
      readonly cellId: number;
      readonly formationId: number;
      readonly label: string;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    }
  | {
      readonly hitKind: "facility";
      readonly cellId: number;
      readonly facilityId: number;
      readonly settlementId?: number;
      readonly label: string;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    }
  | {
      readonly hitKind: "road";
      readonly cellId: number;
      readonly edgeId: number;
      readonly fromCellId: number;
      readonly toCellId: number;
      readonly roadPurpose?: string;
      readonly purposeLabel?: string;
      readonly label: string;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    }
  | {
      readonly hitKind: "river";
      readonly cellId: number;
      readonly toCellId: number;
      readonly flowNorm01: number;
      readonly dischargeM3S?: number;
      readonly velocityMps?: number;
      readonly hydroPotentialKw?: number;
      readonly label: string;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    }
  | {
      readonly hitKind: "cell";
      readonly cellId: number;
      readonly label: string;
      readonly localElevationM?: number;
      readonly unitXyz?: readonly [number, number, number];
      readonly latDeg?: number;
      readonly lonDeg?: number;
    };

export interface CameraTelemetry {
  readonly altitudeNorm: number;
  readonly altitudeRadius: number;
  readonly pitchRad: number;
  readonly headingRad: number;
  readonly focusCellId: number;
  readonly focusUnit: readonly [number, number, number];
}

export type InitialBootPayload =
  | { readonly state: "ready"; readonly snapshot: DiegeticSnapshot }
  | { readonly state: "mod_error"; readonly failure: ModValidationFailureState };

export interface SimBridgeApi {
  getInitialState(): Promise<InitialBootPayload>;
  subscribeSnapshots(listener: (snapshot: DiegeticSnapshot) => void): () => void;
  sendPlayerCommand(command: ValidatedPlayerCommand): Promise<CommandAck>;
  reloadMods(recoverToDefault: boolean): Promise<InitialBootPayload>;
  listAvailableSkins(): Promise<readonly SkinDescriptor[]>;
  readSkinAssetManifest(skinId: string): Promise<SkinManifestBundle>;
  loadMockFixture(fixtureJson: string): Promise<void>;
  subscribeSkinHotReload?(listener: (skinId: string) => void): () => void;
  readUserPreferences?(): Promise<UserPreferencesDto>;
  writeUserPreferences?(prefs: UserPreferencesDto): Promise<void>;
}

/** Default initial mixer levels shared across fresh boots (`UI-AUD-004`). */
export const DEFAULT_AUDIO_MIXER_SETTINGS: AudioMixerSettings = {
  masterVolume: 0.85,
  musicVolume: 0.7,
  ambienceVolume: 0.8,
  foleyVolume: 0.75,
  masterMuted: false,
  musicMuted: false,
  ambienceMuted: false,
  foleyMuted: false,
};

/** Validates incoming IPC payloads against `DiegeticSnapshot` parity rules. */
export function isValidDiegeticSnapshot(value: unknown): value is DiegeticSnapshot {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  if (
    typeof obj.schemaVersion !== "number" ||
    typeof obj.scenarioKey !== "string" ||
    typeof obj.scenarioName !== "string" ||
    typeof obj.globeTopology !== "object" ||
    obj.globeTopology === null ||
    !Array.isArray(obj.knownCells) ||
    typeof obj.hqPresence !== "object" ||
    obj.hqPresence === null ||
    !Array.isArray(obj.trackedConvoys) ||
    !Array.isArray(obj.observedWeather) ||
    !Array.isArray(obj.ledgerRows) ||
    typeof obj.delegationTree !== "object" ||
    obj.delegationTree === null ||
    !Array.isArray(obj.dispatchInbox) ||
    typeof obj.clockAndNotifications !== "object" ||
    obj.clockAndNotifications === null
  ) {
    return false;
  }

  for (const cell of obj.knownCells) {
    if (typeof cell !== "object" || cell === null) return false;
    const c = cell as Record<string, unknown>;
    if (typeof c.cellId !== "number") return false;
    if (c.tier === "unexplored") {
      if (
        "elevationM" in c ||
        "confidence01" in c ||
        "observedTick" in c ||
        "biomeKey" in c ||
        "isOcean" in c ||
        "riverDownstreamCellId" in c ||
        "riverFlowNorm01" in c ||
        "depositEstimates" in c ||
        "lastLiveTick" in c ||
        "meanAnnualTempC" in c ||
        "annualPrecipMm" in c ||
        "aridityIndex01" in c ||
        "soilFertility01" in c ||
        "rockStrataCode" in c ||
        "drainageAreaKm2" in c ||
        "strahlerOrder" in c ||
        "geologyProvince" in c ||
        "roughnessM" in c ||
        "damStatus" in c
      ) {
        return false;
      }
    } else if (c.tier === "rumored") {
      if (
        typeof c.observedTick !== "number" ||
        typeof c.elevationM !== "number" ||
        typeof c.confidence01 !== "number"
      ) {
        return false;
      }
    } else if (c.tier === "stale") {
      if (
        typeof c.observedTick !== "number" ||
        typeof c.elevationM !== "number" ||
        typeof c.confidence01 !== "number"
      ) {
        return false;
      }
    } else if (c.tier === "surveyed") {
      if (
        typeof c.observedTick !== "number" ||
        typeof c.isLiveHqSight !== "boolean" ||
        typeof c.elevationM !== "number" ||
        typeof c.confidence01 !== "number"
      ) {
        return false;
      }
    } else {
      return false;
    }
  }
  return true;
}

const VALID_NOTIFICATION_BEHAVIORS = new Set<string>([
  "LogOnly",
  "BannerAlert",
  "DispatchPopup",
  "DispatchPopupAndAutoPause",
  "log_only",
  "banner_alert",
  "dispatch_popup",
  "dispatch_popup_and_auto_pause",
]);

const VALID_DAM_RULE_CURVES = new Set<string>([
  "balanced_power",
  "flood_control",
  "irrigation_supply",
  "BalancedPower",
  "FloodControl",
  "IrrigationSupply",
]);

function validateDamRuleCurve(raw: unknown): string | null {
  return typeof raw === "string" && VALID_DAM_RULE_CURVES.has(raw) ? raw : null;
}

/** Validates outbound `ValidatedPlayerCommand` payloads strictly before crossing IPC. */
export function validatePlayerCommand(value: unknown): ValidatedPlayerCommand | null {
  if (typeof value !== "object" || value === null) return null;
  const raw = value as Record<string, unknown>;
  const keys = Object.keys(raw);
  switch (raw.kind) {
    case "setPaused":
      if (typeof raw.paused !== "boolean" || keys.length !== 2) return null;
      return { kind: "setPaused", paused: raw.paused };
    case "stepSingleTick":
      if (keys.length !== 1) return null;
      return { kind: "stepSingleTick" };
    case "setSpeedMultiplier":
      if (
        typeof raw.multiplier !== "number" ||
        !Number.isInteger(raw.multiplier) ||
        !((raw.multiplier >= 1 && raw.multiplier <= 64) || raw.multiplier === 168) ||
        keys.length !== 2
      ) {
        return null;
      }
      return { kind: "setSpeedMultiplier", multiplier: raw.multiplier };
    case "pingBridge":
      if (typeof raw.tag !== "string" || raw.tag.length === 0 || raw.tag.length > 64 || keys.length !== 2) {
        return null;
      }
      return { kind: "pingBridge", tag: raw.tag };
    case "issueMandate":
      if (
        keys.length !== 6 ||
        typeof raw.assignee !== "number" ||
        !Number.isInteger(raw.assignee) ||
        raw.assignee < 0 ||
        typeof raw.objectiveKind !== "string" ||
        raw.objectiveKind.length === 0 ||
        raw.objectiveKind.length > 64 ||
        typeof raw.targetCell !== "number" ||
        !Number.isInteger(raw.targetCell) ||
        raw.targetCell < 0 ||
        typeof raw.priorityWeight !== "number" ||
        !Number.isInteger(raw.priorityWeight) ||
        raw.priorityWeight < 1 ||
        raw.priorityWeight > 10 ||
        typeof raw.authorityTier !== "string" ||
        raw.authorityTier.length === 0 ||
        raw.authorityTier.length > 64
      ) {
        return null;
      }
      return {
        kind: "issueMandate",
        assignee: raw.assignee,
        objectiveKind: raw.objectiveKind,
        targetCell: raw.targetCell,
        priorityWeight: raw.priorityWeight,
        authorityTier: raw.authorityTier,
      };
    case "resolveArbitrationProposal":
      if (
        keys.length !== 3 ||
        typeof raw.proposalId !== "number" ||
        !Number.isInteger(raw.proposalId) ||
        raw.proposalId < 1 ||
        typeof raw.chosenOptionIndex !== "number" ||
        !Number.isInteger(raw.chosenOptionIndex) ||
        raw.chosenOptionIndex < 0 ||
        raw.chosenOptionIndex > 16
      ) {
        return null;
      }
      return {
        kind: "resolveArbitrationProposal",
        proposalId: raw.proposalId,
        chosenOptionIndex: raw.chosenOptionIndex,
      };
    case "setPersonalAttentionZoom":
      if (
        keys.length !== 3 ||
        (raw.targetMandateId !== null &&
          (typeof raw.targetMandateId !== "number" ||
            !Number.isInteger(raw.targetMandateId) ||
            raw.targetMandateId < 1)) ||
        typeof raw.share01 !== "number" ||
        !Number.isFinite(raw.share01) ||
        raw.share01 < 0 ||
        raw.share01 > 1
      ) {
        return null;
      }
      return {
        kind: "setPersonalAttentionZoom",
        targetMandateId: raw.targetMandateId,
        share01: raw.share01,
      };
    case "relocateHq":
      if (
        keys.length !== 2 ||
        typeof raw.destinationCell !== "number" ||
        !Number.isInteger(raw.destinationCell) ||
        raw.destinationCell < 0
      ) {
        return null;
      }
      return {
        kind: "relocateHq",
        destinationCell: raw.destinationCell,
      };
    case "updateNotificationRule":
      if (
        keys.length !== 3 ||
        typeof raw.eventClassKey !== "string" ||
        raw.eventClassKey.length === 0 ||
        raw.eventClassKey.length > 64 ||
        typeof raw.behavior !== "string" ||
        !VALID_NOTIFICATION_BEHAVIORS.has(raw.behavior)
      ) {
        return null;
      }
      return {
        kind: "updateNotificationRule",
        eventClassKey: raw.eventClassKey,
        behavior: raw.behavior,
      };
    case "switchGovernanceModel":
      if (
        keys.length !== 2 ||
        typeof raw.modelKey !== "string" ||
        raw.modelKey.length === 0 ||
        raw.modelKey.length > 64
      ) {
        return null;
      }
      return {
        kind: "switchGovernanceModel",
        modelKey: raw.modelKey,
      };
    case "enactEconomicPolicy":
      if (
        (keys.length !== 2 && keys.length !== 3) ||
        typeof raw.policyKey !== "string" ||
        raw.policyKey.length === 0 ||
        raw.policyKey.length > 64
      ) {
        return null;
      }
      if (
        raw.rateOverride01 !== undefined &&
        raw.rateOverride01 !== null &&
        (typeof raw.rateOverride01 !== "number" ||
          !Number.isFinite(raw.rateOverride01) ||
          raw.rateOverride01 < 0 ||
          raw.rateOverride01 > 1)
      ) {
        return null;
      }
      return {
        kind: "enactEconomicPolicy",
        policyKey: raw.policyKey,
        rateOverride01: raw.rateOverride01 === undefined ? null : (raw.rateOverride01 as number | null),
      };
    case "charterPrivateCarriers":
      if (
        keys.length !== 3 ||
        typeof raw.targetSettlement !== "number" ||
        !Number.isInteger(raw.targetSettlement) ||
        raw.targetSettlement < 0 ||
        typeof raw.count !== "number" ||
        !Number.isInteger(raw.count) ||
        raw.count < 1 ||
        raw.count > 1000
      ) {
        return null;
      }
      return {
        kind: "charterPrivateCarriers",
        targetSettlement: raw.targetSettlement,
        count: raw.count,
      };
    case "setInnovationMode":
      if (
        keys.length !== 3 ||
        typeof raw.settlement !== "number" ||
        !Number.isInteger(raw.settlement) ||
        raw.settlement < 0 ||
        typeof raw.modeKey !== "string" ||
        raw.modeKey.length === 0 ||
        raw.modeKey.length > 64
      ) {
        return null;
      }
      return {
        kind: "setInnovationMode",
        settlement: raw.settlement,
        modeKey: raw.modeKey,
      };
    case "issueTheaterDirective":
      if (
        keys.length !== 5 ||
        typeof raw.theaterId !== "number" ||
        !Number.isInteger(raw.theaterId) ||
        raw.theaterId < 1 ||
        typeof raw.commander !== "number" ||
        !Number.isInteger(raw.commander) ||
        raw.commander < 0 ||
        typeof raw.objectiveCell !== "number" ||
        !Number.isInteger(raw.objectiveCell) ||
        raw.objectiveCell < 0 ||
        typeof raw.supplyPriority01 !== "number" ||
        !Number.isFinite(raw.supplyPriority01) ||
        raw.supplyPriority01 < 0 ||
        raw.supplyPriority01 > 1
      ) {
        return null;
      }
      return {
        kind: "issueTheaterDirective",
        theaterId: raw.theaterId,
        commander: raw.commander,
        objectiveCell: raw.objectiveCell,
        supplyPriority01: raw.supplyPriority01,
      };
    case "issueOperationalBattlePlan":
      if (
        keys.length !== 5 ||
        typeof raw.formationId !== "number" ||
        !Number.isInteger(raw.formationId) ||
        raw.formationId < 1 ||
        typeof raw.targetCell !== "number" ||
        !Number.isInteger(raw.targetCell) ||
        raw.targetCell < 0 ||
        typeof raw.doctrineKey !== "string" ||
        raw.doctrineKey.length === 0 ||
        raw.doctrineKey.length > 64 ||
        typeof raw.noRetreat !== "boolean"
      ) {
        return null;
      }
      return {
        kind: "issueOperationalBattlePlan",
        formationId: raw.formationId,
        targetCell: raw.targetCell,
        doctrineKey: raw.doctrineKey,
        noRetreat: raw.noRetreat,
      };
    case "acknowledgeSuccession":
      if (keys.length !== 1) {
        return null;
      }
      return { kind: "acknowledgeSuccession" };
    case "constructDam": {
      if (
        (keys.length !== 2 && keys.length !== 3) ||
        typeof raw.targetCell !== "number" ||
        !Number.isInteger(raw.targetCell) ||
        raw.targetCell < 0
      ) {
        return null;
      }
      const curve = raw.ruleCurve === undefined ? "balanced_power" : validateDamRuleCurve(raw.ruleCurve);
      if (!curve) return null;
      return { kind: "constructDam", targetCell: raw.targetCell, ruleCurve: curve };
    }
    case "setDamRuleCurve": {
      const curve = validateDamRuleCurve(raw.ruleCurve);
      if (
        keys.length !== 3 ||
        typeof raw.targetCell !== "number" ||
        !Number.isInteger(raw.targetCell) ||
        raw.targetCell < 0 ||
        !curve
      ) {
        return null;
      }
      return { kind: "setDamRuleCurve", targetCell: raw.targetCell, ruleCurve: curve };
    }
    default:
      return null;
  }
}
