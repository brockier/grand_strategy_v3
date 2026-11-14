import {
  DiegeticStore,
  InitialBootPayload,
  PluginRegistry,
  SimBridgeApi,
  UiIntentBus,
  ValidatedPlayerCommand,
} from "./spine";
import { initializeGlobeViewport } from "./globe";
import { createBaselineOverlayPlugin } from "./overlays";
import { ModDiagnosticDialog, createPlaceholderWindowPlugin } from "./windows";
import { createLedgerStubPlugin } from "./views";
import { createDispatchInboxStubPlugin } from "./dispatches";
import { mountCommandHud } from "./hud";
import { applySkinBundle } from "./skins";
import { mountAudioController } from "./audio";

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

declare global {
  interface Window {
    simBridge?: SimBridgeApi;
    __ngBootStatus?: BootStatusReport;
    __ngTriggerRecoverDefault?: () => Promise<InitialBootPayload>;
    __ngSendTestCommand?: (cmd: unknown) => Promise<unknown>;
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

  try {
    const skins = await bridge.listAvailableSkins();
    if (skins.length > 0) {
      const bundle = await bridge.readSkinAssetManifest(skins[0].skinId);
      applySkinBundle(bundle);
    }
  } catch {
    // Default styling fallback handled gracefully
  }

  const store = new DiegeticStore();
  const bus = new UiIntentBus();
  const registry = new PluginRegistry();
  registry.registerMapLayer(createBaselineOverlayPlugin());
  registry.registerWindowPlugin(createPlaceholderWindowPlugin());
  registry.registerWindowPlugin(createLedgerStubPlugin());
  registry.registerWindowPlugin(createDispatchInboxStubPlugin());

  mountAudioController(bus);
  mountCommandHud(appRoot, store, bus);

  const globe = await initializeGlobeViewport(canvas, store, bus, registry.getMapLayers());
  status.gpuBackend = globe.status.backend;
  status.shaderCompiled = globe.status.shaderCompiled;

  window.addEventListener("resize", () => {
    globe.resize(window.innerWidth, window.innerHeight);
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

  const initial = await bridge.getInitialState();
  handlePayload(initial, diagDialog);
}

void bootstrapShell();
