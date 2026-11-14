# Module Design: Desktop Window Manager, Docking & Layout Persistence

**Module ID:** `UI-WIN-04`  
**Code Location:** `apps/ui-shell/src/windows/`  
**Milestone & Design Depth:** `M2: Full Detailed Design`  
**Satisfies Requirements:** `UI-WIN-001`, `UI-WIN-002`, `UI-WIN-003`, `UI-WIN-004`, `UI-WIN-005`, `UI-WIN-006`, `UI-WIN-007` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `WindowContentRegistry` (mounted `WindowContentPlugin` providers from `views/`, `dispatches/`, `hud/`)
  - `UiIntentBus` (`OpenSystemWindow`, `FocusEntityInspector`, `CloseWindow`)
  - Saved JSON layout presets from user profile storage
- **Provided Spine Ports (Outputs):**
  - Floating, draggable, resizable, collapsible, z-ordered, snappable, tab-docked DOM window containers
  - Emits `PlayUiFoley` events on open, close, snap, tab switch, and collapse transitions
  - Persists serialized `WorkspaceLayoutSnapshot` JSON

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Content-Agnostic `WindowManagerState`

```typescript
export interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowInstance {
  windowId: string;
  pluginId: string;
  instanceKey: string;      // Entity scope tag: "settlement:42" or "global"
  title: string;
  bounds: WindowBounds;
  minWidth: number;
  minHeight: number;
  maxWidth: number;
  maxHeight: number;
  zIndex: number;
  collapsed: boolean;       // True shrinks body to title bar without unmounting DOM state!
  pinned: boolean;          // False reuses single inspector slot; True opens side-by-side copy
  dockGroupId: string | null;
  activeInGroup: boolean;
}

export interface DockTabGroup {
  groupId: string;
  windowIds: string[];      // Ordered tabs sharing one frame geometry
  activeWindowId: string;
  bounds: WindowBounds;
  collapsed: boolean;
  zIndex: number;
}
```

---

## 3. Algorithms, Pointer Isolation & Window Geometry

### 3.1 Viewport-Clamped Dragging (`UI-WIN-001`)
1. Pointer down on `.win-titlebar` calls `setPointerCapture(pointerId)`, raises z-index, and stores grab offset `(pointerX - bounds.x, pointerY - bounds.y)`.
2. Pointer move updates position while strictly clamping `x` in `[-(bounds.width - 96), viewportW - 96]` and `y` in `[hudTopBarHeight, viewportH - titleBarHeight]` so every title bar and close/restore button remains reachable on screen regardless of drag extremes.

### 3.2 Eight-Way Edge & Corner Resizing (`UI-WIN-002`)
1. Invisible hit-target strips (`6px` edges, `12px` corners) surround the CSS 9-slice frame with appropriate CSS cursors (`ns-resize`, `ew-resize`, `nwse-resize`, `nesw-resize`).
2. Dragging west or north edges shifts origin `(x, y)` simultaneously with `(width, height)` while clamping dimensions strictly to `[minWidth..maxWidth]` and `[minHeight..maxHeight]` before applying origin shifts so opposite edges stay anchored without jitter.

### 3.3 State-Preserving Minimize/Collapse & Pinned Comparison (`UI-WIN-003`, `UI-WIN-004`)
1. Toggling `collapsed = true` applies CSS class `.is-collapsed` hiding the `.win-body` container (`height: 0; overflow: hidden; visibility: hidden;`) **without unmounting child DOM nodes**—preserving virtualized table scrollTop offsets, expanded org-tree branches, and text filter inputs identically upon restore.
2. On `FocusEntityInspector`:
   - If `!intent.pinned` and an unpinned window of `pluginId` already exists, retarget its `instanceKey` and raise z-index.
   - If `intent.pinned` (or existing window was pinned by user toggle), spawn a fresh `WindowInstance` cascaded `+28px, +28px` for side-by-side settlement/leader comparison.

### 3.4 Z-Order Focus & Strict Input Capture (`UI-WIN-005`)
1. Any `pointerdown`, `mousedown`, `click`, or `wheel` event inside a window calls `raiseWindow(windowId)` (monotonic `zCounter += 1`, re-normalizing when approaching safe integer bounds) and invokes `event.stopPropagation()` on the window root container so globe orbit/zoom handlers below never receive leaked events.

### 3.5 Edge Snapping, Tab Docking & Layout Serialization (`UI-WIN-006`, `UI-WIN-007`)
1. **Magnetic Snap (`threshold = 12px`):** While dragging or resizing, candidate edges snap softly to viewport margins and adjacent top-level window outer borders (`Shift` temporarily bypasses snapping).
2. **Tab-Group Docking:** Dropping a dragged window header onto another window's title-bar dropzone merges both into a `DockTabGroup` sharing unified chrome and clickable tab pills; dragging a tab `> 24px` outside the tab strip detaches it back into a standalone floating window.
3. **Layout Presets:** Saves normalized `(x / viewportW, y / viewportH, width, height, collapsed, pinned, dockGroups)` into named presets; loading on a smaller monitor clamps dimensions proportionally inside safe bounds.

---

## 4. Isolated Cartridge Verification Strategy

1. **Clamped Drag & 8-Way Resize (`@req UI-WIN-001`, `@req UI-WIN-002`):** Playwright drags windows off all 4 screen edges confirming title bar accessibility, and exercises all 8 resize handles verifying min/max size clamping.
2. **Collapse State Preservation (`@req UI-WIN-003`):** Scrolls a virtual table inside Window A, collapses to header, restores, and asserts identical `scrollTop` and input text.
3. **Pinned Multi-Window & Event Isolation (`@req UI-WIN-004`, `@req UI-WIN-005`):** Pins Settlement 1, inspects Settlement 2 to assert 2 simultaneous windows, scrolls wheel over overlapped upper window, and asserts globe camera altitude stays constant.
4. **Tab Docking & Preset Round-Trip (`@req UI-WIN-006`, `@req UI-WIN-007`):** Playwright merges Windows A & B into tabs, switches tabs, detaches B back out, saves layout JSON, resizes viewport to `1280x720`, restores layout, and asserts zero out-of-bounds windows.

---

## 5. Lift-and-Shift Replacement Guide

1. The window manager knows zero domain concepts about stockpiles, troops, or charts; every interior view renders via `WindowContentPlugin::mountContent(...)`.
2. Replacing floating windows with tiling binary-split panes or changing docking ergonomics requires modifying strictly `apps/ui-shell/src/windows/`.
