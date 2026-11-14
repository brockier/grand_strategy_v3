import { DiegeticSnapshot, DiegeticStoreAccess, GlobeLayerHost, MapLayerPlugin, UiIntentBus } from "../spine";

/** Baseline map overlay plugin cartridge for M0 shell verification. */
export function createBaselineOverlayPlugin(): MapLayerPlugin {
  return {
    layerId: "epistemic_fog_stub",
    displayName: "Survey Horizon",
    supportsMode: () => true,
    mount(_host: GlobeLayerHost, _store: DiegeticStoreAccess, _bus: UiIntentBus) {},
    onSnapshotUpdate(_snapshot: DiegeticSnapshot) {},
    onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number) {},
    dispose() {},
  };
}
