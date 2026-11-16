import {
  CameraTelemetry,
  GlobeProjectionMode,
  GlobeRayHit,
  SkinManifestBundle,
  ValidatedPlayerCommand,
} from "./contracts";

export type EntityKind = "settlement" | "character" | "convoy" | "facility" | "cell" | "formation";

export type UiIntent =
  | { readonly kind: "FlyGlobeToCell"; readonly cellId: number; readonly zoomAltitude?: number }
  | { readonly kind: "ResetGlobeNorth" }
  | {
      readonly kind: "FocusEntityInspector";
      readonly entityKind: EntityKind;
      readonly entityId: number;
      readonly pinned?: boolean;
    }
  | {
      readonly kind: "OpenSystemWindow";
      readonly pluginId: string;
      readonly instanceKey?: string;
      readonly pinned?: boolean;
    }
  | { readonly kind: "CloseWindow"; readonly windowId: string }
  | { readonly kind: "SwitchMapOverlayMode"; readonly modeId: string }
  | { readonly kind: "SwitchGlobeProjection"; readonly projection: GlobeProjectionMode }
  | { readonly kind: "PreviewHqLatencyRoute"; readonly destinationCellId: number | null }
  | { readonly kind: "SwitchActiveSkin"; readonly skinId: string }
  | { readonly kind: "SkinBundleUpdated"; readonly bundle: SkinManifestBundle }
  | { readonly kind: "PlayUiFoley"; readonly cueId: string }
  | { readonly kind: "UpdateCameraTelemetry"; readonly telemetry: CameraTelemetry }
  | {
      readonly kind: "HoverMapFeature";
      readonly hit: GlobeRayHit | null;
      readonly clientX: number;
      readonly clientY: number;
    }
  | { readonly kind: "SelectMapFeature"; readonly hit: GlobeRayHit }
  | { readonly kind: "ApplyWindowLayoutPreset"; readonly presetName: string }
  | { readonly kind: "SaveWindowLayoutPreset"; readonly presetName: string }
  | { readonly kind: "DispatchSimCommand"; readonly command: ValidatedPlayerCommand };

export type UiIntentHandler = (intent: UiIntent) => void;

/** Decoupled event bus connecting UI cartridges cleanly without lateral imports. */
export class UiIntentBus {
  private readonly handlers = new Set<UiIntentHandler>();

  public subscribe(handler: UiIntentHandler): () => void {
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
    };
  }

  public emit(intent: UiIntent): void {
    for (const handler of this.handlers) {
      handler(intent);
    }
  }
}
