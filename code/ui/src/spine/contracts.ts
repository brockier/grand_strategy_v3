/**
 * Authoritative UI Spine contracts mirroring `player-view` IPC DTOs (`UI-SPINE-00`, `UI-SHL-002`).
 * Pure TypeScript definitions and runtime schema validators with zero sibling or Electron/Three imports.
 */

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

export type KnownCellDto =
  | {
      readonly tier: "unexplored";
      readonly cellId: number;
    }
  | {
      readonly tier: "surveyed";
      readonly cellId: number;
      readonly observedTick: number;
      readonly isLiveHqSight: boolean;
      readonly elevationM: number;
      readonly confidence01: number;
      readonly provenanceLabel: string;
    };

export interface HqPresenceDto {
  readonly mode: string;
  readonly cellId: number;
  readonly settlementName: string;
  readonly sightRadiusCells: number;
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
}

export interface SubordinateSummaryDto {
  readonly characterId: number;
  readonly displayName: string;
  readonly roleTitle: string;
  readonly competence: number;
}

export interface DelegationTreeDto {
  readonly leaderTitle: string;
  readonly capacityAttention: number;
  readonly usedAttention: number;
  readonly subordinates: readonly SubordinateSummaryDto[];
}

export interface TrackedConvoyDto {
  readonly convoyId: number;
  readonly label: string;
  readonly status: string;
  readonly projectedCellId: number;
}

export interface DiegeticDispatchDto {
  readonly dispatchId: number;
  readonly title: string;
  readonly sentTick: number;
  readonly arrivedTick: number;
  readonly originLabel: string;
}

export interface ClockAndNotificationsDto {
  readonly tick: number;
  readonly calendarText: string;
  readonly paused: boolean;
  readonly speedMultiplier: number;
  readonly activeAlertCount: number;
}

export interface DiegeticSnapshot {
  readonly schemaVersion: number;
  readonly scenarioKey: string;
  readonly scenarioName: string;
  readonly globeTopology: GlobeTopologyDto;
  readonly knownCells: readonly KnownCellDto[];
  readonly hqPresence: HqPresenceDto;
  readonly trackedConvoys: readonly TrackedConvoyDto[];
  readonly observedWeather: readonly ObservedWeatherDto[];
  readonly ledgerRows: readonly ReportedLedgerRowDto[];
  readonly delegationTree: DelegationTreeDto;
  readonly dispatchInbox: readonly DiegeticDispatchDto[];
  readonly clockAndNotifications: ClockAndNotificationsDto;
}

export type ValidatedPlayerCommand =
  | { readonly kind: "setPaused"; readonly paused: boolean }
  | { readonly kind: "stepSingleTick" }
  | { readonly kind: "setSpeedMultiplier"; readonly multiplier: number }
  | { readonly kind: "pingBridge"; readonly tag: string };

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
  readonly oceanShallowHex: string;
  readonly oceanDeepHex: string;
  readonly landLowHex: string;
  readonly landHighHex: string;
  readonly contourInkHex: string;
  readonly unexploredVellumHex: string;
  readonly parchmentGrainTexture: string;
  readonly hatchPatternTexture: string;
}

export interface SkinManifestBundle {
  readonly skinId: string;
  readonly displayName: string;
  readonly stylesheetHref: string;
  readonly cssText: string;
  readonly globePalette: GlobePalette;
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
}

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
      // Epistemic air-gap invariant: Unexplored cell payloads omit terrain/mineral properties.
      if ("elevationM" in c || "confidence01" in c || "observedTick" in c) {
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
        raw.multiplier < 1 ||
        raw.multiplier > 64 ||
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
    default:
      return null;
  }
}
