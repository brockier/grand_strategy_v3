import { DiegeticSnapshot } from "./contracts";
import { UiIntentBus } from "./intent-bus";
import { DiegeticStoreAccess } from "./store";

export interface GlobeLayerHost {
  readonly overlayRootId: string;
}

export interface MapLayerPlugin {
  readonly layerId: string;
  readonly displayName: string;
  readonly supportsMode: (modeId: string) => boolean;
  mount(host: GlobeLayerHost, store: DiegeticStoreAccess, bus: UiIntentBus): void;
  onSnapshotUpdate(snapshot: DiegeticSnapshot): void;
  onFrameRender(cameraAltitudeNorm: number, elapsedSec: number): void;
  dispose(): void;
}

export interface UiSpineContext {
  readonly store: DiegeticStoreAccess;
  readonly bus: UiIntentBus;
}

export interface WindowContentPlugin {
  readonly pluginId: string;
  readonly defaultTitle: string;
  readonly minWidth: number;
  readonly minHeight: number;
  readonly maxWidth?: number;
  readonly maxHeight?: number;
  mountContent(container: HTMLElement, instanceKey: string, spine: UiSpineContext): () => void;
}

/** Central registry for pluggable map overlays and window plugins. */
export class PluginRegistry {
  private readonly mapLayers = new Map<string, MapLayerPlugin>();
  private readonly windowPlugins = new Map<string, WindowContentPlugin>();

  public registerMapLayer(plugin: MapLayerPlugin): void {
    this.mapLayers.set(plugin.layerId, plugin);
  }

  public getMapLayers(): readonly MapLayerPlugin[] {
    return Array.from(this.mapLayers.values());
  }

  public registerWindowPlugin(plugin: WindowContentPlugin): void {
    this.windowPlugins.set(plugin.pluginId, plugin);
  }

  public getWindowPlugin(pluginId: string): WindowContentPlugin | undefined {
    return this.windowPlugins.get(pluginId);
  }
}
