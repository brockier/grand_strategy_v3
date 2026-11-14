import {
  CommandAck,
  DiegeticSnapshot,
  InitialBootPayload,
  SimBridgeApi,
  SkinDescriptor,
  SkinManifestBundle,
  ValidatedPlayerCommand,
  isValidDiegeticSnapshot,
  validatePlayerCommand,
} from "./contracts";

/**
 * Deterministic offline fixture player implementing `SimBridgeApi` (`UI-SHL-005`).
 * Executes Vitest and Playwright harnesses cleanly without spawning any Rust backend processes.
 */
export class MockSnapshotPlayer implements SimBridgeApi {
  private readonly timeline: DiegeticSnapshot[] = [];
  private cursor = 0;
  private readonly listeners = new Set<(snapshot: DiegeticSnapshot) => void>();
  public readonly recordedCommands: ValidatedPlayerCommand[] = [];

  constructor(initialSnapshots: readonly DiegeticSnapshot[] = []) {
    for (const snap of initialSnapshots) {
      if (!isValidDiegeticSnapshot(snap)) {
        throw new Error("Invalid snapshot passed to MockSnapshotPlayer");
      }
      this.timeline.push(snap);
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
    const currentTick = this.timeline[this.cursor]?.clockAndNotifications.tick ?? 0;
    if (!validated) {
      return {
        accepted: false,
        tick: currentTick,
        errorCode: "INVALID_COMMAND",
        message: "Rejected malformed player command payload",
      };
    }
    this.recordedCommands.push(validated);
    return {
      accepted: true,
      tick: currentTick,
      errorCode: null,
      message: `Recorded ${validated.kind}`,
    };
  }

  public async reloadMods(_recoverToDefault: boolean): Promise<InitialBootPayload> {
    return this.getInitialState();
  }

  public async listAvailableSkins(): Promise<readonly SkinDescriptor[]> {
    return [
      {
        skinId: "cartographer-parchment",
        displayName: "Cartographer's Parchment",
        stylesheetPath: "theme.css",
      },
    ];
  }

  public async readSkinAssetManifest(skinId: string): Promise<SkinManifestBundle> {
    return {
      skinId,
      displayName: "Cartographer's Parchment",
      stylesheetHref: `../../skins/${skinId}/theme.css`,
      cssText: ":root { --skin-bg-panel: #f0e4cc; --skin-text-primary: #231c15; }",
      globePalette: {
        oceanShallowHex: "#87a3ad",
        oceanDeepHex: "#3d5966",
        landLowHex: "#d8caa6",
        landHighHex: "#8c7258",
        contourInkHex: "#3b2c1d",
        unexploredVellumHex: "#e6d8b9",
        parchmentGrainTexture: "textures/parchment_tile.png",
        hatchPatternTexture: "textures/ink_hatch_tile.png",
      },
    };
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
