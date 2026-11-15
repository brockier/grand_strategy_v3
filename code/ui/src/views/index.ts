import {
  DiegeticSnapshot,
  KnownCellDto,
  ReportedLedgerRowDto,
  TimeSeriesDto,
  WindowContentPlugin,
  createProvenanceBadgeElement,
  selectAuditDiscrepancies,
  selectDelegationTree,
  selectFacilityRows,
  selectHqPresence,
  selectKnownCells,
  selectKnownSettlements,
  selectLedgerRows,
  selectPopulationRows,
  selectTimeSeries,
  selectTrackedConvoys,
} from "../spine";

type LedgerTabKind = "commodities" | "cohorts" | "convoys" | "facilities";
type SortDir = "ascending" | "descending";

interface NormalizedLedgerRow {
  readonly id: string;
  readonly col0Text: string;
  readonly col0IconKey?: string;
  readonly col1Text: string;
  readonly targetCellId: number | null;
  readonly focusKind: "settlement" | "convoy" | "facility";
  readonly focusEntityId: number;
  readonly primaryNum: number;
  readonly primaryLabel: string;
  readonly secondaryText: string;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
  readonly reporterName?: string;
  readonly isLiveSight?: boolean;
}

export interface ChartLegendItemLayout {
  readonly seriesKey: string;
  readonly label: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface ChartPlotLayout {
  readonly width: number;
  readonly height: number;
  readonly padLeft: number;
  readonly padRight: number;
  readonly padTop: number;
  readonly padBottom: number;
  readonly plotWidth: number;
  readonly plotHeight: number;
  readonly minTick: number;
  readonly maxTick: number;
  readonly minVal: number;
  readonly maxVal: number;
  readonly legendItems: readonly ChartLegendItemLayout[];
}

const LEDGER_ROW_HEIGHT_PX = 32;
const LEDGER_OVERSCAN_ROWS = 6;

function wireImageWithFallback(
  img: HTMLImageElement,
  primaryUrl: string,
  fallbackUrl: string,
  alt: string,
): void {
  img.alt = alt;
  let usedFallback = false;
  img.addEventListener("error", () => {
    if (!usedFallback && fallbackUrl && img.src !== fallbackUrl) {
      usedFallback = true;
      img.src = fallbackUrl;
    }
  });
  img.src = primaryUrl || fallbackUrl;
}

function createClickableToken(
  doc: Document,
  label: string,
  onClick: () => void,
): HTMLButtonElement {
  const btn = doc.createElement("button");
  btn.type = "button";
  btn.className = "ng-dispatch-token";
  btn.textContent = label;
  btn.addEventListener("click", onClick);
  return btn;
}

function buildNormalizedLedgerRows(
  tab: LedgerTabKind,
  snap: DiegeticSnapshot | null,
): NormalizedLedgerRow[] {
  if (!snap) return [];
  const settlementCellById = new Map<number, number>();
  for (const s of snap.knownSettlements ?? []) {
    settlementCellById.set(s.settlementId, s.cellId);
  }
  const currentTick = snap.clockAndNotifications.tick;
  const hqSettlementName = snap.hqPresence.settlementName;

  if (tab === "commodities") {
    return snap.ledgerRows.map((r: ReportedLedgerRowDto, idx) => ({
      id: `c-${idx}-${r.settlementId}-${r.commodityKey}`,
      col0Text: r.commodityName,
      col0IconKey: r.commodityKey,
      col1Text: r.settlementName,
      targetCellId: settlementCellById.get(r.settlementId) ?? null,
      focusKind: "settlement",
      focusEntityId: r.settlementId,
      primaryNum: r.reportedMassKg,
      primaryLabel: `${Math.round(r.reportedMassKg)} kg [${Math.round(r.minEstKg)}..${Math.round(r.maxEstKg)}]`,
      secondaryText: r.category ?? "physical",
      observedTick: r.observedTick,
      confidence01: r.confidence01 ?? (r.settlementName === hqSettlementName ? 0.99 : 0.72),
      provenanceLabel: r.provenanceLabel,
      reporterName: r.reporterName,
      isLiveSight:
        r.observedTick >= currentTick &&
        (r.confidence01 ?? 0.99) >= 0.95 &&
        r.settlementName === hqSettlementName,
    }));
  }

  if (tab === "cohorts") {
    return (snap.populationRows ?? []).map((p) => ({
      id: `p-${p.cohortId}`,
      col0Text: p.vocationName,
      col1Text: p.settlementName,
      targetCellId: settlementCellById.get(p.settlementId) ?? null,
      focusKind: "settlement",
      focusEntityId: p.settlementId,
      primaryNum: p.reportedHeadcount,
      primaryLabel: `${p.reportedHeadcount} [${p.minHeadcount}..${p.maxHeadcount}]`,
      secondaryText: `Skill ${Math.round(p.meanSkill01 * 100)}% · ${p.unrestStage}`,
      observedTick: p.observedTick,
      confidence01: p.confidence01,
      provenanceLabel: p.provenanceLabel,
      isLiveSight: p.observedTick >= currentTick && p.confidence01 >= 0.95,
    }));
  }

  if (tab === "convoys") {
    return snap.trackedConvoys.map((c) => ({
      id: `v-${c.convoyId}`,
      col0Text: c.label,
      col1Text: `Cell #${c.projectedCellId}`,
      targetCellId: c.projectedCellId,
      focusKind: "convoy",
      focusEntityId: c.convoyId,
      primaryNum: c.expectedArrivalTick ?? 0,
      primaryLabel: `${c.status} (ETA t${c.expectedArrivalTick ?? 0})`,
      secondaryText: c.channel ?? "dead-reckoning",
      observedTick: c.departureTick ?? currentTick,
      confidence01: c.confidence01 ?? 0.65,
      provenanceLabel: c.provenanceLabel ?? "Logistics Track",
      isLiveSight: c.status.toLowerCase().includes("confirmed"),
    }));
  }

  return (snap.facilityRows ?? []).map((f) => ({
    id: `f-${f.facilityId}`,
    col0Text: f.processName,
    col1Text: f.settlementName,
    targetCellId: settlementCellById.get(f.settlementId) ?? null,
    focusKind: "facility",
    focusEntityId: f.facilityId,
    primaryNum: f.assignedWorkers,
    primaryLabel: `${f.assignedWorkers} workers · tools ${Math.round(f.toolCondition01 * 100)}%`,
    secondaryText: f.weatherHalted
      ? `Halted (flood ${Math.round(f.shaftWater01 * 100)}%)`
      : `Active (water ${Math.round(f.shaftWater01 * 100)}%)`,
    observedTick: f.observedTick,
    confidence01: 0.9,
    provenanceLabel: f.provenanceLabel,
    isLiveSight: f.observedTick >= currentTick && f.settlementName === hqSettlementName,
  }));
}

/**
 * Row-virtualized, sortable, and filterable Reported Ledger supporting 15,000+ rows
 * with strict bounded DOM row rendering and provenance badging (`UI-DAT-001`, `UI-DAT-005`, `UI-WIN-003`).
 */
export function createLedgerStubPlugin(): WindowContentPlugin {
  return {
    pluginId: "physical_ledger",
    defaultTitle: "Observed Physical Ledger",
    minWidth: 360,
    minHeight: 240,
    hotkey: "e",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const tabBar = document.createElement("div");
      tabBar.className = "ng-hud-cluster";
      tabBar.setAttribute("role", "group");
      tabBar.setAttribute("aria-label", "Ledger Dataset");

      let activeTab: LedgerTabKind = "commodities";
      let sortCol = 0;
      let sortDir: SortDir = "ascending";

      const tabDefs: ReadonlyArray<{ readonly id: LedgerTabKind; readonly label: string }> = [
        { id: "commodities", label: "Commodities" },
        { id: "cohorts", label: "Cohorts" },
        { id: "convoys", label: "Convoys" },
        { id: "facilities", label: "Facilities" },
      ];

      const tabButtons = new Map<LedgerTabKind, HTMLButtonElement>();
      for (const td of tabDefs) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "ng-btn";
        btn.dataset.ledgerTab = td.id;
        btn.textContent = td.label;
        tabButtons.set(td.id, btn);
        tabBar.appendChild(btn);
      }

      const controlsRow = document.createElement("div");
      controlsRow.className = "ng-hud-cluster";

      const filterInput = document.createElement("input");
      filterInput.type = "search";
      filterInput.className = "ng-text-input";
      filterInput.setAttribute("data-ledger-filter", "true");
      filterInput.placeholder = "Filter commodities, settlements, or sources...";
      filterInput.setAttribute("aria-label", "Filter ledger rows");

      const countStatus = document.createElement("span");
      countStatus.className = "ng-hud-stat";
      countStatus.setAttribute("aria-live", "polite");

      controlsRow.append(filterInput, countStatus);

      const viewport = document.createElement("div");
      viewport.className = "ng-virtual-viewport";
      viewport.setAttribute("data-ledger-scroll", "true");
      viewport.setAttribute("role", "region");
      viewport.tabIndex = 0;
      viewport.setAttribute("aria-label", "Reported Ledger Data Grid");

      const table = document.createElement("table");
      table.className = "ng-ledger-table";

      const thead = document.createElement("thead");
      const headerTr = document.createElement("tr");
      const colHeaders = ["Subject", "Hub / Location", "Reported Quantity", "Detail", "Provenance"];
      const thElements: HTMLTableCellElement[] = [];
      const sortButtons: HTMLButtonElement[] = [];

      for (let idx = 0; idx < colHeaders.length; idx += 1) {
        const th = document.createElement("th");
        th.scope = "col";
        th.setAttribute("aria-sort", idx === sortCol ? sortDir : "none");

        const sBtn = document.createElement("button");
        sBtn.type = "button";
        sBtn.className = "ng-sort-btn";
        sBtn.dataset.sortCol = String(idx);
        const labelSpan = document.createElement("span");
        labelSpan.textContent = colHeaders[idx];
        const indicator = document.createElement("span");
        indicator.setAttribute("aria-hidden", "true");
        indicator.textContent = idx === sortCol ? " ▲" : " ↕";
        sBtn.append(labelSpan, indicator);

        sBtn.addEventListener("click", () => {
          if (sortCol === idx) {
            sortDir = sortDir === "ascending" ? "descending" : "ascending";
          } else {
            sortCol = idx;
            sortDir = idx === 2 ? "descending" : "ascending";
          }
          recomputeRows();
          spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
        });

        th.appendChild(sBtn);
        headerTr.appendChild(th);
        thElements.push(th);
        sortButtons.push(sBtn);
      }
      thead.appendChild(headerTr);

      const tbody = document.createElement("tbody");
      table.append(thead, tbody);
      viewport.appendChild(table);

      let sortedRows: NormalizedLedgerRow[] = [];

      const updateHeaderLabelsAndAria = () => {
        const labelsByTab: Record<LedgerTabKind, readonly string[]> = {
          commodities: ["Commodity", "Settlement", "Stockpile Mass", "Category", "Provenance"],
          cohorts: ["Vocation", "Settlement", "Headcount", "Skill & Unrest", "Provenance"],
          convoys: ["Convoy", "Projected Cell", "Status & ETA", "Channel", "Provenance"],
          facilities: ["Facility Process", "Settlement", "Staffing & Tools", "State", "Provenance"],
        };
        const titles = labelsByTab[activeTab];
        for (let i = 0; i < thElements.length; i += 1) {
          thElements[i].setAttribute("aria-sort", i === sortCol ? sortDir : "none");
          const spans = sortButtons[i].querySelectorAll("span");
          if (spans[0]) spans[0].textContent = titles[i];
          if (spans[1]) {
            spans[1].textContent = i === sortCol ? (sortDir === "ascending" ? " ▲" : " ▼") : " ↕";
          }
        }
        for (const [id, btn] of tabButtons) {
          btn.setAttribute("aria-pressed", id === activeTab ? "true" : "false");
          btn.classList.toggle("is-active", id === activeTab);
        }
      };

      const renderVirtualSlice = () => {
        const snap = spine.store.getSnapshot();
        const currentTick = snap?.clockAndNotifications.tick ?? 0;
        const total = sortedRows.length;

        if (total === 0) {
          const emptyTr = document.createElement("tr");
          emptyTr.className = "ng-virtual-row";
          const emptyTd = document.createElement("td");
          emptyTd.colSpan = 5;
          emptyTd.textContent = "No matching reported ledger entries.";
          emptyTr.appendChild(emptyTd);
          tbody.replaceChildren(emptyTr);
          return;
        }

        const scrollTop = Math.max(0, viewport.scrollTop);
        const viewHeight = viewport.clientHeight > 0 ? viewport.clientHeight : 320;
        const firstVisible = Math.floor(scrollTop / LEDGER_ROW_HEIGHT_PX);
        const start = Math.max(0, firstVisible - LEDGER_OVERSCAN_ROWS);
        const visibleSpan = Math.ceil(viewHeight / LEDGER_ROW_HEIGHT_PX) + LEDGER_OVERSCAN_ROWS * 2;
        const end = Math.min(total, start + visibleSpan);

        const frag = document.createDocumentFragment();
        if (start > 0) {
          const topSpacer = document.createElement("tr");
          topSpacer.setAttribute("data-virtual-spacer", "top");
          const td = document.createElement("td");
          td.colSpan = 5;
          td.style.height = `${start * LEDGER_ROW_HEIGHT_PX}px`;
          td.style.padding = "0";
          td.style.border = "0";
          topSpacer.appendChild(td);
          frag.appendChild(topSpacer);
        }

        const fallbackIcon = spine.assetResolver?.resolveIcon("__fallback__") ?? "";

        for (let i = start; i < end; i += 1) {
          const row = sortedRows[i];
          const tr = document.createElement("tr");
          tr.className = "ng-virtual-row ng-ledger-row";
          tr.dataset.rowId = row.id;

          const td0 = document.createElement("td");
          if (row.col0IconKey) {
            const img = document.createElement("img");
            img.className = "ng-commodity-icon";
            const primaryUrl = spine.assetResolver?.resolveIcon(row.col0IconKey) ?? "";
            wireImageWithFallback(img, primaryUrl, fallbackIcon, row.col0Text);
            td0.append(img, document.createTextNode(` ${row.col0Text}`));
          } else {
            td0.textContent = row.col0Text;
          }

          const td1 = document.createElement("td");
          if (row.targetCellId !== null) {
            const targetCellId = row.targetCellId;
            const linkBtn = createClickableToken(document, row.col1Text, () => {
              spine.bus.emit({ kind: "FlyGlobeToCell", cellId: targetCellId });
              spine.bus.emit({
                kind: "FocusEntityInspector",
                entityKind: row.focusKind,
                entityId: row.focusEntityId,
              });
              spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
            });
            td1.appendChild(linkBtn);
          } else {
            td1.textContent = row.col1Text;
          }

          const td2 = document.createElement("td");
          td2.textContent = row.primaryLabel;

          const td3 = document.createElement("td");
          td3.textContent = row.secondaryText;

          const td4 = document.createElement("td");
          const badge = createProvenanceBadgeElement(document, {
            currentTick,
            observedTick: row.observedTick,
            confidence01: row.confidence01,
            provenanceLabel: row.provenanceLabel,
            reporterName: row.reporterName,
            isLiveSight: row.isLiveSight,
          });
          td4.appendChild(badge);

          tr.append(td0, td1, td2, td3, td4);
          frag.appendChild(tr);
        }

        if (end < total) {
          const bottomSpacer = document.createElement("tr");
          bottomSpacer.setAttribute("data-virtual-spacer", "bottom");
          const td = document.createElement("td");
          td.colSpan = 5;
          td.style.height = `${(total - end) * LEDGER_ROW_HEIGHT_PX}px`;
          td.style.padding = "0";
          td.style.border = "0";
          bottomSpacer.appendChild(td);
          frag.appendChild(bottomSpacer);
        }

        tbody.replaceChildren(frag);
      };

      const recomputeRows = () => {
        updateHeaderLabelsAndAria();
        const snap = spine.store.getSnapshot();
        const raw = buildNormalizedLedgerRows(activeTab, snap);
        const query = filterInput.value.trim().toLowerCase();
        const filtered = query
          ? raw.filter(
              (r) =>
                r.col0Text.toLowerCase().includes(query) ||
                r.col1Text.toLowerCase().includes(query) ||
                r.secondaryText.toLowerCase().includes(query) ||
                r.provenanceLabel.toLowerCase().includes(query),
            )
          : raw;

        const dirSign = sortDir === "ascending" ? 1 : -1;
        filtered.sort((a, b) => {
          if (sortCol === 0) return a.col0Text.localeCompare(b.col0Text) * dirSign;
          if (sortCol === 1) return a.col1Text.localeCompare(b.col1Text) * dirSign;
          if (sortCol === 2) return (a.primaryNum - b.primaryNum) * dirSign;
          if (sortCol === 3) return a.secondaryText.localeCompare(b.secondaryText) * dirSign;
          return (a.observedTick - b.observedTick) * dirSign;
        });

        sortedRows = filtered;
        countStatus.textContent = `${filtered.length} records`;
        renderVirtualSlice();
      };

      for (const [id, btn] of tabButtons) {
        btn.addEventListener("click", () => {
          if (activeTab === id) return;
          activeTab = id;
          viewport.scrollTop = 0;
          recomputeRows();
          spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
        });
      }

      filterInput.addEventListener("input", () => {
        viewport.scrollTop = 0;
        recomputeRows();
      });
      viewport.addEventListener("scroll", renderVirtualSlice);

      recomputeRows();
      root.append(tabBar, controlsRow, viewport);
      container.replaceChildren(root);

      const unsubStore = spine.store.subscribe(recomputeRows);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") renderVirtualSlice();
      });
      return () => {
        unsubStore();
        unsubBus();
      };
    },
  };
}

export const createVirtualizedLedgerWindowPlugin = createLedgerStubPlugin;

/**
 * Computes responsive axis bounds and non-overlapping legend item coordinates
 * for `TimeSeriesChartWindow` (`UI-DAT-002`).
 */
export function computeChartLayout(
  seriesList: readonly TimeSeriesDto[],
  width: number,
  height: number,
  tickWindowLimit: number | null = null,
): ChartPlotLayout {
  const safeW = Math.max(240, width);
  const safeH = Math.max(160, height);
  const padLeft = 54;
  const padRight = 18;
  const padBottom = 28;

  // Layout non-overlapping horizontal legend rows before plotting area begins
  const legendItems: ChartLegendItemLayout[] = [];
  let cursorX = padLeft;
  let cursorY = 6;
  const rowH = 18;
  for (const s of seriesList) {
    const itemW = Math.min(safeW - padLeft - padRight, Math.max(88, s.label.length * 7.2 + 22));
    if (cursorX + itemW > safeW - padRight && cursorX > padLeft) {
      cursorX = padLeft;
      cursorY += rowH;
    }
    legendItems.push({
      seriesKey: s.seriesKey,
      label: s.label,
      x: cursorX,
      y: cursorY,
      width: itemW,
      height: 16,
    });
    cursorX += itemW + 10;
  }
  const padTop = Math.max(32, cursorY + rowH + 10);
  const plotWidth = Math.max(40, safeW - padLeft - padRight);
  const plotHeight = Math.max(40, safeH - padTop - padBottom);

  let maxTick = 1;
  let minTick = Number.POSITIVE_INFINITY;
  let minVal = Number.POSITIVE_INFINITY;
  let maxVal = Number.NEGATIVE_INFINITY;

  for (const s of seriesList) {
    for (const pt of s.points) {
      maxTick = Math.max(maxTick, pt.tick);
      minTick = Math.min(minTick, pt.tick);
    }
  }
  if (!Number.isFinite(minTick)) minTick = 0;
  if (tickWindowLimit !== null && tickWindowLimit > 0 && maxTick - minTick > tickWindowLimit) {
    minTick = maxTick - tickWindowLimit;
  }

  for (const s of seriesList) {
    for (const pt of s.points) {
      if (pt.tick < minTick) continue;
      minVal = Math.min(minVal, pt.minBound, pt.value);
      maxVal = Math.max(maxVal, pt.maxBound, pt.value);
    }
  }
  if (!Number.isFinite(minVal) || !Number.isFinite(maxVal)) {
    minVal = 0;
    maxVal = 100;
  }
  if (Math.abs(maxVal - minVal) < 1e-6) {
    minVal = Math.max(0, minVal - 1);
    maxVal = maxVal + 1;
  }
  if (maxTick <= minTick) {
    maxTick = minTick + 1;
  }

  return {
    width: safeW,
    height: safeH,
    padLeft,
    padRight,
    padTop,
    padBottom,
    plotWidth,
    plotHeight,
    minTick,
    maxTick,
    minVal,
    maxVal,
    legendItems,
  };
}

/**
 * Pure Canvas 2D multi-series Time-Series Chart with uncertainty bounds,
 * interactive zoom spans, hover crosshairs, and external CSS theme tokens (`UI-DAT-002`).
 */
export function createTimeSeriesChartWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "time_series_chart",
    defaultTitle: "Historical Curves & Trends",
    minWidth: 420,
    minHeight: 280,
    hotkey: "t",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const toolbar = document.createElement("div");
      toolbar.className = "ng-hud-cluster";

      let tickWindowLimit: number | null = null;
      const hiddenSeries = new Set<string>();
      let hoverTick: number | null = null;

      const spanOptions: ReadonlyArray<{ readonly label: string; readonly ticks: number | null }> = [
        { label: "All History", ticks: null },
        { label: "Last 72h", ticks: 72 },
        { label: "Last 24h", ticks: 24 },
      ];
      const spanButtons = new Map<string, HTMLButtonElement>();
      for (const opt of spanOptions) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "ng-btn";
        btn.dataset.chartSpan = opt.ticks === null ? "all" : String(opt.ticks);
        btn.setAttribute("aria-pressed", opt.ticks === null ? "true" : "false");
        btn.textContent = opt.label;
        btn.addEventListener("click", () => {
          tickWindowLimit = opt.ticks;
          for (const [k, b] of spanButtons) {
            b.setAttribute("aria-pressed", k === btn.dataset.chartSpan ? "true" : "false");
          }
          drawChart();
        });
        spanButtons.set(btn.dataset.chartSpan, btn);
        toolbar.appendChild(btn);
      }

      const seriesFilterRow = document.createElement("div");
      seriesFilterRow.className = "ng-hud-cluster";

      const wrap = document.createElement("div");
      wrap.className = "ng-chart-canvas-wrap";

      const canvas = document.createElement("canvas");
      canvas.className = "ng-timeseries-canvas";
      canvas.width = 560;
      canvas.height = 240;
      canvas.setAttribute("aria-label", "Multi-series telemetry chart");

      const tooltip = document.createElement("div");
      tooltip.className = "ng-chart-tooltip";
      tooltip.hidden = true;

      const crosshairOut = document.createElement("output");
      crosshairOut.className = "ng-impact-output";
      crosshairOut.textContent = "Hover plot to inspect historical bounds.";

      wrap.append(canvas, tooltip);

      const getCssToken = (token: string): string => {
        if (typeof getComputedStyle === "function" && document.documentElement) {
          const val = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
          if (val) return val;
        }
        return "currentColor";
      };

      const drawChart = () => {
        const allSeries = selectTimeSeries(spine.store);
        const visibleSeries = allSeries.filter((s) => !hiddenSeries.has(s.seriesKey));

        // Rebuild interactive series toggle checkboxes cleanly
        const toggles: HTMLLabelElement[] = [];
        for (const s of allSeries) {
          const lbl = document.createElement("label");
          lbl.className = "ng-hud-cluster";
          const cb = document.createElement("input");
          cb.type = "checkbox";
          cb.checked = !hiddenSeries.has(s.seriesKey);
          cb.addEventListener("change", () => {
            if (cb.checked) hiddenSeries.delete(s.seriesKey);
            else hiddenSeries.add(s.seriesKey);
            drawChart();
          });
          lbl.append(cb, document.createTextNode(` ${s.label} (${s.unit})`));
          toggles.push(lbl);
        }
        seriesFilterRow.replaceChildren(...toggles);

        const targetW = Math.max(320, wrap.clientWidth || canvas.width || 560);
        const targetH = Math.max(210, wrap.clientHeight || canvas.height || 240);
        canvas.width = targetW;
        canvas.height = targetH;

        const layout = computeChartLayout(visibleSeries, targetW, targetH, tickWindowLimit);
        canvas.dataset.lastMinTick = String(layout.minTick);
        canvas.dataset.lastMaxTick = String(layout.maxTick);
        canvas.dataset.lastMinVal = layout.minVal.toFixed(2);
        canvas.dataset.lastMaxVal = layout.maxVal.toFixed(2);

        let ctx: CanvasRenderingContext2D | null = null;
        try {
          ctx = canvas.getContext("2d");
        } catch {
          ctx = null;
        }
        if (!ctx) return;

        ctx.clearRect(0, 0, targetW, targetH);

        const gridStroke = getCssToken("--chart-grid-stroke");
        const axisInk = getCssToken("--chart-axis-ink");
        const bandFill = getCssToken("--chart-band-fill");
        const paletteTokens = [
          getCssToken("--chart-series-1"),
          getCssToken("--chart-series-2"),
          getCssToken("--chart-series-3"),
          getCssToken("--chart-series-4"),
        ];

        const xForTick = (t: number) =>
          layout.padLeft + ((t - layout.minTick) / (layout.maxTick - layout.minTick)) * layout.plotWidth;
        const yForVal = (v: number) =>
          layout.padTop +
          layout.plotHeight -
          ((v - layout.minVal) / (layout.maxVal - layout.minVal)) * layout.plotHeight;

        ctx.strokeStyle = gridStroke;
        ctx.lineWidth = 1;
        for (let g = 0; g <= 4; g += 1) {
          const y = layout.padTop + (layout.plotHeight * g) / 4;
          ctx.beginPath();
          ctx.moveTo(layout.padLeft, y);
          ctx.lineTo(layout.padLeft + layout.plotWidth, y);
          ctx.stroke();

          const valAtTick = layout.maxVal - ((layout.maxVal - layout.minVal) * g) / 4;
          ctx.fillStyle = axisInk;
          ctx.font = "11px sans-serif";
          ctx.fillText(valAtTick.toFixed(0), 6, y + 4);
        }

        // Draw non-overlapping legend items
        for (let idx = 0; idx < layout.legendItems.length; idx += 1) {
          const item = layout.legendItems[idx];
          const stroke = paletteTokens[idx % paletteTokens.length];
          ctx.fillStyle = stroke;
          ctx.fillRect(item.x, item.y + 4, 10, 8);
          ctx.fillStyle = axisInk;
          ctx.font = "11px sans-serif";
          ctx.fillText(item.label, item.x + 14, item.y + 12);
        }

        for (let idx = 0; idx < visibleSeries.length; idx += 1) {
          const s = visibleSeries[idx];
          const pts = s.points.filter((pt) => pt.tick >= layout.minTick);
          if (pts.length === 0) continue;
          const color = paletteTokens[idx % paletteTokens.length];

          // Confidence uncertainty band
          ctx.fillStyle = bandFill;
          ctx.beginPath();
          for (let i = 0; i < pts.length; i += 1) {
            const px = xForTick(pts[i].tick);
            const pyMax = yForVal(pts[i].maxBound);
            if (i === 0) ctx.moveTo(px, pyMax);
            else ctx.lineTo(px, pyMax);
          }
          for (let i = pts.length - 1; i >= 0; i -= 1) {
            const px = xForTick(pts[i].tick);
            const pyMin = yForVal(pts[i].minBound);
            ctx.lineTo(px, pyMin);
          }
          ctx.closePath();
          ctx.fill();

          // Reported trend line
          ctx.strokeStyle = color;
          ctx.lineWidth = 2;
          ctx.beginPath();
          for (let i = 0; i < pts.length; i += 1) {
            const px = xForTick(pts[i].tick);
            const py = yForVal(pts[i].value);
            if (i === 0) ctx.moveTo(px, py);
            else ctx.lineTo(px, py);
          }
          ctx.stroke();
        }

        if (hoverTick !== null) {
          const clampedTick = Math.max(layout.minTick, Math.min(layout.maxTick, hoverTick));
          const hx = xForTick(clampedTick);
          ctx.strokeStyle = axisInk;
          ctx.setLineDash([4, 3]);
          ctx.beginPath();
          ctx.moveTo(hx, layout.padTop);
          ctx.lineTo(hx, layout.padTop + layout.plotHeight);
          ctx.stroke();
          ctx.setLineDash([]);

          const readouts: string[] = [];
          for (const s of visibleSeries) {
            let nearest = s.points[0];
            let bestDist = Infinity;
            for (const pt of s.points) {
              const d = Math.abs(pt.tick - clampedTick);
              if (d < bestDist) {
                bestDist = d;
                nearest = pt;
              }
            }
            if (nearest) {
              readouts.push(
                `${s.label}: ${nearest.value.toFixed(1)} ${s.unit} [${nearest.minBound.toFixed(1)}..${nearest.maxBound.toFixed(1)}] (${nearest.isConfirmed ? "Direct" : "Est."})`,
              );
            }
          }
          const summary = `Tick ${Math.round(clampedTick)} — ${readouts.join(" | ")}`;
          crosshairOut.textContent = summary;
          tooltip.hidden = false;
          tooltip.textContent = summary;
          tooltip.style.left = `${Math.min(targetW - 180, Math.max(8, Math.round(hx + 8)))}px`;
          tooltip.style.top = `${Math.round(layout.padTop + 6)}px`;
        }
      };

      canvas.addEventListener("pointermove", (ev) => {
        const allSeries = selectTimeSeries(spine.store).filter((s) => !hiddenSeries.has(s.seriesKey));
        const layout = computeChartLayout(allSeries, canvas.width, canvas.height, tickWindowLimit);
        const rect = canvas.getBoundingClientRect();
        const relX = (ev.clientX - rect.left) - layout.padLeft;
        const normX = Math.max(0, Math.min(1, relX / Math.max(1, layout.plotWidth)));
        hoverTick = layout.minTick + normX * (layout.maxTick - layout.minTick);
        drawChart();
      });

      canvas.addEventListener("pointerleave", () => {
        hoverTick = null;
        tooltip.hidden = true;
        drawChart();
      });

      let resizeObserver: ResizeObserver | null = null;
      if (typeof ResizeObserver === "function") {
        resizeObserver = new ResizeObserver(() => drawChart());
        resizeObserver.observe(wrap);
      }

      drawChart();
      root.append(toolbar, seriesFilterRow, wrap, crosshairOut);
      container.replaceChildren(root);

      const unsubStore = spine.store.subscribe(drawChart);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") drawChart();
      });
      return () => {
        resizeObserver?.disconnect();
        unsubStore();
        unsubBus();
      };
    },
  };
}

/**
 * Interactive Command & Delegation Org Tree displaying sovereign Attention Capacity,
 * oversight dilution warnings, expandable subordinates, and click-through inspection
 * (`UI-DAT-003`, `UI-DAT-004`).
 */
export function createDelegationOrgTreeWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "delegation_org_tree",
    defaultTitle: "Delegation & Attention Org Tree",
    minWidth: 440,
    minHeight: 300,
    hotkey: "o",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const expandedSubs = new Set<number>();

      const render = () => {
        const tree = selectDelegationTree(spine.store);
        const snap = spine.store.getSnapshot();
        if (!tree || !snap) {
          const empty = document.createElement("p");
          empty.textContent = "Awaiting delegation hierarchy telemetry.";
          root.replaceChildren(empty);
          return;
        }

        const overloadRatio =
          tree.overloadRatio ??
          (tree.capacityAttention > 0 ? tree.usedAttention / tree.capacityAttention : 0);
        const isOverloaded = overloadRatio > 1.0 || tree.usedAttention > tree.capacityAttention;

        const leaderBox = document.createElement("section");
        leaderBox.className = "ng-subordinate-card";
        leaderBox.setAttribute("data-overload-state", isOverloaded ? "crit" : overloadRatio > 0.8 ? "warn" : "ok");

        const leaderHeading = document.createElement("strong");
        leaderHeading.textContent = `${tree.leaderTitle} — Attention ${tree.usedAttention.toFixed(1)} / ${tree.capacityAttention.toFixed(1)}`;

        const meter = document.createElement("meter");
        meter.min = 0;
        meter.max = Math.max(1, tree.capacityAttention);
        meter.value = Math.min(tree.usedAttention, Math.max(1, tree.capacityAttention));
        meter.low = tree.capacityAttention * 0.6;
        meter.high = tree.capacityAttention * 0.85;
        meter.optimum = tree.capacityAttention * 0.5;

        const overheadNote = document.createElement("p");
        overheadNote.textContent = isOverloaded
          ? `OVERLOAD WARNING (${(overloadRatio * 100).toFixed(0)}% load, clerical overhead ${(tree.clericalOverhead ?? 0).toFixed(1)}): Subordinates suffer oversight dilution.`
          : `Oversight Load ${(overloadRatio * 100).toFixed(0)}% · Clerical Overhead ${(tree.clericalOverhead ?? 0).toFixed(1)}`;

        leaderBox.append(leaderHeading, meter, overheadNote);

        const orgGrid = document.createElement("div");
        orgGrid.className = "ng-org-grid";

        for (const sub of tree.subordinates) {
          if (!expandedSubs.has(sub.characterId)) {
            expandedSubs.add(sub.characterId);
          }
          const card = document.createElement("article");
          card.className = "ng-subordinate-card";
          card.dataset.characterId = String(sub.characterId);

          const headerRow = document.createElement("div");
          headerRow.className = "ng-hud-cluster";

          const expandBtn = document.createElement("button");
          expandBtn.type = "button";
          expandBtn.className = "ng-btn";
          const isOpen = expandedSubs.has(sub.characterId);
          expandBtn.setAttribute("aria-expanded", isOpen ? "true" : "false");
          expandBtn.textContent = isOpen ? "Collapse" : "Expand";
          expandBtn.addEventListener("click", () => {
            if (expandedSubs.has(sub.characterId)) expandedSubs.delete(sub.characterId);
            else expandedSubs.add(sub.characterId);
            render();
          });

          const nameLink = createClickableToken(
            document,
            `${sub.displayName} (${sub.roleTitle})`,
            () => {
              spine.bus.emit({
                kind: "FocusEntityInspector",
                entityKind: "character",
                entityId: sub.characterId,
              });
              if (sub.homeCellId !== undefined) {
                spine.bus.emit({ kind: "FlyGlobeToCell", cellId: sub.homeCellId });
              }
              spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
            },
          );

          headerRow.append(expandBtn, nameLink);

          const metricsLine = document.createElement("p");
          const compPct = Math.round(sub.competence * 100);
          const loyalty = sub.loyaltyLabel ? ` · Loyalty: ${sub.loyaltyLabel}` : "";
          const attn = sub.allocatedAttention !== undefined ? ` · Attn ${sub.allocatedAttention.toFixed(1)}` : "";
          metricsLine.textContent = `Competence ${compPct}%${loyalty}${attn}`;

          card.append(headerRow, metricsLine);

          if (isOpen) {
            const mandates = sub.assignedMandates ?? [];
            const ul = document.createElement("ul");
            ul.className = "ng-ledger-list";
            if (mandates.length === 0) {
              const li = document.createElement("li");
              li.textContent = "No active mandates assigned.";
              ul.appendChild(li);
            } else {
              for (const m of mandates) {
                const li = document.createElement("li");
                const isDiluted = m.realizedEfficiency < 0.75 || isOverloaded;
                if (isDiluted) li.setAttribute("data-diluted-mandate", "true");
                const targetBtn = createClickableToken(
                  document,
                  `${m.summary} [Eff ${Math.round(m.realizedEfficiency * 100)}% · ${m.status}${isDiluted ? " · DILUTED" : ""}]`,
                  () => {
                    spine.bus.emit({ kind: "FlyGlobeToCell", cellId: m.targetCellId });
                    spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
                  },
                );
                li.appendChild(targetBtn);
                ul.appendChild(li);
              }
            }
            card.appendChild(ul);
          }

          orgGrid.appendChild(card);
        }

        root.replaceChildren(leaderBox, orgGrid);
      };

      render();
      container.replaceChildren(root);
      const unsubStore = spine.store.subscribe(render);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") render();
      });
      return () => {
        unsubStore();
        unsubBus();
      };
    },
  };
}

function appendDepositEstimatesSection(
  doc: Document,
  parent: HTMLElement,
  cell: KnownCellDto | undefined,
  currentTick: number,
): void {
  if (!cell || cell.tier === "unexplored" || cell.tier === "rumored") return;
  const estimates = cell.depositEstimates ?? [];
  if (estimates.length === 0) return;

  const heading = doc.createElement("strong");
  heading.textContent = "Geological Deposit Assays";
  const ul = doc.createElement("ul");
  ul.className = "ng-ledger-list";

  for (const est of estimates) {
    const li = doc.createElement("li");
    li.className = "ng-ledger-row";
    const textSpan = doc.createElement("span");
    textSpan.textContent = `${est.materialLabel}: ${Math.round(est.pointEstTonnes)} t [${Math.round(est.minTonnes)}..${Math.round(est.maxTonnes)} t] · Purity ${Math.round(est.purityEst01 * 100)}% (Tier ${est.instrumentTier})`;
    const badge = createProvenanceBadgeElement(doc, {
      currentTick,
      observedTick: est.observedTick,
      confidence01: est.confidence01,
      provenanceLabel: est.provenanceLabel,
    });
    li.append(textSpan, badge);
    ul.appendChild(li);
  }
  parent.append(heading, ul);
}

/**
 * Multi-instance-capable Diegetic Entity Inspector (`UI-WIN-004`, `UI-SKN-005`, `UI-DAT-004`, `UI-DAT-005`).
 */
export function createEntityInspectorWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "entity_inspector",
    defaultTitle: "Diegetic Field Inspector",
    minWidth: 360,
    minHeight: 260,
    hotkey: "i",
    mountContent(container, instanceKey, spine) {
      const box = document.createElement("div");
      box.className = "ng-responsive-grid";

      const render = () => {
        const snap = spine.store.getSnapshot();
        const currentTick = snap?.clockAndNotifications.tick ?? 0;
        const header = document.createElement("div");
        header.className = "ng-inspector-header";

        const [rawKind, rawId] = instanceKey.split(":");
        const targetKind = rawKind || "settlement";
        const targetId = Number(rawId);

        const img = document.createElement("img");
        img.className = "ng-portrait-img";

        const info = document.createElement("div");
        const title = document.createElement("strong");
        const detail = document.createElement("p");
        const extras = document.createElement("div");
        extras.className = "ng-responsive-grid";

        const fallbackPortrait =
          spine.assetResolver?.resolvePortrait({ characterId: 0, vocation: "__none__" }) ?? "";

        if (!snap) {
          title.textContent = "Awaiting Survey Snapshot";
          detail.textContent = "No survey records received yet.";
          wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, "Default silhouette");
        } else if (targetKind === "character" && Number.isFinite(targetId)) {
          const tree = selectDelegationTree(spine.store);
          const sub = tree?.subordinates.find((s) => s.characterId === targetId) ?? tree?.subordinates[0];
          const primaryUrl =
            spine.assetResolver?.resolvePortrait({
              characterId: sub?.characterId ?? targetId,
              vocation: sub?.roleTitle.toLowerCase().includes("artificer") ? "artificer" : "agronomist",
              ageBracket: "adult",
            }) ?? "";
          wireImageWithFallback(img, primaryUrl, fallbackPortrait, sub?.displayName ?? "Official");
          title.textContent = sub ? `${sub.displayName} — ${sub.roleTitle}` : `Official #${targetId}`;
          detail.textContent = sub
            ? `Competence: ${(sub.competence * 100).toFixed(0)}% | Reported to ${tree?.leaderTitle ?? "Commander"}`
            : "Unverified official record.";
          if (sub) {
            const badge = createProvenanceBadgeElement(document, {
              currentTick,
              observedTick: currentTick,
              confidence01: sub.precision01 ?? 0.85,
              provenanceLabel: "HQ Personnel Dossier",
            });
            extras.appendChild(badge);
            if (sub.homeCellId !== undefined) {
              const flyBtn = createClickableToken(document, `Locate Post (Cell #${sub.homeCellId})`, () => {
                spine.bus.emit({ kind: "FlyGlobeToCell", cellId: sub.homeCellId! });
              });
              extras.appendChild(flyBtn);
            }
          }
          const audits = selectAuditDiscrepancies(spine.store).filter((a) => a.characterId === sub?.characterId);
          for (const a of audits) {
            const warn = document.createElement("p");
            warn.className = "ng-autopause-banner";
            warn.textContent = `Audit Discrepancy (${a.commodityKey}): claimed ${Math.round(a.claimedMassKg)} kg versus verified ${Math.round(a.verifiedMassKg)} kg.`;
            extras.appendChild(warn);
          }
        } else if (targetKind === "convoy" && Number.isFinite(targetId)) {
          const convoys = selectTrackedConvoys(spine.store);
          const conv = convoys.find((c) => c.convoyId === targetId) ?? convoys[0];
          const iconUrl = spine.assetResolver?.resolveIcon("colony_precision_tools") ?? "";
          wireImageWithFallback(img, iconUrl, fallbackPortrait, "Convoy icon");
          title.textContent = conv ? `${conv.label} (${conv.status})` : `Convoy #${targetId}`;
          detail.textContent = conv
            ? `Projected Cell #${conv.projectedCellId} · Channel: ${conv.channel ?? "dead-reckoning"}`
            : "No telemetry available.";
          if (conv) {
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: conv.departureTick ?? currentTick,
                confidence01: conv.confidence01 ?? 0.65,
                provenanceLabel: conv.provenanceLabel ?? "Logistics Telemetry",
                isLiveSight: conv.status.toLowerCase().includes("confirmed"),
              }),
            );
          }
        } else if (targetKind === "facility" && Number.isFinite(targetId)) {
          const fac = selectFacilityRows(spine.store).find((f) => f.facilityId === targetId) ?? selectFacilityRows(spine.store)[0];
          wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, fac?.processName ?? "Facility");
          title.textContent = fac ? `${fac.processName} (${fac.settlementName})` : `Facility #${targetId}`;
          detail.textContent = fac
            ? `${fac.assignedWorkers} workers · Tool condition ${Math.round(fac.toolCondition01 * 100)}%`
            : "No facility record found.";
          if (fac) {
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: fac.observedTick,
                confidence01: 0.9,
                provenanceLabel: fac.provenanceLabel,
              }),
            );
          }
        } else if (targetKind === "cell" && Number.isFinite(targetId)) {
          const cell = selectKnownCells(spine.store).find((c) => c.cellId === targetId);
          wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, `Cell #${targetId}`);
          if (!cell || cell.tier === "unexplored") {
            title.textContent = `Unexplored Territory (Cell #${targetId})`;
            detail.textContent = "No expedition survey report has reached HQ for this cell.";
          } else {
            title.textContent = `${cell.tier.toUpperCase()} Survey — Cell #${cell.cellId}`;
            detail.textContent = `Elevation: ${Math.round(cell.elevationM)}m | Confidence: ${Math.round(cell.confidence01 * 100)}%`;
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: cell.observedTick,
                confidence01: cell.confidence01,
                provenanceLabel: cell.provenanceLabel,
                isLiveSight: cell.tier === "surveyed" ? cell.isLiveHqSight : false,
              }),
            );
            appendDepositEstimatesSection(document, extras, cell, currentTick);
          }
        } else {
          const settlements = selectKnownSettlements(spine.store);
          const hq = selectHqPresence(spine.store);
          const found = Number.isFinite(targetId)
            ? settlements.find((s) => s.settlementId === targetId)
            : settlements[0];
          const tree = selectDelegationTree(spine.store);
          const rep = tree?.subordinates[0];
          const portraitUrl =
            spine.assetResolver?.resolvePortrait({
              characterId: rep?.characterId ?? 1,
              vocation: "agronomist",
              ageBracket: "adult",
            }) ?? "";
          wireImageWithFallback(img, portraitUrl, fallbackPortrait, found?.settlementName ?? "Settlement Overseer");
          const cellId = found?.cellId ?? hq?.cellId ?? 0;
          title.textContent = found ? found.settlementName : hq?.settlementName ?? "Primary Settlement";
          detail.textContent = `Cell #${cellId} | HQ Sight Radius: ${hq?.sightRadiusCells ?? 1} cells`;

          const cell = selectKnownCells(spine.store).find((c) => c.cellId === cellId);
          if (cell && cell.tier !== "unexplored") {
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: cell.observedTick,
                confidence01: cell.confidence01,
                provenanceLabel: cell.provenanceLabel,
                isLiveSight: Boolean(found?.isHq || (cell.tier === "surveyed" && cell.isLiveHqSight)),
              }),
            );
            appendDepositEstimatesSection(document, extras, cell, currentTick);
          }

          const rowsHere = selectLedgerRows(spine.store, found?.settlementId);
          if (rowsHere.length > 0) {
            const topRows = rowsHere.slice(0, 4);
            const stockHeading = document.createElement("strong");
            stockHeading.textContent = "Reported Stockpiles";
            const ul = document.createElement("ul");
            ul.className = "ng-ledger-list";
            for (const r of topRows) {
              const li = document.createElement("li");
              li.className = "ng-ledger-row";
              const label = document.createElement("span");
              label.textContent = `${r.commodityName}: ${Math.round(r.reportedMassKg)} kg [${Math.round(r.minEstKg)}..${Math.round(r.maxEstKg)}]`;
              const badge = createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: r.observedTick,
                confidence01: r.confidence01 ?? 0.85,
                provenanceLabel: r.provenanceLabel,
                reporterName: r.reporterName,
                isLiveSight: r.observedTick >= currentTick && found?.isHq,
              });
              li.append(label, badge);
              ul.appendChild(li);
            }
            extras.append(stockHeading, ul);
          }
        }

        info.append(title, detail);
        header.append(img, info);

        const actions = document.createElement("div");
        actions.className = "ng-hud-cluster";
        const pinCloneBtn = document.createElement("button");
        pinCloneBtn.type = "button";
        pinCloneBtn.className = "ng-btn";
        pinCloneBtn.textContent = "Open Pinned Comparison Copy";
        pinCloneBtn.addEventListener("click", () => {
          spine.bus.emit({
            kind: "OpenSystemWindow",
            pluginId: "entity_inspector",
            instanceKey,
            pinned: true,
          });
        });

        actions.appendChild(pinCloneBtn);
        box.replaceChildren(header, extras, actions);
      };

      render();
      container.replaceChildren(box);
      const unsubStore = spine.store.subscribe(render);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") render();
      });
      return () => {
        unsubStore();
        unsubBus();
      };
    },
  };
}

/**
 * Window Layout Presets & Workspace Profiles selector (`UI-WIN-007`).
 */
export function createLayoutPresetsWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "layout_presets",
    defaultTitle: "Workspace Layout Presets",
    minWidth: 320,
    minHeight: 220,
    hotkey: "l",
    mountContent(container, _instanceKey, spine) {
      const wrap = document.createElement("div");
      wrap.className = "ng-responsive-grid";

      const nameInput = document.createElement("input");
      nameInput.type = "text";
      nameInput.className = "ng-text-input";
      nameInput.placeholder = "New layout preset name...";
      nameInput.setAttribute("aria-label", "New layout preset name");

      const saveBtn = document.createElement("button");
      saveBtn.type = "button";
      saveBtn.className = "ng-btn ng-btn-primary";
      saveBtn.textContent = "Save Current Layout";

      const presetList = document.createElement("div");
      presetList.className = "ng-hud-cluster";

      const render = () => {
        const names = spine.layoutPresets?.listPresetNames() ?? [];
        const buttons: HTMLButtonElement[] = [];
        for (const name of names) {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "ng-btn";
          btn.setAttribute("data-layout-preset", name);
          btn.textContent = name;
          btn.addEventListener("click", () => {
            spine.bus.emit({ kind: "ApplyWindowLayoutPreset", presetName: name });
          });
          buttons.push(btn);
        }
        presetList.replaceChildren(...buttons);
      };

      saveBtn.addEventListener("click", () => {
        const val = nameInput.value.trim();
        if (!val) return;
        spine.bus.emit({ kind: "SaveWindowLayoutPreset", presetName: val });
        nameInput.value = "";
        render();
      });

      render();
      wrap.append(presetList, nameInput, saveBtn);
      container.replaceChildren(wrap);
      return () => {};
    },
  };
}
