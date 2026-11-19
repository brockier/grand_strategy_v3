import {
  LayoutPresetPort,
  PluginRegistry,
  UiSpineContext,
  WindowContentPlugin,
} from "../spine";
import {
  BUILTIN_LAYOUT_PRESETS,
  HUD_TOP_MARGIN_PX,
  SerializedWindowFrameDto,
  clampFramePosition,
  deserializeAndClampLayout,
  serializeLayout,
} from "./layout-serializer";

export type ResizeDirection = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

const MAGNET_SNAP_THRESHOLD_PX = 14;
const COLLAPSED_HEADER_HEIGHT_PX = 40;
const RESIZE_DIRECTIONS: readonly ResizeDirection[] = [
  "n",
  "s",
  "e",
  "w",
  "ne",
  "nw",
  "se",
  "sw",
];

interface ManagedTab {
  readonly tabId: string;
  readonly plugin: WindowContentPlugin;
  instanceKey: string;
  pinned: boolean;
  readonly tabBtn: HTMLButtonElement;
  readonly panelEl: HTMLDivElement;
  readonly innerEl: HTMLDivElement;
  disposeContent: () => void;
}

interface ManagedFrame {
  readonly frameId: string;
  x: number;
  y: number;
  width: number;
  height: number;
  collapsed: boolean;
  activeTabIndex: number;
  zIndex: number;
  readonly el: HTMLElement;
  readonly titleTextEl: HTMLSpanElement;
  readonly dragHandleBtn: HTMLButtonElement;
  readonly pinBtn: HTMLButtonElement;
  readonly collapseBtn: HTMLButtonElement;
  readonly closeBtn: HTMLButtonElement;
  readonly tablistEl: HTMLDivElement;
  readonly bodyEl: HTMLDivElement;
  readonly tabs: ManagedTab[];
}

export interface FrameStateSnapshot {
  readonly frameId: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly collapsed: boolean;
  readonly zIndex: number;
  readonly activeTabIndex: number;
  readonly focused: boolean;
  readonly tabs: ReadonlyArray<{
    readonly tabId: string;
    readonly pluginId: string;
    readonly instanceKey: string;
    readonly pinned: boolean;
  }>;
}

export interface OpenWindowOptions {
  readonly instanceKey?: string;
  readonly pinned?: boolean;
  readonly x?: number;
  readonly y?: number;
  readonly width?: number;
  readonly height?: number;
  readonly collapsed?: boolean;
}

function clamp(v: number, lo: number, hi: number): number {
  if (hi < lo) return lo;
  return Math.min(hi, Math.max(lo, v));
}

function isEditableTarget(el: Element | null): boolean {
  if (!el) return false;
  const tag = el.tagName.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  return (el as HTMLElement).isContentEditable === true;
}

/**
 * Multi-window floating desktop manager implementing drag/clamp, 8-way resize,
 * collapse-without-unmount, multi-instance pinning, deterministic z-order isolation,
 * magnetic snap + accessible tablist docking/tear-off, and layout preset persistence
 * (`UI-WIN-001`..`UI-WIN-007`, `UI-SKN-002`).
 */
export class DesktopWindowManager implements LayoutPresetPort {
  private readonly hostRoot: HTMLElement;
  private readonly registry: PluginRegistry;
  private spine: UiSpineContext;
  private readonly layerEl: HTMLElement;
  private readonly announcerEl: HTMLDivElement;
  private readonly frames: ManagedFrame[] = [];
  private readonly namedPresets = new Map<string, string>();
  private viewportW: number;
  private viewportH: number;
  private nextId = 1;
  private persistCallback?: (activeLayoutJson: string, presets: Record<string, string>) => void;
  private readonly cleanups: Array<() => void> = [];

  constructor(
    hostRoot: HTMLElement,
    registry: PluginRegistry,
    spine: UiSpineContext,
    options: {
      readonly viewportWidth?: number;
      readonly viewportHeight?: number;
      readonly initialPresets?: Readonly<Record<string, string>>;
      readonly onPersist?: (activeLayoutJson: string, presets: Record<string, string>) => void;
    } = {},
  ) {
    this.hostRoot = hostRoot;
    this.registry = registry;
    this.spine = { ...spine, layoutPresets: this };
    this.viewportW = Math.max(320, options.viewportWidth ?? (typeof window !== "undefined" ? window.innerWidth || 1280 : 1280));
    this.viewportH = Math.max(240, options.viewportHeight ?? (typeof window !== "undefined" ? window.innerHeight || 800 : 800));
    this.persistCallback = options.onPersist;

    for (const [name, doc] of Object.entries(BUILTIN_LAYOUT_PRESETS)) {
      this.namedPresets.set(name, JSON.stringify(doc));
    }
    if (options.initialPresets) {
      for (const [name, raw] of Object.entries(options.initialPresets)) {
        this.namedPresets.set(name, raw);
      }
    }

    this.layerEl = document.createElement("section");
    this.layerEl.id = "window-layer";
    this.layerEl.className = "ng-window-layer";
    this.layerEl.setAttribute("aria-label", "Workspace Windows");

    this.announcerEl = document.createElement("div");
    this.announcerEl.id = "window-live-announcer";
    this.announcerEl.className = "ng-sr-only";
    this.announcerEl.setAttribute("aria-live", "polite");

    this.hostRoot.appendChild(this.layerEl);
    this.hostRoot.appendChild(this.announcerEl);

    const unsubBus = this.spine.bus.subscribe((intent) => {
      if (intent.kind === "OpenSystemWindow") {
        this.openWindow(intent.pluginId, {
          instanceKey: intent.instanceKey,
          pinned: intent.pinned,
        });
      } else if (intent.kind === "FocusEntityInspector") {
        const key =
          intent.localElevationM !== undefined
            ? `${intent.entityKind}:${intent.entityId}:${Math.round(intent.localElevationM)}`
            : `${intent.entityKind}:${intent.entityId}`;
        this.openWindow("entity_inspector", {
          instanceKey: key,
          pinned: intent.pinned,
        });
      } else if (intent.kind === "CloseWindow") {
        this.closeWindow(intent.windowId);
      } else if (intent.kind === "ApplyWindowLayoutPreset") {
        this.applyNamedPreset(intent.presetName);
      } else if (intent.kind === "SaveWindowLayoutPreset") {
        this.saveNamedPreset(intent.presetName);
      } else if (intent.kind === "SkinBundleUpdated") {
        this.reclampFramesBelowHud();
      }
    });

    const onKeyDown = (ev: KeyboardEvent) => {
      if (isEditableTarget(document.activeElement)) return;
      if (ev.key === "Escape") {
        const top = this.frames[this.frames.length - 1];
        if (top && !top.tabs[top.activeTabIndex]?.pinned) {
          ev.preventDefault();
          this.closeWindow(top.frameId);
        }
        return;
      }
      if (ev.ctrlKey || ev.altKey || ev.metaKey) return;
      const lowerKey = ev.key.toLowerCase();
      for (const plugin of this.registry.listWindowPlugins()) {
        const matchesHotkey = plugin.hotkey && plugin.hotkey.toLowerCase() === lowerKey;
        const matchesHelpF1 = ev.key === "F1" && plugin.pluginId === "controls_reference";
        if (matchesHotkey || matchesHelpF1) {
          ev.preventDefault();
          this.togglePluginWindow(plugin.pluginId);
          return;
        }
      }
    };

    const doc = this.hostRoot.ownerDocument;
    doc.addEventListener("keydown", onKeyDown);

    const unsubStore = this.spine.store.subscribe(() => {
      this.reclampFramesBelowHud();
    });

    let hudResizeObserver: ResizeObserver | null = null;
    let observedHudEl: HTMLElement | null = null;
    const attachHudResizeObserver = () => {
      const hudEl = this.findHudBarElement();
      if (!hudResizeObserver || hudEl === observedHudEl) return;
      if (observedHudEl) {
        hudResizeObserver.unobserve(observedHudEl);
      }
      observedHudEl = hudEl;
      if (hudEl) {
        hudResizeObserver.observe(hudEl);
        this.reclampFramesBelowHud();
      }
    };

    if (typeof ResizeObserver !== "undefined") {
      hudResizeObserver = new ResizeObserver(() => {
        this.reclampFramesBelowHud();
      });
      attachHudResizeObserver();
    }

    let hudMutationObserver: MutationObserver | null = null;
    if (typeof MutationObserver !== "undefined") {
      hudMutationObserver = new MutationObserver(() => {
        attachHudResizeObserver();
        this.reclampFramesBelowHud();
      });
      hudMutationObserver.observe(this.hostRoot, { childList: true });
    }

    this.cleanups.push(
      unsubBus,
      unsubStore,
      () => doc.removeEventListener("keydown", onKeyDown),
      () => hudResizeObserver?.disconnect(),
      () => hudMutationObserver?.disconnect(),
    );
  }

  public setSpinePorts(next: UiSpineContext): void {
    this.spine = { ...next, layoutPresets: this };
  }

  public setPersistHandler(fn: (activeLayoutJson: string, presets: Record<string, string>) => void): void {
    this.persistCallback = fn;
  }

  private emitPersistence(): void {
    if (!this.persistCallback) return;
    const presetMap: Record<string, string> = {};
    for (const [k, v] of this.namedPresets.entries()) {
      presetMap[k] = v;
    }
    this.persistCallback(this.exportCurrentLayoutJson(), presetMap);
  }

  private announce(message: string): void {
    this.announcerEl.textContent = message;
  }

  private findHudBarElement(): HTMLElement | null {
    return (
      this.hostRoot.querySelector<HTMLElement>("#command-hud-bar, .ng-hud-bar") ??
      this.hostRoot.ownerDocument.querySelector<HTMLElement>("#command-hud-bar, .ng-hud-bar")
    );
  }

  public getEffectiveTopMarginPx(): number {
    const hudEl = this.findHudBarElement();
    if (!hudEl) return HUD_TOP_MARGIN_PX;
    const rect = typeof hudEl.getBoundingClientRect === "function" ? hudEl.getBoundingClientRect() : null;
    const rectBottom = rect && rect.height > 0 && rect.bottom > 0 ? rect.bottom : 0;
    const offsetBottom = hudEl.offsetHeight > 0 ? hudEl.offsetTop + hudEl.offsetHeight : 0;
    const measuredBottom = Math.max(rectBottom, offsetBottom);
    if (measuredBottom <= 0) return HUD_TOP_MARGIN_PX;
    return Math.max(HUD_TOP_MARGIN_PX, Math.ceil(measuredBottom + 8));
  }

  public reclampFramesBelowHud(): void {
    const topMargin = this.getEffectiveTopMarginPx();
    for (const frame of this.frames) {
      const bounds = this.computeFrameBounds(frame, topMargin);
      const clampedW = Math.round(clamp(frame.width, bounds.minW, bounds.maxW));
      const clampedH = Math.round(clamp(frame.height, bounds.minH, bounds.maxH));
      const clampedPos = clampFramePosition(
        frame.x,
        frame.y,
        clampedW,
        this.viewportW,
        this.viewportH,
        topMargin,
      );
      if (
        clampedW !== frame.width ||
        clampedH !== frame.height ||
        clampedPos.x !== frame.x ||
        clampedPos.y !== frame.y
      ) {
        frame.width = clampedW;
        frame.height = clampedH;
        frame.x = clampedPos.x;
        frame.y = clampedPos.y;
        this.syncFrameGeometry(frame);
      }
    }
  }

  private computeFrameBounds(
    frame: ManagedFrame,
    topMarginPx: number = this.getEffectiveTopMarginPx(),
  ): {
    minW: number;
    minH: number;
    maxW: number;
    maxH: number;
  } {
    let minW = 240;
    let minH = 160;
    let maxW = Math.max(240, this.viewportW - 16);
    let maxH = Math.max(160, this.viewportH - topMarginPx - 12);
    for (const tab of frame.tabs) {
      minW = Math.max(minW, tab.plugin.minWidth);
      minH = Math.max(minH, tab.plugin.minHeight);
      if (tab.plugin.maxWidth !== undefined) maxW = Math.min(maxW, tab.plugin.maxWidth);
      if (tab.plugin.maxHeight !== undefined) maxH = Math.min(maxH, tab.plugin.maxHeight);
    }
    return { minW, minH, maxW: Math.max(minW, maxW), maxH: Math.max(minH, maxH) };
  }

  public togglePluginWindow(pluginId: string): string | null {
    const existing = this.frames.find((f) =>
      f.tabs.some((t) => t.plugin.pluginId === pluginId && !t.pinned),
    );
    if (existing) {
      const idx = existing.tabs.findIndex((t) => t.plugin.pluginId === pluginId && !t.pinned);
      this.closeTab(existing.frameId, idx);
      return null;
    }
    return this.openWindow(pluginId);
  }

  public openWindow(pluginId: string, opts: OpenWindowOptions = {}): string | null {
    const plugin = this.registry.getWindowPlugin(pluginId);
    if (!plugin) return null;

    const wantPinned = Boolean(opts.pinned);
    const wantKey = opts.instanceKey ?? "default";

    // Reuse matching unpinned tab unless explicitly pinned for side-by-side comparison (`UI-WIN-004`).
    if (!wantPinned) {
      for (const frame of this.frames) {
        const idx = frame.tabs.findIndex((t) => {
          if (t.plugin.pluginId !== pluginId || t.pinned) return false;
          if (pluginId === "entity_inspector") return true;
          return t.instanceKey === wantKey;
        });
        if (idx !== -1) {
          const tab = frame.tabs[idx];
          if (tab.instanceKey !== wantKey) {
            tab.disposeContent();
            tab.instanceKey = wantKey;
            tab.innerEl.replaceChildren();
            tab.disposeContent = plugin.mountContent(tab.innerEl, wantKey, this.spine);
          }
          if (frame.collapsed) {
            this.toggleCollapse(frame.frameId);
          }
          this.activateTab(frame.frameId, tab.tabId);
          this.focusFrame(frame.frameId);
          this.reclampFramesBelowHud();
          return frame.frameId;
        }
      }
    }

    const frameId = `win-${this.nextId}`;
    this.nextId += 1;
    const topMargin = this.getEffectiveTopMarginPx();
    const width = Math.max(plugin.minWidth, opts.width ?? plugin.minWidth + 60);
    const height = Math.max(plugin.minHeight, opts.height ?? plugin.minHeight + 60);
    const stagger = (this.frames.length % 5) * 32;
    const rawX = opts.x ?? 24 + stagger;
    const rawY = opts.y ?? topMargin + 12 + stagger;
    const clampedPos = clampFramePosition(rawX, rawY, width, this.viewportW, this.viewportH, topMargin);

    const frame = this.createFrameElement(frameId, clampedPos.x, clampedPos.y, width, height, Boolean(opts.collapsed));
    const tab = this.createTabForPlugin(frame, plugin, wantKey, wantPinned);
    frame.tabs.push(tab);
    frame.activeTabIndex = 0;

    this.frames.push(frame);
    this.layerEl.appendChild(frame.el);
    this.syncFrameChrome(frame);
    this.focusFrame(frame.frameId);
    this.spine.bus.emit({ kind: "PlayUiFoley", cueId: "window_open" });
    this.emitPersistence();
    return frame.frameId;
  }

  private createFrameElement(
    frameId: string,
    x: number,
    y: number,
    width: number,
    height: number,
    collapsed: boolean,
  ): ManagedFrame {
    const el = document.createElement("section");
    el.id = frameId;
    el.className = "ng-floating-window ng-window-frame";
    if (collapsed) el.classList.add("is-collapsed");
    el.setAttribute("data-frame-id", frameId);
    el.setAttribute("data-chrome-occluder", "true");
    el.setAttribute("aria-label", "Floating Window");

    const titlebar = document.createElement("div");
    titlebar.className = "ng-win-titlebar";

    const dragBtn = document.createElement("button");
    dragBtn.type = "button";
    dragBtn.className = "ng-win-drag-handle";
    dragBtn.setAttribute("data-win-drag", "true");

    const titleTextEl = document.createElement("span");
    titleTextEl.textContent = "Window";
    dragBtn.appendChild(titleTextEl);

    const controls = document.createElement("div");
    controls.className = "ng-win-controls";

    const pinBtn = document.createElement("button");
    pinBtn.type = "button";
    pinBtn.className = "ng-win-icon-btn";
    pinBtn.setAttribute("data-win-action", "pin");
    pinBtn.setAttribute("aria-pressed", "false");
    pinBtn.setAttribute("aria-label", "Pin window for side-by-side comparison");
    pinBtn.textContent = "Pin";

    const collapseBtn = document.createElement("button");
    collapseBtn.type = "button";
    collapseBtn.className = "ng-win-icon-btn";
    collapseBtn.setAttribute("data-win-action", "collapse");
    collapseBtn.setAttribute("aria-expanded", collapsed ? "false" : "true");
    collapseBtn.setAttribute("aria-label", "Collapse window body");
    collapseBtn.textContent = collapsed ? "+" : "-";

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "ng-win-icon-btn";
    closeBtn.setAttribute("data-win-action", "close");
    closeBtn.setAttribute("aria-label", "Close window");
    closeBtn.textContent = "x";

    controls.append(pinBtn, collapseBtn, closeBtn);
    titlebar.append(dragBtn, controls);

    const tablistEl = document.createElement("div");
    tablistEl.className = "ng-win-tablist";
    tablistEl.setAttribute("role", "tablist");
    tablistEl.setAttribute("aria-label", "Docked window tabs");
    tablistEl.hidden = true;

    const bodyEl = document.createElement("div");
    bodyEl.className = "ng-window-body";

    el.append(titlebar, tablistEl, bodyEl);

    for (const dir of RESIZE_DIRECTIONS) {
      const handle = document.createElement("div");
      handle.className = "ng-resize-handle";
      handle.setAttribute("data-resize-dir", dir);
      handle.setAttribute("aria-hidden", "true");
      el.appendChild(handle);
    }

    const frame: ManagedFrame = {
      frameId,
      x,
      y,
      width,
      height,
      collapsed,
      activeTabIndex: 0,
      zIndex: 100,
      el,
      titleTextEl,
      dragHandleBtn: dragBtn,
      pinBtn,
      collapseBtn,
      closeBtn,
      tablistEl,
      bodyEl,
      tabs: [],
    };

    this.wireFrameInteractions(frame);
    return frame;
  }

  private createTabForPlugin(
    frame: ManagedFrame,
    plugin: WindowContentPlugin,
    instanceKey: string,
    pinned: boolean,
  ): ManagedTab {
    const tabId = `tab-${this.nextId}`;
    this.nextId += 1;

    const tabBtn = document.createElement("button");
    tabBtn.type = "button";
    tabBtn.id = tabId;
    tabBtn.className = "ng-win-tab";
    tabBtn.setAttribute("role", "tab");
    tabBtn.setAttribute("aria-controls", `panel-${tabId}`);
    tabBtn.setAttribute("aria-selected", "false");
    tabBtn.tabIndex = -1;
    tabBtn.textContent = pinned ? `${plugin.defaultTitle} [Pinned]` : plugin.defaultTitle;

    const panelEl = document.createElement("div");
    panelEl.id = `panel-${tabId}`;
    panelEl.className = "ng-win-tabpanel ng-scroll-container";
    panelEl.setAttribute("role", "tabpanel");
    panelEl.setAttribute("aria-labelledby", tabId);

    const innerEl = document.createElement("div");
    innerEl.className = "ng-win-tabpanel-inner";
    panelEl.appendChild(innerEl);

    const disposeContent = plugin.mountContent(innerEl, instanceKey, this.spine);

    const managedTab: ManagedTab = {
      tabId,
      plugin,
      instanceKey,
      pinned,
      tabBtn,
      panelEl,
      innerEl,
      disposeContent,
    };

    this.wireTabInteractions(managedTab);
    frame.tablistEl.appendChild(tabBtn);
    frame.bodyEl.appendChild(panelEl);
    return managedTab;
  }

  private wireTabInteractions(tab: ManagedTab): void {
    const ownerFrame = () => this.frames.find((f) => f.tabs.includes(tab));

    tab.panelEl.addEventListener("beforematch", () => {
      const f = ownerFrame();
      if (f) {
        this.activateTab(f.frameId, tab.tabId);
        this.focusFrame(f.frameId);
      }
    });

    tab.tabBtn.addEventListener("click", () => {
      const f = ownerFrame();
      if (f) {
        this.activateTab(f.frameId, tab.tabId);
        this.focusFrame(f.frameId);
      }
    });

    tab.tabBtn.addEventListener("keydown", (ev: KeyboardEvent) => {
      const f = ownerFrame();
      if (!f || f.tabs.length <= 1) return;
      const curIdx = f.tabs.indexOf(tab);
      let nextIdx = curIdx;
      if (ev.key === "ArrowRight") nextIdx = (curIdx + 1) % f.tabs.length;
      else if (ev.key === "ArrowLeft") nextIdx = (curIdx - 1 + f.tabs.length) % f.tabs.length;
      else if (ev.key === "Home") nextIdx = 0;
      else if (ev.key === "End") nextIdx = f.tabs.length - 1;
      else return;

      ev.preventDefault();
      this.activateTab(f.frameId, f.tabs[nextIdx].tabId);
      f.tabs[nextIdx].tabBtn.focus();
    });

    // Outward tab drag tear-off (`UI-WIN-006`)
    let draggingTab = false;
    let startX = 0;
    let startY = 0;
    tab.tabBtn.addEventListener("pointerdown", (ev: PointerEvent) => {
      const f = ownerFrame();
      if (!f || f.tabs.length <= 1 || ev.button !== 0) return;
      draggingTab = true;
      startX = ev.clientX;
      startY = ev.clientY;
      tab.tabBtn.setPointerCapture?.(ev.pointerId);
    });

    tab.tabBtn.addEventListener("pointermove", (ev: PointerEvent) => {
      if (!draggingTab) return;
      const f = ownerFrame();
      if (!f || f.tabs.length <= 1) {
        draggingTab = false;
        return;
      }
      const distOut = Math.hypot(ev.clientX - startX, ev.clientY - startY);
      if (Math.abs(ev.clientY - startY) > 24 || distOut > 48) {
        draggingTab = false;
        const idx = f.tabs.indexOf(tab);
        if (idx !== -1) {
          this.tearOffTab(f.frameId, idx, ev.clientX - 120, ev.clientY - 16);
        }
      }
    });

    const endTabDrag = () => {
      draggingTab = false;
    };
    tab.tabBtn.addEventListener("pointerup", endTabDrag);
    tab.tabBtn.addEventListener("lostpointercapture", endTabDrag);
  }

  private wireFrameInteractions(frame: ManagedFrame): void {
    // Deterministic focus + input isolation (`UI-WIN-005`)
    frame.el.addEventListener("pointerdown", (ev: PointerEvent) => {
      this.focusFrame(frame.frameId);
      ev.stopPropagation();
    });
    frame.el.addEventListener("wheel", (ev: WheelEvent) => {
      ev.stopPropagation();
    });

    frame.pinBtn.addEventListener("click", () => this.togglePin(frame.frameId));
    frame.collapseBtn.addEventListener("click", () => this.toggleCollapse(frame.frameId));
    frame.closeBtn.addEventListener("click", () => {
      this.closeTab(frame.frameId, frame.activeTabIndex);
    });

    // Pointer-capture drag + magnetic snap + titlebar docking (`UI-WIN-001`, `UI-WIN-006`)
    let dragActive = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let frameOrigX = 0;
    let frameOrigY = 0;

    frame.dragHandleBtn.addEventListener("pointerdown", (ev: PointerEvent) => {
      if (ev.button !== 0) return;
      ev.stopPropagation();
      this.focusFrame(frame.frameId);
      dragActive = true;
      dragStartX = ev.clientX;
      dragStartY = ev.clientY;
      frameOrigX = frame.x;
      frameOrigY = frame.y;
      frame.dragHandleBtn.classList.add("dragging");
      frame.dragHandleBtn.setPointerCapture?.(ev.pointerId);
    });

    frame.dragHandleBtn.addEventListener("pointermove", (ev: PointerEvent) => {
      if (!dragActive) return;
      const nextX = frameOrigX + (ev.clientX - dragStartX);
      const nextY = frameOrigY + (ev.clientY - dragStartY);
      this.moveFrame(frame.frameId, nextX, nextY, true);
    });

    const finishDrag = (ev?: PointerEvent) => {
      if (!dragActive) return;
      dragActive = false;
      frame.dragHandleBtn.classList.remove("dragging");
      if (ev) {
        const dropTarget = this.findDockTargetAtPoint(frame.frameId, ev.clientX, ev.clientY);
        if (dropTarget) {
          this.dockFrameInto(frame.frameId, dropTarget.frameId);
          return;
        }
      }
      this.emitPersistence();
    };

    frame.dragHandleBtn.addEventListener("pointerup", finishDrag);
    frame.dragHandleBtn.addEventListener("lostpointercapture", () => finishDrag());

    // Keyboard arrow-key drag accessibility (`UI-WIN-001`)
    frame.dragHandleBtn.addEventListener("keydown", (ev: KeyboardEvent) => {
      const step = ev.shiftKey ? 48 : 16;
      let dx = 0;
      let dy = 0;
      if (ev.key === "ArrowLeft") dx = -step;
      else if (ev.key === "ArrowRight") dx = step;
      else if (ev.key === "ArrowUp") dy = -step;
      else if (ev.key === "ArrowDown") dy = step;
      else return;

      ev.preventDefault();
      this.moveFrame(frame.frameId, frame.x + dx, frame.y + dy, false);
      const title = frame.tabs[frame.activeTabIndex]?.plugin.defaultTitle ?? frame.frameId;
      this.announce(`Moved ${title} to x=${frame.x}, y=${frame.y}`);
      this.emitPersistence();
    });

    // Eight-direction resize handles (`UI-WIN-002`)
    for (const handle of Array.from(frame.el.querySelectorAll<HTMLElement>(".ng-resize-handle"))) {
      const dir = handle.getAttribute("data-resize-dir") as ResizeDirection;
      let resizing = false;
      let startX = 0;
      let startY = 0;
      let origRect = { x: 0, y: 0, width: 0, height: 0 };

      handle.addEventListener("pointerdown", (ev: PointerEvent) => {
        if (ev.button !== 0 || frame.collapsed) return;
        ev.stopPropagation();
        this.focusFrame(frame.frameId);
        resizing = true;
        startX = ev.clientX;
        startY = ev.clientY;
        origRect = { x: frame.x, y: frame.y, width: frame.width, height: frame.height };
        handle.setPointerCapture?.(ev.pointerId);
      });

      handle.addEventListener("pointermove", (ev: PointerEvent) => {
        if (!resizing) return;
        const dx = ev.clientX - startX;
        const dy = ev.clientY - startY;
        let nextX = origRect.x;
        let nextY = origRect.y;
        let nextW = origRect.width;
        let nextH = origRect.height;

        if (dir.includes("e")) nextW = origRect.width + dx;
        if (dir.includes("s")) nextH = origRect.height + dy;
        if (dir.includes("w")) {
          nextW = origRect.width - dx;
          nextX = origRect.x + dx;
        }
        if (dir.includes("n")) {
          nextH = origRect.height - dy;
          nextY = origRect.y + dy;
        }

        const bounds = this.computeFrameBounds(frame);
        const clampedW = clamp(nextW, bounds.minW, bounds.maxW);
        const clampedH = clamp(nextH, bounds.minH, bounds.maxH);
        if (dir.includes("w")) {
          nextX = origRect.x + (origRect.width - clampedW);
        }
        if (dir.includes("n")) {
          nextY = origRect.y + (origRect.height - clampedH);
        }
        this.resizeFrame(frame.frameId, {
          x: nextX,
          y: nextY,
          width: clampedW,
          height: clampedH,
        });
      });

      const stopResize = () => {
        if (!resizing) return;
        resizing = false;
        this.emitPersistence();
      };
      handle.addEventListener("pointerup", stopResize);
      handle.addEventListener("lostpointercapture", stopResize);
    }
  }

  private findDockTargetAtPoint(sourceFrameId: string, clientX: number, clientY: number): ManagedFrame | null {
    for (let i = this.frames.length - 1; i >= 0; i -= 1) {
      const f = this.frames[i];
      if (f.frameId === sourceFrameId) continue;
      if (
        clientX >= f.x &&
        clientX <= f.x + f.width &&
        clientY >= f.y &&
        clientY <= f.y + 52
      ) {
        return f;
      }
    }
    return null;
  }

  public moveFrame(frameId: string, targetX: number, targetY: number, applyMagnetSnap = true): void {
    const frame = this.frames.find((f) => f.frameId === frameId);
    if (!frame) return;

    const topMargin = this.getEffectiveTopMarginPx();
    let candidateX = targetX;
    let candidateY = targetY;

    if (applyMagnetSnap) {
      const xTargets: number[] = [0, this.viewportW - frame.width];
      const yTargets: number[] = [topMargin, this.viewportH - frame.height];
      for (const other of this.frames) {
        if (other.frameId === frame.frameId) continue;
        xTargets.push(other.x, other.x + other.width, other.x - frame.width);
        const otherBottom = other.y + (other.collapsed ? COLLAPSED_HEADER_HEIGHT_PX : other.height);
        yTargets.push(other.y, otherBottom, other.y - frame.height);
      }
      for (const tx of xTargets) {
        if (Math.abs(candidateX - tx) <= MAGNET_SNAP_THRESHOLD_PX) {
          candidateX = tx;
          break;
        }
      }
      for (const ty of yTargets) {
        if (Math.abs(candidateY - ty) <= MAGNET_SNAP_THRESHOLD_PX) {
          candidateY = ty;
          break;
        }
      }
    }

    const clamped = clampFramePosition(candidateX, candidateY, frame.width, this.viewportW, this.viewportH, topMargin);
    frame.x = clamped.x;
    frame.y = clamped.y;
    this.syncFrameGeometry(frame);
  }

  public resizeFrame(
    frameId: string,
    rect: { readonly x?: number; readonly y?: number; readonly width: number; readonly height: number },
  ): void {
    const frame = this.frames.find((f) => f.frameId === frameId);
    if (!frame) return;
    const topMargin = this.getEffectiveTopMarginPx();
    const bounds = this.computeFrameBounds(frame, topMargin);
    frame.width = Math.round(clamp(rect.width, bounds.minW, bounds.maxW));
    frame.height = Math.round(clamp(rect.height, bounds.minH, bounds.maxH));
    const clampedPos = clampFramePosition(
      rect.x ?? frame.x,
      rect.y ?? frame.y,
      frame.width,
      this.viewportW,
      this.viewportH,
      topMargin,
    );
    frame.x = clampedPos.x;
    frame.y = clampedPos.y;
    this.syncFrameGeometry(frame);
  }

  public focusFrame(frameId: string): void {
    const idx = this.frames.findIndex((f) => f.frameId === frameId);
    if (idx === -1) return;
    const [picked] = this.frames.splice(idx, 1);
    this.frames.push(picked);

    for (let i = 0; i < this.frames.length; i += 1) {
      const f = this.frames[i];
      f.zIndex = 100 + i;
      f.el.style.zIndex = String(f.zIndex);
      f.el.setAttribute("data-focused", i === this.frames.length - 1 ? "true" : "false");
    }
  }

  public toggleCollapse(frameId: string): boolean {
    const frame = this.frames.find((f) => f.frameId === frameId);
    if (!frame) return false;
    frame.collapsed = !frame.collapsed;
    frame.el.classList.toggle("is-collapsed", frame.collapsed);
    frame.collapseBtn.setAttribute("aria-expanded", frame.collapsed ? "false" : "true");
    frame.collapseBtn.textContent = frame.collapsed ? "+" : "-";
    this.emitPersistence();
    return frame.collapsed;
  }

  public togglePin(frameId: string): boolean {
    const frame = this.frames.find((f) => f.frameId === frameId);
    if (!frame) return false;
    const tab = frame.tabs[frame.activeTabIndex];
    if (!tab) return false;
    tab.pinned = !tab.pinned;
    this.syncFrameChrome(frame);
    this.emitPersistence();
    return tab.pinned;
  }

  public activateTab(frameId: string, tabIdOrIndex: string | number): void {
    const frame = this.frames.find((f) => f.frameId === frameId);
    if (!frame || frame.tabs.length === 0) return;
    const idx =
      typeof tabIdOrIndex === "number"
        ? clamp(tabIdOrIndex, 0, frame.tabs.length - 1)
        : frame.tabs.findIndex((t) => t.tabId === tabIdOrIndex);
    if (idx === -1) return;
    const changed = frame.activeTabIndex !== idx;
    frame.activeTabIndex = idx;
    this.syncFrameChrome(frame);
    if (changed) {
      this.spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
    }
  }

  public dockFrameInto(sourceFrameId: string, targetFrameId: string): boolean {
    if (sourceFrameId === targetFrameId) return false;
    const srcIdx = this.frames.findIndex((f) => f.frameId === sourceFrameId);
    const dst = this.frames.find((f) => f.frameId === targetFrameId);
    if (srcIdx === -1 || !dst) return false;
    const src = this.frames[srcIdx];

    while (src.tabs.length > 0) {
      const movedTab = src.tabs.shift()!;
      dst.tablistEl.appendChild(movedTab.tabBtn);
      dst.bodyEl.appendChild(movedTab.panelEl);
      dst.tabs.push(movedTab);
    }
    this.frames.splice(srcIdx, 1);
    src.el.remove();

    dst.activeTabIndex = dst.tabs.length - 1;
    if (dst.collapsed) this.toggleCollapse(dst.frameId);
    this.syncFrameChrome(dst);
    this.focusFrame(dst.frameId);
    this.spine.bus.emit({ kind: "PlayUiFoley", cueId: "window_open" });
    this.emitPersistence();
    return true;
  }

  public tearOffTab(
    sourceFrameId: string,
    tabIndex: number,
    dropX = 120,
    dropY = 120,
  ): string | null {
    const src = this.frames.find((f) => f.frameId === sourceFrameId);
    if (!src || src.tabs.length <= 1) return null;
    if (tabIndex < 0 || tabIndex >= src.tabs.length) return null;

    const [tab] = src.tabs.splice(tabIndex, 1);
    src.activeTabIndex = clamp(src.activeTabIndex, 0, src.tabs.length - 1);
    this.syncFrameChrome(src);

    const frameId = `win-${this.nextId}`;
    this.nextId += 1;
    const topMargin = this.getEffectiveTopMarginPx();
    const width = Math.max(tab.plugin.minWidth + 40, src.width);
    const height = Math.max(tab.plugin.minHeight + 40, src.height);
    const clampedPos = clampFramePosition(dropX, dropY, width, this.viewportW, this.viewportH, topMargin);

    const nextFrame = this.createFrameElement(frameId, clampedPos.x, clampedPos.y, width, height, false);
    nextFrame.tabs.push(tab);
    nextFrame.tablistEl.appendChild(tab.tabBtn);
    nextFrame.bodyEl.appendChild(tab.panelEl);
    nextFrame.activeTabIndex = 0;

    this.frames.push(nextFrame);
    this.layerEl.appendChild(nextFrame.el);
    this.syncFrameChrome(nextFrame);
    this.focusFrame(frameId);
    this.emitPersistence();
    return frameId;
  }

  public closeTab(frameId: string, tabIndex: number): void {
    const frame = this.frames.find((f) => f.frameId === frameId);
    if (!frame || tabIndex < 0 || tabIndex >= frame.tabs.length) return;
    const [removed] = frame.tabs.splice(tabIndex, 1);
    removed.disposeContent();
    removed.tabBtn.remove();
    removed.panelEl.remove();

    if (frame.tabs.length === 0) {
      const fIdx = this.frames.indexOf(frame);
      if (fIdx !== -1) this.frames.splice(fIdx, 1);
      frame.el.remove();
    } else {
      frame.activeTabIndex = clamp(frame.activeTabIndex, 0, frame.tabs.length - 1);
      this.syncFrameChrome(frame);
    }
    this.spine.bus.emit({ kind: "PlayUiFoley", cueId: "window_close" });
    this.emitPersistence();
  }

  public closeWindow(frameId: string): void {
    const idx = this.frames.findIndex((f) => f.frameId === frameId);
    if (idx === -1) return;
    const [removed] = this.frames.splice(idx, 1);
    for (const tab of removed.tabs) {
      tab.disposeContent();
    }
    removed.el.remove();
    this.spine.bus.emit({ kind: "PlayUiFoley", cueId: "window_close" });
    this.emitPersistence();
  }

  public closeAllWindows(): void {
    while (this.frames.length > 0) {
      const f = this.frames.pop()!;
      for (const tab of f.tabs) {
        tab.disposeContent();
      }
      f.el.remove();
    }
  }

  private syncFrameGeometry(frame: ManagedFrame): void {
    frame.el.style.left = `${frame.x}px`;
    frame.el.style.top = `${frame.y}px`;
    frame.el.style.width = `${frame.width}px`;
    frame.el.style.height = `${frame.height}px`;
  }

  private syncFrameChrome(frame: ManagedFrame): void {
    this.syncFrameGeometry(frame);
    const activeTab = frame.tabs[frame.activeTabIndex];
    const title = activeTab
      ? activeTab.pinned
        ? `${activeTab.plugin.defaultTitle} [Pinned]`
        : activeTab.plugin.defaultTitle
      : "Window";
    frame.titleTextEl.textContent = title;
    frame.dragHandleBtn.setAttribute("aria-label", `Move ${title}`);
    frame.el.setAttribute("aria-label", title);
    frame.pinBtn.setAttribute("aria-pressed", activeTab?.pinned ? "true" : "false");

    frame.tablistEl.hidden = frame.tabs.length <= 1;

    for (let i = 0; i < frame.tabs.length; i += 1) {
      const t = frame.tabs[i];
      const selected = i === frame.activeTabIndex;
      t.tabBtn.setAttribute("aria-selected", selected ? "true" : "false");
      t.tabBtn.tabIndex = selected ? 0 : -1;
      t.tabBtn.textContent = t.pinned
        ? `${t.plugin.defaultTitle} [Pinned]`
        : t.plugin.defaultTitle;

      if (selected) {
        t.panelEl.removeAttribute("hidden");
      } else {
        t.panelEl.setAttribute("hidden", "until-found");
      }
    }
  }

  public isPointOccluded(clientX: number, clientY: number): boolean {
    for (const f of this.frames) {
      const effH = f.collapsed ? COLLAPSED_HEADER_HEIGHT_PX : f.height;
      if (
        clientX >= f.x &&
        clientX <= f.x + f.width &&
        clientY >= f.y &&
        clientY <= f.y + effH
      ) {
        return true;
      }
    }

    if (typeof document.querySelectorAll === "function") {
      const occluders = document.querySelectorAll<HTMLElement>(".ng-hud-bar, [data-chrome-occluder='true'], dialog[open]");
      for (const el of Array.from(occluders)) {
        const rect = el.getBoundingClientRect();
        if (
          rect.width > 0 &&
          rect.height > 0 &&
          clientX >= rect.left &&
          clientX <= rect.right &&
          clientY >= rect.top &&
          clientY <= rect.bottom
        ) {
          return true;
        }
      }
    }
    return false;
  }

  public listFrames(): readonly FrameStateSnapshot[] {
    return this.frames.map((f, idx) => ({
      frameId: f.frameId,
      x: f.x,
      y: f.y,
      width: f.width,
      height: f.height,
      collapsed: f.collapsed,
      zIndex: f.zIndex,
      activeTabIndex: f.activeTabIndex,
      focused: idx === this.frames.length - 1,
      tabs: f.tabs.map((t) => ({
        tabId: t.tabId,
        pluginId: t.plugin.pluginId,
        instanceKey: t.instanceKey,
        pinned: t.pinned,
      })),
    }));
  }

  public getFrameSnapshot(frameId: string): FrameStateSnapshot | null {
    return this.listFrames().find((f) => f.frameId === frameId) ?? null;
  }

  public listPresetNames(): readonly string[] {
    return Array.from(this.namedPresets.keys());
  }

  public exportCurrentLayoutJson(): string {
    const serialized: SerializedWindowFrameDto[] = this.frames.map((f) => ({
      frameId: f.frameId,
      x: f.x,
      y: f.y,
      width: f.width,
      height: f.height,
      collapsed: f.collapsed,
      activeTabIndex: f.activeTabIndex,
      tabs: f.tabs.map((t) => ({
        pluginId: t.plugin.pluginId,
        instanceKey: t.instanceKey,
        pinned: t.pinned,
      })),
    }));
    return serializeLayout(serialized, this.viewportW, this.viewportH);
  }

  public saveNamedPreset(name: string): void {
    const trimmed = name.trim();
    if (!trimmed) return;
    this.namedPresets.set(trimmed, this.exportCurrentLayoutJson());
    this.emitPersistence();
  }

  public applyNamedPreset(name: string): boolean {
    const raw = this.namedPresets.get(name);
    if (!raw) return false;
    const ok = this.restoreFromJson(raw);
    if (ok) this.emitPersistence();
    return ok;
  }

  public restoreFromJson(rawJson: string): boolean {
    const topMargin = this.getEffectiveTopMarginPx();
    const doc = deserializeAndClampLayout(
      rawJson,
      this.viewportW,
      this.viewportH,
      (pluginId) => {
        const p = this.registry.getWindowPlugin(pluginId);
        return p
          ? {
              minWidth: p.minWidth,
              minHeight: p.minHeight,
              maxWidth: p.maxWidth,
              maxHeight: p.maxHeight,
            }
          : undefined;
      },
      topMargin,
    );
    if (!doc) return false;

    this.closeAllWindows();
    for (const fDto of doc.frames) {
      const validTabs = fDto.tabs.filter((t) => Boolean(this.registry.getWindowPlugin(t.pluginId)));
      if (validTabs.length === 0) continue;

      const frameId = `win-${this.nextId}`;
      this.nextId += 1;
      const frame = this.createFrameElement(
        frameId,
        fDto.x,
        fDto.y,
        fDto.width,
        fDto.height,
        fDto.collapsed,
      );
      for (const tDto of validTabs) {
        const plugin = this.registry.getWindowPlugin(tDto.pluginId)!;
        frame.tabs.push(this.createTabForPlugin(frame, plugin, tDto.instanceKey, tDto.pinned));
      }
      frame.activeTabIndex = clamp(fDto.activeTabIndex, 0, frame.tabs.length - 1);
      this.frames.push(frame);
      this.layerEl.appendChild(frame.el);
      this.syncFrameChrome(frame);
    }

    if (this.frames.length > 0) {
      this.focusFrame(this.frames[this.frames.length - 1].frameId);
    }
    return true;
  }

  public handleViewportResize(newWidth: number, newHeight: number): void {
    const prevJson = this.exportCurrentLayoutJson();
    this.viewportW = Math.max(320, newWidth);
    this.viewportH = Math.max(240, newHeight);
    if (this.frames.length > 0) {
      this.restoreFromJson(prevJson);
    }
  }

  public dispose(): void {
    this.closeAllWindows();
    while (this.cleanups.length > 0) {
      this.cleanups.pop()?.();
    }
    this.layerEl.remove();
    this.announcerEl.remove();
  }
}
