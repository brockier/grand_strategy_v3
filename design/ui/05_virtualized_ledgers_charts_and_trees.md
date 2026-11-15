# Module Design: Virtualized Ledgers, Charts & Delegation Org Trees

**Module ID:** `UI-DAT-05`  
**Code Location:** `code/ui/src/views/`  
**Milestone & Design Depth:** `M3–M4: Full Detailed Design` + `M5`/`M6: Locked WindowContentPlugin Slots`  
**Satisfies Requirements:** `UI-DAT-001`, `UI-DAT-002`, `UI-DAT-003`, `UI-DAT-006`, `UI-DAT-007` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`selectLedgerRows`, `selectTimeSeries`, `selectDelegationTree`)
  - `AssetUrlResolverPort` (commodity and portrait icon URLs)
  - Epistemic provenance badge helper contract from `UI-SPINE-00`
- **Provided Spine Ports (Outputs):**
  - Registered `WindowContentPlugin` implementations mounted by `WindowContentRegistry`:
    1. `StockpileAndCohortLedgerView` (`UI-DAT-001`)
    2. `HistoricalMetricsChartView` (`UI-DAT-002`)
    3. `CommandDelegationOrgTreeView` (`UI-DAT-003`)
    4. `NonStateEconomyInspectorView` (`UI-DAT-006`, `M5` slot)
    5. `DiegeticKnowledgeArchiveView` (`UI-DAT-007`, `M6` slot)
  - Emits `FocusEntityInspector` and `FlyGlobeToCell` intents onto `UiIntentBus`

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Zero-Dependency Row-Virtualized Table (`UI-DAT-001`)

To comply strictly with `UI-LIC-001` (zero paid/commercial data-grid dependencies) while rendering 10,000+ rows cleanly:

```typescript
interface VirtualTableState<TRow> {
  rawRows: readonly TRow[];
  filteredIndices: Uint32Array;     // Sorted and facet-filtered row pointer array
  sortColumn: string;
  sortDirection: "asc" | "desc";
  textFilter: string;
  facetFilters: Record<string, string>;
  rowHeightPx: number;
  scrollTopPx: number;
  viewportHeightPx: number;
  overscanCount: number;            // Fixed 8 buffer rows above and below viewport
}
```

### 2.2 Permissive Time-Series Chart Engine (`UI-DAT-002`)

```typescript
interface ChartViewportState {
  series: readonly ChartSeries[]; // Color resolved via CSS custom property token, never hardcoded!
  tickStart: number;
  tickEnd: number;
  hoverTick: number | null;
  hiddenSeriesIds: Set<string>;
}
```

### 2.3 Delegation Org Tree Model (`UI-DAT-003`)

Hierarchical tree layout storing collapsed branch sets, node coordinates `(x, y)` computed via Reingold-Tilford / simple tidy tree layout, per-leader Attention progress (`currentLoad / capacity`), and `oversightDilution01` severity badge.

---

## 3. Algorithms & High-Density Rendering Mechanics

### 3.1 `O(visible)` DOM Virtualization (`UI-DAT-001`)
1. When dataset or filter/sort changes, sort and filter `filteredIndices` in tight typed-array passes (`< 50ms` even at `25,000` rows).
2. Calculate `firstVisible = Math.max(0, Math.floor(scrollTopPx / rowHeightPx) - overscanCount)` and `lastVisible = Math.min(filteredIndices.length, Math.ceil((scrollTopPx + viewportHeightPx) / rowHeightPx) + overscanCount)`.
3. Render strictly `lastVisible - firstVisible` DOM row nodes inside a container sized `height = filteredIndices.length * rowHeightPx` positioned with `transform: translateY(firstVisible * rowHeightPx)`. Total live DOM `<tr>`/row nodes stay strictly under `150` regardless of whether the colony ledger holds `100` or `50,000` batches.
4. Numeric columns displaying remote estimates render explicit confidence intervals `[min .. max]` plus stale-age badges (`UI-DAT-005`).

### 3.2 Canvas/SVG Multi-Series Chart (`UI-DAT-002`)
1. Uses a resize-observed high-DPI HTML5 `<canvas>` plot layer paired with an accessible SVG/HTML legend styled exclusively by `--skin-chart-series-1..N` CSS tokens (`UI-SKN-001`).
2. Downsamples series exceeding pixel width (`> 2,000` data points) using Largest-Triangle-Three-Buckets (`LTTB`) so zoomable multi-decade stockpile or population histories draw smoothly in `< 2ms` per frame.

### 3.3 Org Tree Attention Visualization (`UI-DAT-003`)
1. Draws SVG orthogonal parent-child links from Player HQ down through appointed overseers and active mandates.
2. Each leader card shows: portrait thumbnail, Attention load meter (`4.5 / 5.0`), active mandate count, and visual dilution alerts whenever `currentLoad > capacity` or a subordinate is out of communication range.

### 3.4 Locked `M5`/`M6` Inspector Plugin Slots (`UI-DAT-006`, `UI-DAT-007`)
`NonStateEconomyInspectorView` (`M5`) and `DiegeticKnowledgeArchiveView` (`M6`) register clean plugin descriptors bound to typed optional selectors on `DiegeticStore`. Prior to `M5`/`M6`, opening them displays an informational scenario-feature-inactive notice; once `M5`/`M6` snapshots populate, they mount rich virtual tables for non-state institutions and empirical material assays without touching `WindowManager`.

---

## 4. Isolated Cartridge Verification Strategy

1. **15,000-Row Virtualization Benchmark (`@req UI-DAT-001`):** Playwright loads a `15,000`-row ledger fixture, asserts live DOM row nodes `< 150`, measures sort duration `< 50ms`, and scrolls rapidly verifying zero dropped frames.
2. **Chart Resize & Tooltip Accuracy (`@req UI-DAT-002`):** Vitest asserts LTTB scaling bounds, crosshair nearest-tick readout, and non-overlapping legend sizing across narrow and wide windows.
3. **Org Tree Dilution Rendering (`@req UI-DAT-003`):** Feeds snapshot fixture exceeding leader Attention capacity and confirms dilution warning attributes appear on the affected node and child mandates.

---

## 5. Lift-and-Shift Replacement Guide

1. Every widget (`VirtualLedgerTable`, `TimeSeriesChart`, `DelegationOrgTree`) registers independently as a `WindowContentPlugin` inside `code/ui/src/views/`.
2. You can replace the custom Canvas chart with any permissive (`MIT`/`Apache-2.0`) charting library or swap the org-tree layout algorithm completely inside `src/views/` with zero impact on `src/windows/` or `src/globe/`.
