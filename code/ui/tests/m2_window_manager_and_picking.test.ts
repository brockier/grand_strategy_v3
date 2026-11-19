import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  GlobeLayerHost,
  PluginRegistry,
  UiIntentBus,
  UiIntent,
} from "../src/spine";
import {
  DesktopWindowManager,
  createPlaceholderWindowPlugin,
  deserializeAndClampLayout,
} from "../src/windows";
import { createEntityInspectorWindowPlugin, createLedgerStubPlugin } from "../src/views";
import { createDispatchInboxStubPlugin } from "../src/dispatches";
import { unitToLatLonDegrees } from "../src/globe";
import { OcclusionAwareCellPicker } from "../src/overlays";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m2_snapshot.json");
const M2_SNAPSHOTS = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot[];

interface DomEnvBackup {
  window: unknown;
  document: unknown;
  HTMLElement: unknown;
}

describe("M2 Desktop Window Manager, Docking, Layout Serialization & Chrome-Occluded Picking", () => {
  let dom: JSDOM;
  let backup: DomEnvBackup;
  let root: HTMLElement;
  let registry: PluginRegistry;
  let store: DiegeticStore;
  let bus: UiIntentBus;
  let manager: DesktopWindowManager;

  beforeEach(() => {
    dom = new JSDOM("<!doctype html><html><body><div id='app-shell'><canvas id='globe-canvas' width='1280' height='800'></canvas></div></body></html>");
    backup = {
      window: globalThis.window,
      document: globalThis.document,
      HTMLElement: globalThis.HTMLElement,
    };
    (globalThis as unknown as { window: unknown }).window = dom.window;
    (globalThis as unknown as { document: unknown }).document = dom.window.document;
    (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = dom.window.HTMLElement;

    root = dom.window.document.getElementById("app-shell") as HTMLElement;
    store = new DiegeticStore();
    store.ingestSnapshot(M2_SNAPSHOTS[0]);
    bus = new UiIntentBus();
    registry = new PluginRegistry();
    registry.registerWindowPlugin(createPlaceholderWindowPlugin());
    registry.registerWindowPlugin(createLedgerStubPlugin());
    registry.registerWindowPlugin(createEntityInspectorWindowPlugin());
    registry.registerWindowPlugin(createDispatchInboxStubPlugin());

    manager = new DesktopWindowManager(
      root,
      registry,
      { store, bus },
      { viewportWidth: 1280, viewportHeight: 800 },
    );
  });

  afterEach(() => {
    manager.dispose();
    (globalThis as unknown as { window: unknown }).window = backup.window;
    (globalThis as unknown as { document: unknown }).document = backup.document;
    (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = backup.HTMLElement;
  });

  // @req UI-WIN-001
  it("clamps dragged windows inside viewport bounds and supports arrow-key moving with aria-live announcements (UI-WIN-001)", () => {
    const winId = manager.openWindow("physical_ledger", { x: 100, y: 120, width: 400, height: 300 })!;
    expect(winId).toBeTruthy();

    // Drag beyond far top-left and bottom-right bounds -> clamped cleanly keeping header accessible
    manager.moveFrame(winId, -900, -500, false);
    const snapTL = manager.getFrameSnapshot(winId)!;
    expect(snapTL.x).toBe(-(400 - 80));
    expect(snapTL.y).toBe(48);

    manager.moveFrame(winId, 5000, 5000, false);
    const snapBR = manager.getFrameSnapshot(winId)!;
    expect(snapBR.x).toBe(1280 - 80);
    expect(snapBR.y).toBe(800 - 36);

    // Keyboard arrow-key repositioning announces new position in polite live region
    manager.moveFrame(winId, 200, 200, false);
    const dragBtn = dom.window.document.querySelector(`#${winId} .ng-win-drag-handle`) as HTMLButtonElement;
    dragBtn.focus();
    dragBtn.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "ArrowRight", shiftKey: true, bubbles: true }));
    dragBtn.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));

    const afterKey = manager.getFrameSnapshot(winId)!;
    expect(afterKey.x).toBe(248);
    expect(afterKey.y).toBe(216);

    const liveRegion = dom.window.document.getElementById("window-live-announcer")!;
    expect(liveRegion.getAttribute("aria-live")).toBe("polite");
    expect(liveRegion.textContent).toContain("x=248");
    expect(liveRegion.textContent).toContain("y=216");
  });

  // @req UI-WIN-002
  it("provides 8-direction resize handles clamping strictly between min and max plugin bounds (UI-WIN-002)", () => {
    const winId = manager.openWindow("physical_ledger", { x: 80, y: 80, width: 420, height: 320 })!;
    const handles = dom.window.document.querySelectorAll(`#${winId} .ng-resize-handle`);
    expect(handles).toHaveLength(8);
    const dirs = Array.from(handles).map((h) => h.getAttribute("data-resize-dir"));
    expect(dirs.sort()).toEqual(["e", "n", "ne", "nw", "s", "se", "sw", "w"]);

    // Shrink below minWidth(360) x minHeight(240) -> clamped strictly to minimums
    manager.resizeFrame(winId, { width: 100, height: 80 });
    const clampedMin = manager.getFrameSnapshot(winId)!;
    expect(clampedMin.width).toBe(360);
    expect(clampedMin.height).toBe(240);

    // Expand beyond viewport width/height -> clamped to available screen budget
    manager.resizeFrame(winId, { width: 3000, height: 2000 });
    const clampedMax = manager.getFrameSnapshot(winId)!;
    expect(clampedMax.width).toBe(1280 - 16);
    expect(clampedMax.height).toBe(800 - 48 - 12);
  });

  // @req UI-WIN-003
  it("collapses and expands windows without unmounting or resetting child filter input or scrollTop (UI-WIN-003)", () => {
    const winId = manager.openWindow("physical_ledger", { x: 80, y: 80, width: 400, height: 300 })!;
    const frameEl = dom.window.document.getElementById(winId)!;
    const filterInput = frameEl.querySelector<HTMLInputElement>("[data-ledger-filter='true']")!;
    const scrollEl = frameEl.querySelector<HTMLElement>("[data-ledger-scroll='true']")!;

    filterInput.value = "timber";
    filterInput.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    scrollEl.scrollTop = 94;

    const collapsed = manager.toggleCollapse(winId);
    expect(collapsed).toBe(true);
    expect(frameEl.classList.contains("is-collapsed")).toBe(true);
    expect(frameEl.querySelector(".ng-win-tabpanel")?.textContent).not.toBe("");

    const expanded = manager.toggleCollapse(winId);
    expect(expanded).toBe(false);
    expect(frameEl.classList.contains("is-collapsed")).toBe(false);
    expect(frameEl.querySelector<HTMLInputElement>("[data-ledger-filter='true']")?.value).toBe("timber");
    expect(frameEl.querySelector<HTMLElement>("[data-ledger-scroll='true']")?.scrollTop).toBe(94);
  });

  // @req UI-WIN-004
  it("supports close cleanup, global hotkeys, and pinned multi-instance side-by-side comparison windows (UI-WIN-004)", () => {
    // Hotkey 'i' opens inspector
    dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "i", bubbles: true }));
    expect(manager.listFrames()).toHaveLength(1);
    const firstId = manager.listFrames()[0].frameId;

    // Hotkey ignored while typing inside search input
    dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "p", bubbles: true }));
    const ledgerId = manager.listFrames()[1].frameId;
    const filterInput = dom.window.document.querySelector<HTMLInputElement>(`#${ledgerId} [data-ledger-filter='true']`)!;
    filterInput.focus();
    dom.window.document.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "o", bubbles: true }));
    expect(manager.listFrames()).toHaveLength(2);
    filterInput.blur();

    // Unpinned inspector updates in-place; pinning allows distinct second inspector window alongside first
    bus.emit({ kind: "FocusEntityInspector", entityKind: "settlement", entityId: 0 });
    expect(manager.listFrames().filter((f) => f.tabs[0].pluginId === "entity_inspector")).toHaveLength(1);

    manager.togglePin(firstId);
    expect(manager.getFrameSnapshot(firstId)?.tabs[0].pinned).toBe(true);

    bus.emit({ kind: "FocusEntityInspector", entityKind: "settlement", entityId: 1, pinned: true });
    const inspectors = manager.listFrames().filter((f) => f.tabs[0].pluginId === "entity_inspector");
    expect(inspectors).toHaveLength(2);
    expect(inspectors[0].tabs[0].instanceKey).not.toBe(inspectors[1].tabs[0].instanceKey);

    // Close cleans up frame DOM completely
    manager.closeWindow(ledgerId);
    expect(dom.window.document.getElementById(ledgerId)).toBeNull();
  });

  // @req UI-WIN-005
  // @req UI-MAP-005.1
  it("maintains deterministic z-order stacking, stops wheel propagation, and blocks map picking under chrome (UI-WIN-005, UI-MAP-005)", () => {
    const winA = manager.openWindow("physical_ledger", { x: 100, y: 100, width: 380, height: 280 })!;
    const winB = manager.openWindow("dispatch_inbox", { x: 200, y: 140, width: 380, height: 260 })!;

    expect(manager.getFrameSnapshot(winB)!.zIndex).toBeGreaterThan(manager.getFrameSnapshot(winA)!.zIndex);

    // Pointer interaction promotes winA deterministically above winB and isolates events
    const elA = dom.window.document.getElementById(winA)!;
    const parentWheelSpy = { called: false };
    root.addEventListener("wheel", () => {
      parentWheelSpy.called = true;
    });
    elA.dispatchEvent(new dom.window.MouseEvent("pointerdown", { bubbles: true }));
    elA.dispatchEvent(new dom.window.Event("wheel", { bubbles: true }));

    expect(manager.getFrameSnapshot(winA)!.zIndex).toBeGreaterThan(manager.getFrameSnapshot(winB)!.zIndex);
    expect(parentWheelSpy.called).toBe(false);

    // OcclusionAwareCellPicker must ignore pointer coordinates inside open window A (`UI-MAP-005`)
    const canvas = dom.window.document.getElementById("globe-canvas") as HTMLCanvasElement;
    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));

    const sampleUnit: [number, number, number] = [0, 0, 1];
    const eqCoords = unitToLatLonDegrees(sampleUnit);
    const northCoords = unitToLatLonDegrees([0, 1, 0]);
    const southCoords = unitToLatLonDegrees([0, -1, 0]);
    expect(eqCoords.latDeg).toBeCloseTo(0, 4);
    expect(eqCoords.lonDeg).toBeCloseTo(0, 4);
    expect(northCoords.latDeg).toBeCloseTo(90, 4);
    expect(southCoords.latDeg).toBeCloseTo(-90, 4);

    const host: GlobeLayerHost = {
      overlayRootId: "globe-overlay-root",
      getCanvasElement: () => canvas,
      pickGlobeAtNdc: () => ({
        hitKind: "settlement",
        cellId: 0,
        settlementId: 0,
        label: "First Landing Perimeter (HQ)",
        unitXyz: sampleUnit,
        latDeg: eqCoords.latDeg,
        lonDeg: eqCoords.lonDeg,
      }),
      isPointOccludedByChrome: (x, y) => manager.isPointOccluded(x, y),
    };
    const picker = new OcclusionAwareCellPicker();
    picker.mount(host, store, bus);

    // Coordinate (180, 180) is covered by winA -> occluded!
    canvas.dispatchEvent(new dom.window.MouseEvent("pointermove", { clientX: 180, clientY: 180, bubbles: true }));
    canvas.dispatchEvent(new dom.window.MouseEvent("click", { clientX: 180, clientY: 180, bubbles: true }));
    expect(emitted.some((e) => e.kind === "SelectMapFeature")).toBe(false);

    // Coordinate (10, 400) is open canvas -> picks settlement with continuous coordinates -> emits SelectMapFeature + FocusEntityInspector
    canvas.dispatchEvent(new dom.window.MouseEvent("pointermove", { clientX: 10, clientY: 400, bubbles: true }));
    expect(dom.window.document.getElementById("map-hover-tooltip")?.hidden).toBe(false);
    expect(dom.window.document.getElementById("map-hover-tooltip")?.textContent).toContain("First Landing Perimeter");

    canvas.dispatchEvent(new dom.window.MouseEvent("pointerdown", { clientX: 10, clientY: 400, bubbles: true }));
    canvas.dispatchEvent(new dom.window.MouseEvent("click", { clientX: 10, clientY: 400, bubbles: true }));
    const selectEv = emitted.find((e) => e.kind === "SelectMapFeature");
    expect(selectEv).toBeDefined();
    if (selectEv?.kind === "SelectMapFeature") {
      expect(selectEv.hit?.unitXyz).toEqual(sampleUnit);
      expect(selectEv.hit?.latDeg).toBeCloseTo(0, 4);
      expect(selectEv.hit?.lonDeg).toBeCloseTo(0, 4);
    }
    expect(emitted.some((e) => e.kind === "FocusEntityInspector")).toBe(true);

    picker.dispose();
  });

  // @req UI-WIN-006
  it("snaps magnetically within 14px, docks frames into accessible role=tablist with hidden='until-found'/beforematch, and tears off outward (UI-WIN-006)", () => {
    const winA = manager.openWindow("physical_ledger", { x: 50, y: 100, width: 380, height: 280 })!;
    const winB = manager.openWindow("dispatch_inbox", { x: 500, y: 100, width: 380, height: 280 })!;

    // Drag winB within 10px of winA's right edge (50 + 380 = 430) -> snaps cleanly to 430
    manager.moveFrame(winB, 439, 108, true);
    const snapped = manager.getFrameSnapshot(winB)!;
    expect(snapped.x).toBe(430);
    expect(snapped.y).toBe(100);

    // Dock winB into winA -> multi-tab frame with role=tablist, aria-selected, and hidden="until-found"
    expect(manager.dockFrameInto(winB, winA)).toBe(true);
    expect(manager.listFrames()).toHaveLength(1);
    const combined = manager.getFrameSnapshot(winA)!;
    expect(combined.tabs).toHaveLength(2);
    expect(combined.activeTabIndex).toBe(1);

    const frameEl = dom.window.document.getElementById(winA)!;
    const tabs = frameEl.querySelectorAll("[role='tab']");
    const panels = frameEl.querySelectorAll<HTMLElement>("[role='tabpanel']");
    expect(tabs).toHaveLength(2);
    expect(tabs[0].getAttribute("aria-selected")).toBe("false");
    expect(tabs[1].getAttribute("aria-selected")).toBe("true");
    expect(panels[0].getAttribute("hidden")).toBe("until-found");
    expect(panels[1].hasAttribute("hidden")).toBe(false);

    // Search-in-page `beforematch` activates hidden tab 0 cleanly
    panels[0].dispatchEvent(new dom.window.Event("beforematch", { bubbles: false }));
    expect(manager.getFrameSnapshot(winA)!.activeTabIndex).toBe(0);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");

    // Outward tear-off splits tab 1 back into an independent window frame
    const tornId = manager.tearOffTab(winA, 1, 620, 180);
    expect(tornId).toBeTruthy();
    expect(manager.listFrames()).toHaveLength(2);
    expect(manager.getFrameSnapshot(tornId!)!.x).toBe(620);
  });

  // @req UI-WIN-007
  it("serializes layouts, scales/clamps offscreen frames across resolution changes (1920x1080 -> 1280x720), and restores named presets (UI-WIN-007)", () => {
    expect(manager.applyNamedPreset("economic-audit")).toBe(true);
    expect(manager.listFrames().length).toBeGreaterThanOrEqual(2);

    // Create high-res 1920x1080 layout near right/bottom edge and restore at 1280x720
    const offscreenJson = JSON.stringify({
      schemaVersion: 1,
      viewportWidth: 1920,
      viewportHeight: 1080,
      frames: [
        {
          frameId: "wide-ledger",
          x: 1520,
          y: 780,
          width: 480,
          height: 360,
          collapsed: false,
          activeTabIndex: 0,
          tabs: [{ pluginId: "physical_ledger", instanceKey: "default", pinned: false }],
        },
      ],
    });

    const clamped = deserializeAndClampLayout(offscreenJson, 1280, 720, (id) => {
      const p = registry.getWindowPlugin(id);
      return p ? { minWidth: p.minWidth, minHeight: p.minHeight } : undefined;
    })!;
    expect(clamped.frames).toHaveLength(1);
    expect(clamped.frames[0].x).toBeLessThanOrEqual(1280 - 80);
    expect(clamped.frames[0].y).toBeLessThanOrEqual(720 - 36);
    expect(clamped.frames[0].y).toBeGreaterThanOrEqual(48);

    // Save custom named preset and switch between built-in presets deterministically
    manager.restoreFromJson(offscreenJson);
    manager.saveNamedPreset("custom-colonial");
    expect(manager.applyNamedPreset("minimalist-exploration")).toBe(true);
    expect(manager.listFrames()).toHaveLength(1);
    expect(manager.listFrames()[0].collapsed).toBe(true);

    expect(manager.applyNamedPreset("custom-colonial")).toBe(true);
    expect(manager.listFrames()).toHaveLength(1);
    expect(manager.listFrames()[0].collapsed).toBe(false);
  });

  // @req UI-WIN-001
  // @req UI-WIN-007
  it("dynamically clamps new, dragged, and preset windows below wrapped multi-row top HUD bars (UI-WIN-001, UI-WIN-007)", () => {
    const hud = dom.window.document.createElement("header");
    hud.id = "command-hud-bar";
    hud.className = "ng-hud-bar";
    hud.getBoundingClientRect = () =>
      ({
        top: 8,
        bottom: 98,
        left: 12,
        right: 1268,
        width: 1256,
        height: 90,
        x: 12,
        y: 8,
        toJSON: () => ({}),
      }) as DOMRect;
    root.appendChild(hud);

    const winId = manager.openWindow("entity_inspector")!;
    expect(manager.getFrameSnapshot(winId)!.y).toBeGreaterThanOrEqual(106);

    manager.moveFrame(winId, 20, 10, false);
    expect(manager.getFrameSnapshot(winId)!.y).toBeGreaterThanOrEqual(106);

    expect(manager.applyNamedPreset("war-room")).toBe(true);
    for (const frame of manager.listFrames()) {
      expect(frame.y).toBeGreaterThanOrEqual(106);
    }
  });

  // @req UI-GLB-008
  // @req UI-MAP-005.1
  it("re-evaluates hover circle and tooltip at stationary cursor while WASDQE camera navigation moves the globe (UI-GLB-008, UI-MAP-005.1)", () => {
    const canvas = dom.window.document.getElementById("globe-canvas") as HTMLCanvasElement;
    let camStep = 0;
    const sampleCellA: [number, number, number] = [0.98, 0.14, 0.14];
    const sampleCellB: [number, number, number] = [0.85, 0.42, 0.31];
    const host: GlobeLayerHost = {
      scene: {},
      camera: {},
      overlayRootId: "app-shell",
      getCanvasElement: () => canvas,
      projectUnitToScreen: () => ({ xPx: 400, yPx: 300, visible: true }),
      pickGlobeAtNdc: () =>
        camStep === 0
          ? {
              hitKind: "cell",
              cellId: 1326,
              localElevationM: 566,
              label: "savanna_steppe (Cell 1326, 566m)",
              unitXyz: sampleCellA,
              latDeg: 8,
              lonDeg: 8,
            }
          : {
              hitKind: "cell",
              cellId: 1307,
              localElevationM: 218,
              label: "savanna_steppe (Cell 1307, 218m)",
              unitXyz: sampleCellB,
              latDeg: 24,
              lonDeg: 20,
            },
      isPointOccludedByChrome: () => false,
    };

    const emitted: UiIntent[] = [];
    bus.subscribe((intent) => emitted.push(intent));

    const picker = new OcclusionAwareCellPicker();
    picker.mount(host, store, bus);

    canvas.dispatchEvent(new dom.window.MouseEvent("pointermove", { clientX: 400, clientY: 300, bubbles: true }));
    expect(dom.window.document.getElementById("map-hover-tooltip")?.textContent).toContain("Cell 1326");

    camStep = 1;
    bus.emit({
      kind: "UpdateCameraTelemetry",
      telemetry: {
        altitudeNorm: 0.25,
        altitudeRadius: 1.4,
        pitchRad: 0.7,
        headingRad: 0.2,
        focusCellId: 1307,
        focusUnit: sampleCellB,
      },
    });

    const tip = dom.window.document.getElementById("map-hover-tooltip");
    expect(tip?.hidden).toBe(false);
    expect(tip?.textContent).toContain("Cell 1307");
    const lastHover = [...emitted].reverse().find((i) => i.kind === "HoverMapFeature");
    expect(lastHover?.kind === "HoverMapFeature" ? lastHover.hit?.cellId : null).toBe(1307);
    expect(lastHover?.kind === "HoverMapFeature" ? lastHover.hit?.unitXyz : null).toEqual(sampleCellB);

    picker.dispose();
  });
});
