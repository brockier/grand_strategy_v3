import {
  AudioMixerSettings,
  DiegeticSnapshot,
  GlobeMapLabelAnchor,
  GlobeProjectionMode,
  GlobeRayHit,
  UserPreferencesDto,
} from "./contracts";
import { UiIntentBus } from "./intent-bus";
import { DiegeticStoreAccess } from "./store";

export interface GlobeLayerHost {
  readonly overlayRootId: string;
  getCanvasElement(): HTMLCanvasElement;
  pickGlobeAtNdc(ndcX: number, ndcY: number): GlobeRayHit | null;
  isPointOccludedByChrome(clientX: number, clientY: number): boolean;
  projectUnitToScreen?(unit: readonly [number, number, number]): {
    readonly xPx: number;
    readonly yPx: number;
    readonly visible: boolean;
  };
  getMapLabelAnchors?(): readonly GlobeMapLabelAnchor[];
  getProjectionMode?(): GlobeProjectionMode;
  setProjectionMode?(mode: GlobeProjectionMode): void;
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

export interface PortraitLookupQuery {
  readonly characterId: number;
  readonly ageBracket?: string;
  readonly vocation?: string;
  readonly eraTag?: string;
  readonly healthBand?: string;
}

export interface AssetUrlResolverPort {
  resolvePortrait(query: PortraitLookupQuery): string;
  resolveIcon(key: string): string;
  resolveDispatchPlate(key: string): string;
  getAvailableSkinIds(): readonly string[];
  getActiveSkinId(): string;
}

export interface AudioPreferencesPort {
  getSettings(): AudioMixerSettings;
  updateSettings(patch: Partial<AudioMixerSettings>): void;
  getEffectiveGains(): {
    readonly master: number;
    readonly music: number;
    readonly ambience: number;
    readonly foley: number;
    readonly surfaceEnv: number;
    readonly orbitalEnv: number;
  };
}

export interface LayoutPresetPort {
  listPresetNames(): readonly string[];
  exportCurrentLayoutJson(): string;
  saveNamedPreset(name: string): void;
  applyNamedPreset(name: string): boolean;
}

export interface UiSpineContext {
  readonly store: DiegeticStoreAccess;
  readonly bus: UiIntentBus;
  readonly assetResolver?: AssetUrlResolverPort;
  readonly audioPrefs?: AudioPreferencesPort;
  readonly layoutPresets?: LayoutPresetPort;
  readonly readPreferences?: () => Promise<UserPreferencesDto>;
  readonly writePreferences?: (prefs: UserPreferencesDto) => Promise<void>;
}

export interface WindowContentPlugin {
  readonly pluginId: string;
  readonly defaultTitle: string;
  readonly minWidth: number;
  readonly minHeight: number;
  readonly maxWidth?: number;
  readonly maxHeight?: number;
  readonly hotkey?: string;
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

  public listWindowPlugins(): readonly WindowContentPlugin[] {
    return Array.from(this.windowPlugins.values());
  }
}
