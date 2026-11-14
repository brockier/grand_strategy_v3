import { ValidatedPlayerCommand } from "./contracts";

export type EntityKind = "settlement" | "character" | "convoy" | "facility" | "cell";

export type UiIntent =
  | { readonly kind: "FlyGlobeToCell"; readonly cellId: number; readonly zoomAltitude?: number }
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
  | { readonly kind: "PreviewHqLatencyRoute"; readonly destinationCellId: number | null }
  | { readonly kind: "SwitchActiveSkin"; readonly skinId: string }
  | { readonly kind: "PlayUiFoley"; readonly cueId: string }
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
