import {
  CommandAck,
  DEFAULT_AUDIO_MIXER_SETTINGS,
  DiegeticSnapshot,
  GlobePalette,
  InitialBootPayload,
  SimBridgeApi,
  SkinDescriptor,
  SkinManifestBundle,
  UserPreferencesDto,
  ValidatedPlayerCommand,
  isValidDiegeticSnapshot,
  validatePlayerCommand,
} from "./contracts";

const NEUTRAL_TEST_PALETTE: GlobePalette = {
  oceanShallowRgb: [135, 163, 173],
  oceanDeepRgb: [61, 89, 102],
  landLowRgb: [115, 138, 87],
  landMidRgb: [176, 162, 116],
  landHighRgb: [140, 114, 88],
  peakSnowRgb: [244, 239, 228],
  riverWaterRgb: [54, 108, 133],
  roadOchreRgb: [166, 89, 40],
  settlementMarkerRgb: [139, 46, 34],
  contourInkRgb: [59, 44, 29],
  compassRoseRgb: [110, 85, 57],
  unexploredVellumRgb: [230, 216, 185],
  parchmentGrainTextureUrl: "",
  hatchPatternTextureUrl: "",
  terrainBiomeTextureUrl: "",
};

/**
 * Deterministic offline fixture player implementing `SimBridgeApi` (`UI-SHL-005`).
 * Executes Vitest and Playwright harnesses cleanly without spawning any Rust backend processes.
 */
export class MockSnapshotPlayer implements SimBridgeApi {
  private readonly timeline: DiegeticSnapshot[] = [];
  private cursor = 0;
  private readonly listeners = new Set<(snapshot: DiegeticSnapshot) => void>();
  private readonly hotReloadListeners = new Set<(skinId: string) => void>();
  private readonly skinBundles = new Map<string, SkinManifestBundle>();
  private prefs: UserPreferencesDto = {
    audioMixer: { ...DEFAULT_AUDIO_MIXER_SETTINGS },
    namedLayoutPresets: {},
  };
  public readonly recordedCommands: ValidatedPlayerCommand[] = [];

  constructor(
    initialSnapshots: readonly DiegeticSnapshot[] = [],
    initialSkins: readonly SkinManifestBundle[] = [],
  ) {
    for (const snap of initialSnapshots) {
      if (!isValidDiegeticSnapshot(snap)) {
        throw new Error("Invalid snapshot passed to MockSnapshotPlayer");
      }
      this.timeline.push(snap);
    }
    for (const bundle of initialSkins) {
      this.skinBundles.set(bundle.skinId, bundle);
    }
    if (this.skinBundles.size === 0) {
      this.skinBundles.set("cartographer-parchment", {
        skinId: "cartographer-parchment",
        displayName: "Cartographer's Parchment",
        stylesheetHref: "../../skins/cartographer-parchment/theme.css",
        cssText: "",
        globePalette: NEUTRAL_TEST_PALETTE,
      });
    }
  }

  public registerMockSkin(bundle: SkinManifestBundle): void {
    this.skinBundles.set(bundle.skinId, bundle);
  }

  public emitHotReload(skinId: string): void {
    for (const listener of this.hotReloadListeners) {
      listener(skinId);
    }
  }

  public async getInitialState(): Promise<InitialBootPayload> {
    if (this.timeline.length === 0) {
      return {
        state: "mod_error",
        failure: {
          ok: false,
          diagnostics: [
            {
              severity: "Error",
              layer: "MockFixture",
              filePath: "tests/fixtures/empty.json",
              line: 1,
              column: 1,
              fieldPath: "timeline",
              message: "No mock snapshot loaded in MockSnapshotPlayer",
            },
          ],
        },
      };
    }
    return { state: "ready", snapshot: this.timeline[this.cursor] };
  }

  public subscribeSnapshots(listener: (snapshot: DiegeticSnapshot) => void): () => void {
    this.listeners.add(listener);
    if (this.timeline.length > 0) {
      listener(this.timeline[this.cursor]);
    }
    return () => {
      this.listeners.delete(listener);
    };
  }

  public stepMockSnapshot(index: number): DiegeticSnapshot {
    if (index < 0 || index >= this.timeline.length) {
      throw new RangeError(`Snapshot index ${index} out of bounds (len=${this.timeline.length})`);
    }
    this.cursor = index;
    const snap = this.timeline[this.cursor];
    for (const listener of this.listeners) {
      listener(snap);
    }
    return snap;
  }

  public async sendPlayerCommand(command: ValidatedPlayerCommand): Promise<CommandAck> {
    const validated = validatePlayerCommand(command);
    const active = this.timeline[this.cursor];
    const currentTick = active?.clockAndNotifications.tick ?? 0;
    if (!validated) {
      return {
        accepted: false,
        tick: currentTick,
        errorCode: "INVALID_COMMAND",
        message: "Rejected malformed player command payload",
      };
    }
    this.recordedCommands.push(validated);
    if (active) {
      const next = this.applyCommandToSnapshot(active, validated);
      this.timeline[this.cursor] = next;
      for (const listener of this.listeners) {
        listener(next);
      }
    }
    return {
      accepted: true,
      tick: currentTick,
      errorCode: null,
      message: `Recorded ${validated.kind}`,
    };
  }

  private applyCommandToSnapshot(
    snap: DiegeticSnapshot,
    cmd: ValidatedPlayerCommand,
  ): DiegeticSnapshot {
    const next = structuredClone(snap) as DiegeticSnapshot & {
      clockAndNotifications: { paused: boolean; speedMultiplier: number; autoPauseReason?: string | null };
      delegationTree: {
        focusedMandateId?: number | null;
        focusedAttentionShare?: number;
        usedAttention: number;
        activeMandates?: Array<{
          mandateId: number;
          assigneeId: number;
          targetSettlementId: number;
          targetCellId: number;
          objectiveKind: string;
          authorityTier: string;
          summary: string;
          priorityWeight: number;
          status: string;
          realizedEfficiency: number;
          progress01: number;
          subtasks: string[];
        }>;
      };
      hqPresence: {
        inTransit?: boolean;
        destinationCellId?: number | null;
        etaTick?: number | null;
      };
      openProposals?: Array<{ proposalId: number }>;
      notificationRules?: Array<{ eventClassKey: string; displayName: string; behavior: string }>;
    };

    switch (cmd.kind) {
      case "setPaused":
        next.clockAndNotifications.paused = cmd.paused;
        if (!cmd.paused) next.clockAndNotifications.autoPauseReason = null;
        break;
      case "setSpeedMultiplier":
        next.clockAndNotifications.speedMultiplier = cmd.multiplier;
        break;
      case "setPersonalAttentionZoom":
        next.delegationTree.focusedMandateId = cmd.targetMandateId;
        next.delegationTree.focusedAttentionShare = cmd.share01;
        break;
      case "issueMandate": {
        const list = next.delegationTree.activeMandates ?? [];
        const newId = list.reduce((m, item) => Math.max(m, item.mandateId), 0) + 1;
        list.push({
          mandateId: newId,
          assigneeId: cmd.assignee,
          targetSettlementId: 0,
          targetCellId: cmd.targetCell,
          objectiveKind: cmd.objectiveKind,
          authorityTier: cmd.authorityTier,
          summary: `${cmd.objectiveKind} (${cmd.authorityTier}) -> cell ${cmd.targetCell}`,
          priorityWeight: cmd.priorityWeight,
          status: "in_transit_courier",
          realizedEfficiency: 0.75,
          progress01: 0,
          subtasks: ["Courier dispatch in transit"],
        });
        next.delegationTree.activeMandates = list;
        next.delegationTree.usedAttention += 10;
        break;
      }
      case "resolveArbitrationProposal":
        if (next.openProposals) {
          next.openProposals = next.openProposals.filter((p) => p.proposalId !== cmd.proposalId);
        }
        break;
      case "relocateHq":
        next.hqPresence.inTransit = true;
        next.hqPresence.destinationCellId = cmd.destinationCell;
        next.hqPresence.etaTick = next.clockAndNotifications.tick + 12;
        break;
      case "updateNotificationRule": {
        const rules = next.notificationRules ?? [];
        const existing = rules.find((r) => r.eventClassKey === cmd.eventClassKey);
        if (existing) {
          existing.behavior = cmd.behavior;
        } else {
          rules.push({
            eventClassKey: cmd.eventClassKey,
            displayName: cmd.eventClassKey,
            behavior: cmd.behavior,
          });
        }
        next.notificationRules = rules;
        break;
      }
      default:
        break;
    }
    return next;
  }

  public async reloadMods(_recoverToDefault: boolean): Promise<InitialBootPayload> {
    return this.getInitialState();
  }

  public async listAvailableSkins(): Promise<readonly SkinDescriptor[]> {
    return Array.from(this.skinBundles.values()).map((b) => ({
      skinId: b.skinId,
      displayName: b.displayName,
      stylesheetPath: b.stylesheetHref,
    }));
  }

  public async readSkinAssetManifest(skinId: string): Promise<SkinManifestBundle> {
    const found = this.skinBundles.get(skinId);
    if (found) return found;
    const first = Array.from(this.skinBundles.values())[0];
    return {
      ...first,
      skinId,
      displayName: skinId,
    };
  }

  public subscribeSkinHotReload(listener: (skinId: string) => void): () => void {
    this.hotReloadListeners.add(listener);
    return () => {
      this.hotReloadListeners.delete(listener);
    };
  }

  public async readUserPreferences(): Promise<UserPreferencesDto> {
    return structuredClone(this.prefs);
  }

  public async writeUserPreferences(next: UserPreferencesDto): Promise<void> {
    this.prefs = structuredClone(next);
  }

  public async loadMockFixture(fixtureJson: string): Promise<void> {
    const parsed: unknown = JSON.parse(fixtureJson);
    const items = Array.isArray(parsed) ? parsed : [parsed];
    this.timeline.length = 0;
    for (const item of items) {
      if (!isValidDiegeticSnapshot(item)) {
        throw new Error("Mock fixture failed DiegeticSnapshot schema validation");
      }
      this.timeline.push(item);
    }
    this.cursor = 0;
    const active = this.timeline[0];
    if (active) {
      for (const listener of this.listeners) {
        listener(active);
      }
    }
  }
}
