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
  | { readonly hitKind: "convoy"; readonly cellId: number; readonly convoyId: number; readonly label: string }
  | { readonly hitKind: "settlement"; readonly cellId: number; readonly settlementId: number; readonly label: string }
  | { readonly hitKind: "road"; readonly cellId: number; readonly edgeId: number; readonly fromCellId: number; readonly toCellId: number; readonly label: string }
  | { readonly hitKind: "river"; readonly cellId: number; readonly toCellId: number; readonly flowNorm01: number; readonly label: string }
  | { readonly hitKind: "cell"; readonly cellId: number; readonly label: string };

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
        "lastLiveTick" in c
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
    default:
      return null;
  }
}
