import {
  DEFAULT_AUDIO_MIXER_SETTINGS,
  DiegeticStore,
  InitialBootPayload,
  PluginRegistry,
  SimBridgeApi,
  UiIntentBus,
  UserPreferencesDto,
  ValidatedPlayerCommand,
} from "./spine";
import { initializeGlobeViewport, GlobeViewportController } from "./globe";
import {
  EpistemicCurrencyOverlayLayer,
  LogisticsConvoyOverlayLayer,
  OcclusionAwareCellPicker,
  OverlayToolbarLayer,
  WeatherHydrologyOverlayLayer,
  createBaselineOverlayPlugin,
  createOperationalWarfareStubLayer,
  createTradeAndCongestionStubLayer,
} from "./overlays";
import {
  DesktopWindowManager,
  ModDiagnosticDialog,
  createPlaceholderWindowPlugin,
} from "./windows";
import {
  createDelegationOrgTreeWindowPlugin,
  createEntityInspectorWindowPlugin,
  createLayoutPresetsWindowPlugin,
  createLedgerStubPlugin,
  createTimeSeriesChartWindowPlugin,
} from "./views";
import { createDispatchInboxStubPlugin } from "./dispatches";
import {
  createArbitrationWindowPlugin,
  createAttentionZoomWindowPlugin,
  createHqRelocationPlannerWindowPlugin,
  createMandateComposerWindowPlugin,
  createNotificationMatrixWindowPlugin,
  mountCommandHud,
} from "./hud";
import { SkinManager, createSkinSwitcherWindowPlugin } from "./skins";
import { AudioMixerEngine, createAudioSettingsWindowPlugin } from "./audio";

export interface BootStatusReport {
  ready: boolean;
  mode: "live" | "mod_error" | "booting";
  gpuBackend: "webgpu" | "webgl2" | "noop";
  shaderCompiled: boolean;
  diagnosticCount: number;
  hasNodeRequire: boolean;
  hasProcessGlobal: boolean;
  hasSimBridge: boolean;
}

export interface M2RuntimeHandle {
  readonly store: DiegeticStore;
  readonly bus: UiIntentBus;
  readonly registry: PluginRegistry;
  readonly skinManager: SkinManager;
  readonly audioEngine: AudioMixerEngine;
  readonly windowManager: DesktopWindowManager;
  readonly globe: GlobeViewportController;
}

declare global {
  interface Window {
    simBridge?: SimBridgeApi;
    __ngBootStatus?: BootStatusReport;
    __ngTriggerRecoverDefault?: () => Promise<InitialBootPayload>;
    __ngSendTestCommand?: (cmd: unknown) => Promise<unknown>;
    __ngM2Runtime?: M2RuntimeHandle;
  }
}

async function bootstrapShell(): Promise<void> {
  const status: BootStatusReport = {
    ready: false,
    mode: "booting",
    gpuBackend: "noop",
    shaderCompiled: false,
    diagnosticCount: 0,
    hasNodeRequire: typeof (window as unknown as Record<string, unknown>).require !== "undefined",
    hasProcessGlobal: typeof (window as unknown as Record<string, unknown>).process !== "undefined",
    hasSimBridge: typeof window.simBridge === "object" && window.simBridge !== null,
  };
  window.__ngBootStatus = status;

  const bridge = window.simBridge;
  const appRoot = document.getElementById("app-shell");
  const canvas = document.getElementById("globe-canvas") as HTMLCanvasElement | null;
  if (!appRoot || !canvas || !bridge) {
    return;
  }

  let initialPrefs: UserPreferencesDto = {
    audioMixer: { ...DEFAULT_AUDIO_MIXER_SETTINGS },
    namedLayoutPresets: {},
  };
  if (bridge.readUserPreferences) {
    try {
      initialPrefs = await bridge.readUserPreferences();
    } catch {
      // Retain baseline defaults cleanly
    }
  }

  const store = new DiegeticStore();
  const bus = new UiIntentBus();
  const registry = new PluginRegistry();

  const skinManager = new SkinManager(bridge, bus);
  const audioEngine = new AudioMixerEngine(bus, {
    store,
    bridge,
    initialSettings: initialPrefs.audioMixer,
  });

  registry.registerMapLayer(createBaselineOverlayPlugin());
  registry.registerMapLayer(new EpistemicCurrencyOverlayLayer());
  registry.registerMapLayer(new WeatherHydrologyOverlayLayer());
  registry.registerMapLayer(new LogisticsConvoyOverlayLayer());
  registry.registerMapLayer(new OverlayToolbarLayer());
  registry.registerMapLayer(new OcclusionAwareCellPicker());
  registry.registerMapLayer(createTradeAndCongestionStubLayer());
  registry.registerMapLayer(createOperationalWarfareStubLayer());

  registry.registerWindowPlugin(createPlaceholderWindowPlugin());
  registry.registerWindowPlugin(createLedgerStubPlugin());
  registry.registerWindowPlugin(createTimeSeriesChartWindowPlugin());
  registry.registerWindowPlugin(createDelegationOrgTreeWindowPlugin());
  registry.registerWindowPlugin(createEntityInspectorWindowPlugin());
  registry.registerWindowPlugin(createDispatchInboxStubPlugin());
  registry.registerWindowPlugin(createMandateComposerWindowPlugin());
  registry.registerWindowPlugin(createArbitrationWindowPlugin());
  registry.registerWindowPlugin(createAttentionZoomWindowPlugin());
  registry.registerWindowPlugin(createHqRelocationPlannerWindowPlugin());
  registry.registerWindowPlugin(createNotificationMatrixWindowPlugin());
  registry.registerWindowPlugin(createSkinSwitcherWindowPlugin(skinManager));
  registry.registerWindowPlugin(createAudioSettingsWindowPlugin(audioEngine));
  registry.registerWindowPlugin(createLayoutPresetsWindowPlugin());

  const windowManager = new DesktopWindowManager(
    appRoot,
    registry,
    {
      store,
      bus,
      assetResolver: skinManager.assetResolver,
      audioPrefs: audioEngine,
      readPreferences: bridge.readUserPreferences,
      writePreferences: bridge.writeUserPreferences,
    },
    {
      viewportWidth: window.innerWidth || 1280,
      viewportHeight: window.innerHeight || 800,
      initialPresets: initialPrefs.namedLayoutPresets,
      onPersist(activeLayoutJson, presets) {
        const writePrefs = bridge.writeUserPreferences;
        if (!writePrefs) return;
        void (async () => {
          try {
            const latest = bridge.readUserPreferences
              ? await bridge.readUserPreferences()
              : initialPrefs;
            await writePrefs({
              ...latest,
              activeLayoutJson,
              namedLayoutPresets: presets,
            });
          } catch {
            // Non-fatal persistence guard
          }
        })();
      },
    },
  );

  try {
    await skinManager.initialize(initialPrefs.activeSkinId);
  } catch {
    // Retain CSS defaults gracefully if skin read fails
  }

  mountCommandHud(appRoot, store, bus);

  const globe = await initializeGlobeViewport(canvas, store, bus, registry.getMapLayers(), {
    initialPalette: skinManager.getActiveBundle()?.globePalette,
    isPointOccluded: (clientX, clientY) => windowManager.isPointOccluded(clientX, clientY),
  });
  status.gpuBackend = globe.status.backend;
  status.shaderCompiled = globe.status.shaderCompiled;

  if (initialPrefs.activeLayoutJson) {
    windowManager.restoreFromJson(initialPrefs.activeLayoutJson);
  }

  window.addEventListener("resize", () => {
    const w = window.innerWidth || 1280;
    const h = window.innerHeight || 800;
    globe.resize(w, h);
    windowManager.handleViewportResize(w, h);
  });

  bus.subscribe((intent) => {
    if (intent.kind === "DispatchSimCommand") {
      void bridge.sendPlayerCommand(intent.command);
    }
  });

  bridge.subscribeSnapshots((snapshot) => {
    store.ingestSnapshot(snapshot);
    globe.updateFromSnapshot(snapshot);
  });

  const handlePayload = (payload: InitialBootPayload, dialog: ModDiagnosticDialog) => {
    if (payload.state === "ready") {
      dialog.hide();
      store.ingestSnapshot(payload.snapshot);
      globe.updateFromSnapshot(payload.snapshot);
      status.mode = "live";
      status.diagnosticCount = 0;
      status.ready = true;
    } else {
      status.mode = "mod_error";
      status.diagnosticCount = payload.failure.diagnostics.length;
      dialog.showDiagnostics(payload.failure);
      status.ready = true;
    }
  };

  const diagDialog = new ModDiagnosticDialog(appRoot, {
    async onReloadMods() {
      const next = await bridge.reloadMods(false);
      handlePayload(next, diagDialog);
    },
    async onRevertDefault() {
      const next = await bridge.reloadMods(true);
      handlePayload(next, diagDialog);
    },
  });

  window.__ngTriggerRecoverDefault = async () => {
    const next = await bridge.reloadMods(true);
    handlePayload(next, diagDialog);
    return next;
  };

  window.__ngSendTestCommand = async (cmd: unknown) => {
    return bridge.sendPlayerCommand(cmd as ValidatedPlayerCommand);
  };

  window.__ngM2Runtime = {
    store,
    bus,
    registry,
    skinManager,
    audioEngine,
    windowManager,
    globe,
  };

  const initial = await bridge.getInitialState();
  handlePayload(initial, diagDialog);
}

void bootstrapShell();
