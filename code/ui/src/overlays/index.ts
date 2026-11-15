import {
  DiegeticSnapshot,
  DiegeticStoreAccess,
  GlobeLayerHost,
  GlobeRayHit,
  MapLayerPlugin,
  UiIntentBus,
} from "../spine";

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

/** Future Milestone M5 trade & logistics congestion overlay contract stub. */
export function createTradeAndCongestionStubLayer(): MapLayerPlugin {
  return {
    layerId: "trade_and_congestion_stub",
    displayName: "Trade & Congestion (M5)",
    supportsMode: (modeId) => modeId === "trade",
    mount(_host: GlobeLayerHost, _store: DiegeticStoreAccess, _bus: UiIntentBus) {},
    onSnapshotUpdate(_snapshot: DiegeticSnapshot) {},
    onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number) {},
    dispose() {},
  };
}

/** Future Milestone M7 operational warfare fronts overlay contract stub. */
export function createOperationalWarfareStubLayer(): MapLayerPlugin {
  return {
    layerId: "operational_warfare_stub",
    displayName: "Operational Warfare (M7)",
    supportsMode: (modeId) => modeId === "warfare",
    mount(_host: GlobeLayerHost, _store: DiegeticStoreAccess, _bus: UiIntentBus) {},
    onSnapshotUpdate(_snapshot: DiegeticSnapshot) {},
    onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number) {},
    dispose() {},
  };
}

function clientPointToNdc(canvas: HTMLCanvasElement, clientX: number, clientY: number): {
  readonly ndcX: number;
  readonly ndcY: number;
} {
  const rect = canvas.getBoundingClientRect();
  const width = rect.width > 0 ? rect.width : Math.max(canvas.clientWidth || canvas.width || 1280, 1);
  const height = rect.height > 0 ? rect.height : Math.max(canvas.clientHeight || canvas.height || 720, 1);
  const left = rect.width > 0 ? rect.left : 0;
  const top = rect.height > 0 ? rect.top : 0;
  const ndcX = ((clientX - left) / width) * 2 - 1;
  const ndcY = -(((clientY - top) / height) * 2 - 1);
  return { ndcX, ndcY };
}

/**
 * Chrome-occlusion-aware ray-picking & hover tooltip controller (`UI-MAP-005`).
 * Aborts hover/click picking whenever a floating window, HUD bar, or modal dialog
 * occludes the cursor coordinate.
 */
export class OcclusionAwareCellPicker implements MapLayerPlugin {
  readonly layerId = "diegetic_picking_and_hover";
  readonly displayName = "Diegetic Picking & Hover";
  private tooltip: HTMLDivElement | null = null;
  private cleanups: Array<() => void> = [];
  private lastHoveredHit: GlobeRayHit | null = null;

  public supportsMode(): boolean {
    return true;
  }

  public mount(host: GlobeLayerHost, _store: DiegeticStoreAccess, bus: UiIntentBus): void {
    this.dispose();
    const canvas = host.getCanvasElement();
    const overlayRoot = document.getElementById(host.overlayRootId) ?? canvas.parentElement ?? document.body;

    const tip = document.createElement("div");
    tip.id = "map-hover-tooltip";
    tip.className = "ng-map-hover-tooltip";
    tip.setAttribute("role", "status");
    tip.hidden = true;
    overlayRoot.appendChild(tip);
    this.tooltip = tip;

    let downX = 0;
    let downY = 0;
    let pointerDownActive = false;

    const hideHover = (clientX: number, clientY: number) => {
      if (this.tooltip) {
        this.tooltip.hidden = true;
      }
      if (this.lastHoveredHit !== null) {
        this.lastHoveredHit = null;
        bus.emit({ kind: "HoverMapFeature", hit: null, clientX, clientY });
      }
    };

    const onPointerDown = (ev: PointerEvent) => {
      downX = ev.clientX;
      downY = ev.clientY;
      pointerDownActive = !host.isPointOccludedByChrome(ev.clientX, ev.clientY);
    };

    const onPointerMove = (ev: PointerEvent) => {
      if (host.isPointOccludedByChrome(ev.clientX, ev.clientY)) {
        hideHover(ev.clientX, ev.clientY);
        return;
      }
      const { ndcX, ndcY } = clientPointToNdc(canvas, ev.clientX, ev.clientY);
      const hit = host.pickGlobeAtNdc(ndcX, ndcY);
      this.lastHoveredHit = hit;
      if (hit && this.tooltip) {
        this.tooltip.hidden = false;
        this.tooltip.textContent = hit.label;
        this.tooltip.style.left = `${Math.round(ev.clientX + 14)}px`;
        this.tooltip.style.top = `${Math.round(ev.clientY + 14)}px`;
      } else if (this.tooltip) {
        this.tooltip.hidden = true;
      }
      bus.emit({ kind: "HoverMapFeature", hit, clientX: ev.clientX, clientY: ev.clientY });
    };

    const onPointerLeave = () => {
      hideHover(0, 0);
    };

    const onClick = (ev: MouseEvent) => {
      if (host.isPointOccludedByChrome(ev.clientX, ev.clientY)) {
        hideHover(ev.clientX, ev.clientY);
        return;
      }
      const distMoved = pointerDownActive ? Math.hypot(ev.clientX - downX, ev.clientY - downY) : 0;
      pointerDownActive = false;
      if (distMoved >= 6) return;

      const { ndcX, ndcY } = clientPointToNdc(canvas, ev.clientX, ev.clientY);
      const hit = host.pickGlobeAtNdc(ndcX, ndcY);
      if (!hit) return;

      bus.emit({ kind: "SelectMapFeature", hit });
      bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });

      if (hit.hitKind === "settlement") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "settlement",
          entityId: hit.settlementId,
        });
      } else if (hit.hitKind === "convoy") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "convoy",
          entityId: hit.convoyId,
        });
      } else if (hit.hitKind === "cell" || hit.hitKind === "river" || hit.hitKind === "road") {
        bus.emit({
          kind: "FocusEntityInspector",
          entityKind: "cell",
          entityId: hit.cellId,
        });
      }
    };

    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerleave", onPointerLeave);
    canvas.addEventListener("click", onClick);

    this.cleanups.push(() => {
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerleave", onPointerLeave);
      canvas.removeEventListener("click", onClick);
      tip.remove();
    });
  }

  public onSnapshotUpdate(_snapshot: DiegeticSnapshot): void {}

  public onFrameRender(_cameraAltitudeNorm: number, _elapsedSec: number): void {}

  public dispose(): void {
    while (this.cleanups.length > 0) {
      const fn = this.cleanups.pop();
      fn?.();
    }
    this.tooltip = null;
    this.lastHoveredHit = null;
  }
}
