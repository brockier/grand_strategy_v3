import {
  DiegeticSnapshot,
  GoldbergTopologyData,
  KnownCellDto,
  KnownSettlementDto,
  ReportedLedgerRowDto,
  SurveyedRoadEdgeDto,
  TimeSeriesDto,
  WindowContentPlugin,
  createAccessibleTabGroup,
  createProvenanceBadgeElement,
  generateGoldbergTopology,
  selectAuditDiscrepancies,
  selectDelegationTree,
  selectFacilityRows,
  selectGovernance,
  selectHqPresence,
  selectKnowledge,
  selectKnownCells,
  selectKnownSettlements,
  selectLedgerRows,
  selectNonStateEconomy,
  selectObservedWeather,
  selectPopulationRows,
  selectSurveyedRoads,
  selectTimeSeries,
  selectTrackedConvoys,
  selectWarfare,
  syncSelectOptions,
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
    hotkey: "p",
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
    const depthTag = est.accessibilityTier ? ` · Depth Tier ${est.accessibilityTier}` : "";
    const placerTag = est.isAlluvialPlacer ? " · Alluvial Placer" : "";
    const offshoreTag = est.isOffshore ? " · Offshore Shelf" : "";
    textSpan.textContent = `${est.materialLabel}: ${Math.round(est.pointEstTonnes)} t [${Math.round(est.minTonnes)}..${Math.round(est.maxTonnes)} t] · Purity ${Math.round(est.purityEst01 * 100)}% (Assay Tier ${est.instrumentTier}${depthTag}${placerTag}${offshoreTag})`;
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

function formatGeoKeyLabel(raw: string | undefined, fallback: string): string {
  if (!raw || !raw.trim()) return fallback;
  return raw
    .trim()
    .split("_")
    .map((p) => (p.length > 0 ? p[0].toUpperCase() + p.slice(1) : p))
    .join(" ");
}

function appendDamControlSection(
  doc: Document,
  parent: HTMLElement,
  cell: Extract<KnownCellDto, { tier: "surveyed" | "stale" }>,
  spine: ViewPluginContext,
): void {
  const hasRiver =
    !cell.isOcean &&
    (cell.riverChannelDownstreamCellId != null || (cell.strahlerOrder ?? 1) >= 2 || Boolean(cell.damStatus));
  if (!hasRiver) return;

  const sectionTitle = doc.createElement("strong");
  sectionTitle.textContent = "River Engineering & Hydroelectricity";
  parent.appendChild(sectionTitle);

  const dam = cell.damStatus ?? null;
  if (!dam) {
    const btn = doc.createElement("button");
    btn.type = "button";
    btn.className = "ng-hud-btn";
    btn.dataset.action = "construct-dam";
    btn.textContent = `Construct Hydroelectric Dam (Cell #${cell.cellId})`;
    btn.addEventListener("click", () => {
      spine.bus.emit({
        kind: "DispatchSimCommand",
        command: { kind: "constructDam", targetCell: cell.cellId, ruleCurve: "balanced_power" },
      });
    });
    parent.appendChild(btn);
    return;
  }

  const damList = doc.createElement("ul");
  damList.className = "ng-ledger-list";
  const outputLabel =
    dam.liveHydroKw >= 1000
      ? `${(dam.liveHydroKw / 1000).toFixed(2)} MW`
      : `${Math.round(dam.liveHydroKw)} kW`;
  appendInspectorDetailRow(
    doc,
    damList,
    "Dam Status",
    `${dam.breached ? "BREACHED OVERTOPPING" : "Operational"} · Condition ${Math.round(dam.condition01 * 100)}% · Rule: ${formatGeoKeyLabel(dam.ruleCurve, "Balanced Power")}`,
  );
  appendInspectorDetailRow(
    doc,
    damList,
    "Reservoir Storage",
    `${Math.round(dam.reservoirFill01 * 100)}% full (${(dam.reservoirStorageM3 / 1e6).toFixed(2)} / ${(dam.reservoirCapacityM3 / 1e6).toFixed(2)} Mm3) · Head ${dam.reservoirHeadM.toFixed(1)} m · Silt ${Math.round(dam.sedimentSilt01 * 100)}%`,
  );
  appendInspectorDetailRow(
    doc,
    damList,
    "Turbine & Spillway",
    `${outputLabel} generation · Spillway release ${dam.spillwayReleaseM3S.toFixed(1)} m3/s`,
  );
  parent.appendChild(damList);

  const curveBar = doc.createElement("div");
  curveBar.className = "ng-hud-cluster";
  const curves = [
    { key: "balanced_power", label: "Balanced Power" },
    { key: "flood_control", label: "Flood Control" },
    { key: "irrigation_supply", label: "Irrigation Supply" },
  ] as const;
  for (const c of curves) {
    const cBtn = doc.createElement("button");
    cBtn.type = "button";
    cBtn.className = "ng-hud-btn";
    cBtn.dataset.ruleCurve = c.key;
    cBtn.classList.toggle("is-active", dam.ruleCurve === c.key);
    cBtn.setAttribute("aria-pressed", dam.ruleCurve === c.key ? "true" : "false");
    cBtn.textContent = c.label;
    cBtn.addEventListener("click", () => {
      spine.bus.emit({
        kind: "DispatchSimCommand",
        command: { kind: "setDamRuleCurve", targetCell: cell.cellId, ruleCurve: c.key },
      });
    });
    curveBar.appendChild(cBtn);
  }
  parent.appendChild(curveBar);
}

const topologyCacheBySubdiv = new Map<number, GoldbergTopologyData>();

function getCachedTopology(subdivision: number): GoldbergTopologyData {
  const safeSubdiv = Math.max(1, Math.floor(subdivision || 3));
  let cached = topologyCacheBySubdiv.get(safeSubdiv);
  if (!cached) {
    cached = generateGoldbergTopology(safeSubdiv);
    topologyCacheBySubdiv.set(safeSubdiv, cached);
  }
  return cached;
}

function describeRockStrata(code: number | undefined): string {
  switch (code) {
    case 0:
      return "Alluvial & Sedimentary Basin";
    case 1:
      return "Bedded Limestone & Sandstone";
    case 2:
      return "Metamorphic Slate & Schist";
    case 3:
      return "Igneous Granite & Basalt";
    default:
      return "Unassayed Bedrock";
  }
}

function appendInspectorDetailRow(
  doc: Document,
  ul: HTMLElement,
  label: string,
  value: string,
): void {
  const li = doc.createElement("li");
  li.className = "ng-ledger-row";
  const keyEl = doc.createElement("strong");
  keyEl.textContent = label;
  const valEl = doc.createElement("span");
  valEl.textContent = value;
  li.append(keyEl, valEl);
  ul.appendChild(li);
}

function resolveAccessCorridor(
  allRoads: readonly SurveyedRoadEdgeDto[],
  clickedRoad: SurveyedRoadEdgeDto,
  settlementsByCell: ReadonlyMap<number, KnownSettlementDto>,
): {
  readonly corridorEdges: readonly SurveyedRoadEdgeDto[];
  readonly hubSettlement: KnownSettlementDto | undefined;
  readonly outpostCellId: number;
} {
  const samePurpose = clickedRoad.purposeLabel
    ? allRoads.filter(
        (r) =>
          r.roadPurpose === "resource_or_port_access" &&
          r.purposeLabel === clickedRoad.purposeLabel,
      )
    : [clickedRoad];
  const candidateEdges = samePurpose.length > 0 ? samePurpose : [clickedRoad];

  const visitedIds = new Set<number>([clickedRoad.edgeId]);
  const frontierCells = new Set<number>([clickedRoad.fromCellId, clickedRoad.toCellId]);
  let expanded = true;
  while (expanded) {
    expanded = false;
    for (const r of candidateEdges) {
      if (visitedIds.has(r.edgeId)) continue;
      if (frontierCells.has(r.fromCellId) || frontierCells.has(r.toCellId)) {
        visitedIds.add(r.edgeId);
        frontierCells.add(r.fromCellId);
        frontierCells.add(r.toCellId);
        expanded = true;
      }
    }
  }
  const corridorEdges = candidateEdges.filter((r) => visitedIds.has(r.edgeId));
  const deg = new Map<number, number>();
  let hubSettlement: KnownSettlementDto | undefined;
  for (const r of corridorEdges) {
    deg.set(r.fromCellId, (deg.get(r.fromCellId) ?? 0) + 1);
    deg.set(r.toCellId, (deg.get(r.toCellId) ?? 0) + 1);
    if (!hubSettlement) {
      hubSettlement = settlementsByCell.get(r.fromCellId) ?? settlementsByCell.get(r.toCellId);
    }
  }

  let outpostCellId: number | null = null;
  for (const [cid, d] of deg) {
    if (!settlementsByCell.has(cid) && d === 1) {
      outpostCellId = cid;
      break;
    }
  }
  if (outpostCellId === null) {
    outpostCellId = settlementsByCell.has(clickedRoad.fromCellId)
      ? clickedRoad.toCellId
      : clickedRoad.fromCellId;
  }
  return { corridorEdges, hubSettlement, outpostCellId };
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

        const [rawKind, rawId, rawLocalElev] = instanceKey.split(":");
        const targetKind = rawKind || "settlement";
        const targetId = Number(rawId);
        const clickedLocalElevM =
          rawLocalElev !== undefined && rawLocalElev !== "" && Number.isFinite(Number(rawLocalElev))
            ? Number(rawLocalElev)
            : undefined;

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
        } else if (targetKind === "formation" && Number.isFinite(targetId)) {
          const formations = selectWarfare(spine.store)?.formations ?? [];
          const form = formations.find((f) => f.formationId === targetId) ?? formations[0];
          const iconUrl = spine.assetResolver?.resolveIcon("colony_precision_tools") ?? "";
          wireImageWithFallback(img, iconUrl, fallbackPortrait, form?.label ?? "Formation");
          title.textContent = form ? `${form.label} (${form.commanderName})` : `Formation ${targetId}`;
          detail.textContent = form
            ? `${form.personnelCount} personnel · Cohesion ${Math.round(form.cohesion01 * 100)}% · Munitions ${form.munitionsDays.toFixed(1)}d · Rations ${form.rationsDays.toFixed(1)}d · Cell ${form.currentCellId} -> Target Cell ${form.targetCellId}`
            : "No formation telemetry available.";
          if (form) {
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: form.observedTick,
                confidence01: form.confidence01,
                provenanceLabel: form.provenanceLabel,
                isLiveSight: currentTick - form.observedTick <= 2,
              }),
            );
            const posture = document.createElement("span");
            posture.className = "ng-posture-badge";
            posture.textContent = form.noRetreatOrder
              ? "Standing Order: No Retreat"
              : `Doctrine: ${form.doctrineKey}`;
            extras.appendChild(posture);
          }
        } else if (targetKind === "facility" && Number.isFinite(targetId)) {
          const fac = selectFacilityRows(spine.store).find((f) => f.facilityId === targetId);
          if (fac) {
            wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, fac.processName);
            title.textContent = `${fac.processName} (${fac.settlementName})`;
            detail.textContent = `${fac.assignedWorkers} workers · Tool condition ${Math.round(fac.toolCondition01 * 100)}%${fac.weatherHalted ? " · Weather Halted" : ""}`;
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: fac.observedTick,
                confidence01: 0.9,
                provenanceLabel: fac.provenanceLabel,
              }),
            );
            const settBtn = createClickableToken(document, `Inspect ${fac.settlementName}`, () => {
              spine.bus.emit({
                kind: "FocusEntityInspector",
                entityKind: "settlement",
                entityId: fac.settlementId,
              });
            });
            extras.appendChild(settBtn);
          } else {
            const allRoads = selectSurveyedRoads(spine.store);
            const outpostRoad = allRoads.find((r) => 900000 + r.edgeId === targetId);
            const iconUrl = spine.assetResolver?.resolveIcon("colony_precision_tools") ?? "";
            wireImageWithFallback(img, iconUrl, fallbackPortrait, "Resource Outpost");
            if (outpostRoad) {
              const knownSettlements = selectKnownSettlements(spine.store);
              const settlementsByCell = new Map(knownSettlements.map((s) => [s.cellId, s]));
              const { corridorEdges, hubSettlement, outpostCellId } = resolveAccessCorridor(
                allRoads,
                outpostRoad,
                settlementsByCell,
              );
              const purposeLower = (outpostRoad.purposeLabel ?? "").toLowerCase();
              const isQuarry = purposeLower.includes("quarry");
              const isTimber = purposeLower.includes("timber") || purposeLower.includes("mill");

              const outpostKindTitle = isQuarry
                ? "Quarry Outpost"
                : isTimber
                  ? "Timber & Mill Outpost"
                  : "Field Work Outpost";
              const structureDesc = isQuarry
                ? "Open-Cut Stone & Mineral Quarry (Timber Hoist Derrick, Mason Dressing Shed & Tool Store)"
                : isTimber
                  ? "Forest Felling Camp & Sawmill (Waterwheel Sawpit, Seasoning Yard & Tool Shed)"
                  : "Detached Resource & Portage Works";

              const allCells = selectKnownCells(spine.store);
              const siteCell = allCells.find((c) => c.cellId === outpostCellId);
              const hubCell = hubSettlement
                ? allCells.find((c) => c.cellId === hubSettlement.cellId)
                : undefined;

              const cohorts = selectPopulationRows(
                spine.store,
                hubSettlement?.settlementId,
              );
              const crewKeywords = isQuarry
                ? ["miner", "quarry", "mason", "stone", "labor", "labour", "artificer", "artisan"]
                : isTimber
                  ? ["timber", "forest", "wood", "saw", "mill", "carpenter", "labor", "labour", "artificer"]
                  : ["labor", "labour", "porter", "artificer"];
              const matchingCohorts = cohorts.filter((c) => {
                const keyLow = `${c.vocationKey} ${c.vocationName}`.toLowerCase();
                return crewKeywords.some((kw) => keyLow.includes(kw));
              });
              const crewSource = matchingCohorts.length > 0 ? matchingCohorts : cohorts;
              const sourceHeadcount = crewSource.reduce((acc, c) => acc + c.reportedHeadcount, 0);
              const stationedCount =
                sourceHeadcount > 0
                  ? Math.min(sourceHeadcount, Math.max(6, Math.round(sourceHeadcount * 0.08)))
                  : 0;
              const avgSkillPct =
                sourceHeadcount > 0
                  ? Math.round(
                      (crewSource.reduce((acc, c) => acc + c.meanSkill01 * c.reportedHeadcount, 0) /
                        sourceHeadcount) *
                        100,
                    )
                  : 0;
              const avgNutritionPct =
                sourceHeadcount > 0
                  ? Math.round(
                      (crewSource.reduce((acc, c) => acc + c.nutrition01 * c.reportedHeadcount, 0) /
                        sourceHeadcount) *
                        100,
                    )
                  : 0;
              const topVocations = crewSource
                .slice()
                .sort((a, b) => b.reportedHeadcount - a.reportedHeadcount)
                .slice(0, 2)
                .map((c) => c.vocationName)
                .join(", ");

              const settlementFacs = selectFacilityRows(spine.store, hubSettlement?.settlementId);
              const toolCondPct =
                settlementFacs.length > 0
                  ? Math.round(
                      (settlementFacs.reduce((acc, f) => acc + f.toolCondition01, 0) /
                        settlementFacs.length) *
                        100,
                    )
                  : 86;
              const weatherAtSite = selectObservedWeather(spine.store).find(
                (w) => w.cellId === outpostCellId,
              );
              const shiftStatus =
                weatherAtSite &&
                (weatherAtSite.mudIndex01 > 0.55 ||
                  weatherAtSite.precipMmHr > 5 ||
                  (weatherAtSite.floodStage01 ?? 0) > 0.4)
                  ? "Slowed by Mud / Heavy Weather"
                  : "Active Shift";

              const tree = selectDelegationTree(spine.store);
              const overseer =
                tree?.subordinates.find((s) => s.homeSettlementId === hubSettlement?.settlementId) ??
                tree?.subordinates[0];

              title.textContent = `${outpostKindTitle} — Cell #${outpostCellId}${hubSettlement ? ` (${hubSettlement.settlementName})` : ""}`;
              detail.textContent = `${structureDesc} · ${stationedCount > 0 ? `${stationedCount} stationed workers (${shiftStatus})` : `${shiftStatus} · Crew detached from ${hubSettlement?.settlementName ?? "Settlement HQ"}`}`;

              if (siteCell && siteCell.tier !== "unexplored") {
                extras.appendChild(
                  createProvenanceBadgeElement(document, {
                    currentTick,
                    observedTick: siteCell.observedTick,
                    confidence01: siteCell.confidence01,
                    provenanceLabel: siteCell.provenanceLabel,
                    isLiveSight: siteCell.tier === "surveyed" ? siteCell.isLiveHqSight : false,
                  }),
                );
              }

              const specList = document.createElement("ul");
              specList.className = "ng-ledger-list";
              appendInspectorDetailRow(
                document,
                specList,
                "Structure & Readiness",
                `${structureDesc} · Tool Condition ${toolCondPct}% · ${shiftStatus}`,
              );

              const personnelText =
                stationedCount > 0
                  ? `${stationedCount} detached crew (${topVocations}) from ${hubSettlement?.settlementName ?? "HQ"} · Avg Skill ${avgSkillPct}% · Nutrition ${avgNutritionPct}%${overseer ? ` · Overseer: ${overseer.displayName}` : ""}`
                  : `Working shift drawn from ${hubSettlement?.settlementName ?? "HQ"} labor roll (no detached cohort ledger row filed)${overseer ? ` · Overseer: ${overseer.displayName}` : ""}`;
              appendInspectorDetailRow(document, specList, "Stationed Personnel", personnelText);

              if (siteCell && siteCell.tier !== "unexplored") {
                const biomeText = siteCell.biomeKey ?? (siteCell.isOcean ? "ocean" : "Surveyed Land");
                const strataText =
                  siteCell.tier === "surveyed" || siteCell.tier === "stale"
                    ? describeRockStrata(siteCell.rockStrataCode)
                    : "Unassayed Surface";
                const siteElevM = Math.round(siteCell.elevationM);
                let gradeText = "";
                if (hubSettlement && hubCell && hubCell.tier !== "unexplored") {
                  const deltaM = Math.round(siteCell.elevationM - hubCell.elevationM);
                  gradeText =
                    deltaM > 10
                      ? ` · ${deltaM}m downhill haul to ${hubSettlement.settlementName} (${Math.round(hubCell.elevationM)}m)`
                      : deltaM < -10
                        ? ` · ${Math.abs(deltaM)}m uphill haul to ${hubSettlement.settlementName} (${Math.round(hubCell.elevationM)}m)`
                        : ` · Level haul with ${hubSettlement.settlementName} (${Math.round(hubCell.elevationM)}m)`;
                }
                const riverNote =
                  (siteCell.tier === "surveyed" || siteCell.tier === "stale") &&
                  siteCell.riverDischargeM3S !== undefined &&
                  siteCell.riverDischargeM3S > 0
                    ? ` · River inflow ${siteCell.riverDischargeM3S.toFixed(0)} m3/s`
                    : "";
                appendInspectorDetailRow(
                  document,
                  specList,
                  "Site Geology & Terrain",
                  `${biomeText} · Strata: ${strataText} · Node Elevation ${siteElevM}m${gradeText}${riverNote}`,
                );
              }

              const maxPassages = corridorEdges.reduce(
                (m, e) => Math.max(m, e.cumulativePassages ?? 0),
                outpostRoad.cumulativePassages ?? 0,
              );
              const maxTonnes = corridorEdges.reduce(
                (m, e) => Math.max(m, e.cumulativeTonnesPassed ?? 0),
                outpostRoad.cumulativeTonnesPassed ?? 0,
              );
              appendInspectorDetailRow(
                document,
                specList,
                "Access Corridor",
                `${outpostRoad.purposeLabel || `Cells #${outpostRoad.fromCellId} <-> #${outpostRoad.toCellId}`} · ${outpostRoad.infraKey} (Edge #${outpostRoad.edgeId}, ${corridorEdges.length} hop${corridorEdges.length === 1 ? "" : "s"}) · Traffic: ${maxPassages} passages (${maxTonnes.toFixed(1)} t)`,
              );
              extras.appendChild(specList);

              if (siteCell && (siteCell.tier === "surveyed" || siteCell.tier === "stale")) {
                if ((siteCell.depositEstimates ?? []).length > 0) {
                  appendDepositEstimatesSection(document, extras, siteCell, currentTick);
                } else {
                  const fallbackAssay = document.createElement("p");
                  fallbackAssay.textContent = isQuarry
                    ? "Surface Extraction: Dimension stone, rubble aggregate & bedrock spoil active (no deep metallic vein assay filed for this cell yet)."
                    : "Surface Harvest: Standing timber & sawpit waterwheel intake (non-mineral harvest).";
                  extras.appendChild(fallbackAssay);
                }
              }

              const navRow = document.createElement("div");
              navRow.className = "ng-hud-cluster";
              navRow.appendChild(
                createClickableToken(document, `Inspect Corridor Edge #${outpostRoad.edgeId}`, () => {
                  spine.bus.emit({
                    kind: "FocusEntityInspector",
                    entityKind: "road",
                    entityId: outpostRoad.edgeId,
                  });
                }),
              );
              navRow.appendChild(
                createClickableToken(document, `Inspect Site Cell #${outpostCellId}`, () => {
                  spine.bus.emit({
                    kind: "FocusEntityInspector",
                    entityKind: "cell",
                    entityId: outpostCellId,
                  });
                }),
              );
              if (hubSettlement) {
                navRow.appendChild(
                  createClickableToken(document, `Return to ${hubSettlement.settlementName}`, () => {
                    spine.bus.emit({
                      kind: "FocusEntityInspector",
                      entityKind: "settlement",
                      entityId: hubSettlement.settlementId,
                    });
                  }),
                );
              }
              extras.appendChild(navRow);
            } else {
              title.textContent = `Facility #${targetId}`;
              detail.textContent = "No facility record found.";
            }
          }
        } else if (targetKind === "road" && Number.isFinite(targetId)) {
          const road = selectSurveyedRoads(spine.store).find((r) => r.edgeId === targetId);
          const iconUrl = spine.assetResolver?.resolveIcon("colony_precision_tools") ?? "";
          wireImageWithFallback(img, iconUrl, fallbackPortrait, "Road corridor");
          if (road) {
            const purposeText = road.purposeLabel || road.roadPurpose || "Purposeful Corridor";
            title.textContent = `${purposeText} (${road.infraKey})`;
            detail.textContent = `Edge #${road.edgeId} · Cells #${road.fromCellId} <-> #${road.toCellId} · Traffic: ${road.cumulativePassages ?? 0} passages (${(road.cumulativeTonnesPassed ?? 0).toFixed(1)} t) · ${road.washedOut ? "Washed Out" : "Passable"}${road.bridgeCapKg ? ` · Bridge Cap ${Math.round(road.bridgeCapKg)} kg` : ""}`;
            const flyFrom = createClickableToken(document, `Locate Cell #${road.fromCellId}`, () => {
              spine.bus.emit({ kind: "FlyGlobeToCell", cellId: road.fromCellId });
            });
            const flyTo = createClickableToken(document, `Locate Cell #${road.toCellId}`, () => {
              spine.bus.emit({ kind: "FlyGlobeToCell", cellId: road.toCellId });
            });
            extras.append(flyFrom, flyTo);
          } else {
            title.textContent = `Road Edge #${targetId}`;
            detail.textContent = "No surveyed road corridor telemetry found for this edge.";
          }
        } else if (targetKind === "river" && Number.isFinite(targetId)) {
          const cells = selectKnownCells(spine.store);
          const cell = cells.find((c) => c.cellId === targetId);
          const iconUrl = spine.assetResolver?.resolveIcon("colony_precision_tools") ?? "";
          wireImageWithFallback(img, iconUrl, fallbackPortrait, "River channel");
          if (cell && (cell.tier === "surveyed" || cell.tier === "stale")) {
            const dsId =
              cell.riverChannelDownstreamCellId !== undefined
                ? cell.riverChannelDownstreamCellId
                : (cell.riverDownstreamCellId ?? null);
            const dsCell = dsId !== null ? cells.find((c) => c.cellId === dsId) : undefined;
            const terminusLabel =
              dsId === null
                ? "Inland Basin Lake Terminus"
                : dsCell && dsCell.tier !== "unexplored"
                  ? dsCell.isOcean || dsCell.elevationM < 0
                    ? `Sea Coast Outlet (Cell #${dsId})`
                    : dsCell.isLake
                      ? `Inland Lake Basin (Cell #${dsId})`
                      : `Downstream Confluence (Cell #${dsId})`
                  : `Downstream Cell #${dsId}`;
            const q = cell.riverDischargeM3S ?? 0;
            const vel = cell.riverVelocityMps ?? 0;
            const kw = cell.hydroPotentialKw ?? 0;
            const hydroText = kw >= 1000 ? `${(kw / 1000).toFixed(2)} MW` : `${Math.round(kw)} kW`;
            const baseBargeKmH = 6.2;
            const downKmH = (baseBargeKmH + vel * 3.6 * 0.85).toFixed(1);
            const upKmH = Math.max(1.7, baseBargeKmH - vel * 3.6 * 0.65).toFixed(1);
            const navLabel = formatGeoKeyLabel(cell.riverNavigability, "Channel");
            const estuaryText = cell.isEstuary ? " · Tidal Estuary" : "";
            const provText = cell.geologyProvince ? ` · ${formatGeoKeyLabel(cell.geologyProvince, "Platform")}` : "";
            title.textContent = `Perennial River Reach (Cell #${cell.cellId})`;
            detail.textContent = `Flow: Cell #${cell.cellId} -> ${terminusLabel} | Discharge: ${q.toFixed(1)} m3/s | Current: ${vel.toFixed(2)} m/s | ${navLabel}${estuaryText}${provText} | Barge Speeds: ${downKmH} km/h downstream / ${upKmH} km/h upstream | Hydro Potential: ${hydroText} | Strahler Order ${cell.strahlerOrder ?? 1}`;
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: cell.observedTick,
                confidence01: cell.confidence01,
                provenanceLabel: cell.provenanceLabel,
                isLiveSight: cell.tier === "surveyed" ? cell.isLiveHqSight : false,
              }),
            );
            appendDamControlSection(document, extras, cell, spine);
            if (dsId !== null) {
              extras.appendChild(
                createClickableToken(document, `Follow Flow Downstream (#${dsId})`, () => {
                  spine.bus.emit({ kind: "FlyGlobeToCell", cellId: dsId });
                }),
              );
            }
          } else {
            title.textContent = `River Reach (Cell #${targetId})`;
            detail.textContent = "River reach lies beyond live survey reports.";
          }
        } else if (targetKind === "cell" && Number.isFinite(targetId)) {
          const allCells = selectKnownCells(spine.store);
          const cell = allCells.find((c) => c.cellId === targetId);
          wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, `Cell #${targetId}`);
          if (!cell || cell.tier === "unexplored") {
            title.textContent = `Unexplored Territory (Cell #${targetId})`;
            detail.textContent = "No expedition survey report has reached HQ for this cell.";
          } else {
            const lakeTag = cell.tier !== "rumored" && cell.isLake ? " · Inland Lake Basin" : "";
            title.textContent = `${cell.tier.toUpperCase()} Survey — Cell #${cell.cellId}${lakeTag}`;
            const centerElevM = Math.round(cell.elevationM);
            const elevSummary =
              clickedLocalElevM !== undefined
                ? `Local ground: ${Math.round(clickedLocalElevM)}m (Cell center node: ${centerElevM}m)`
                : `Cell center node elevation: ${centerElevM}m`;
            detail.textContent = `${elevSummary} | Confidence: ${Math.round(cell.confidence01 * 100)}%`;
            extras.appendChild(
              createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: cell.observedTick,
                confidence01: cell.confidence01,
                provenanceLabel: cell.provenanceLabel,
                isLiveSight: cell.tier === "surveyed" ? cell.isLiveHqSight : false,
              }),
            );

            const geoList = document.createElement("ul");
            geoList.className = "ng-ledger-list";
            appendInspectorDetailRow(
              document,
              geoList,
              "Biome & Strata",
              `${cell.biomeKey ?? (cell.isOcean ? "ocean" : "Surveyed Land")}${
                cell.tier === "surveyed" || cell.tier === "stale"
                  ? ` · ${describeRockStrata(cell.rockStrataCode)}`
                  : ""
              }`,
            );
            if (cell.tier === "surveyed" || cell.tier === "stale") {
              if (cell.geologyProvince !== undefined || cell.roughnessM !== undefined) {
                appendInspectorDetailRow(
                  document,
                  geoList,
                  "Geology & Roughness",
                  `${formatGeoKeyLabel(cell.geologyProvince, cell.isOcean ? "Marine Basin" : "Platform")} · Relief Roughness ${Math.round(cell.roughnessM ?? 28)} m${
                    (cell.faultInstability01 ?? 0) > 0.3 || (cell.volcanicHazard01 ?? 0) > 0.3
                      ? ` · Seismic ${Math.round((cell.faultInstability01 ?? 0) * 100)}% / Volcanic ${Math.round((cell.volcanicHazard01 ?? 0) * 100)}%`
                      : ""
                  }`,
                );
              }
              if (cell.isOcean) {
                if (cell.marineUpwelling01 !== undefined || cell.shelfNutrient01 !== undefined) {
                  appendInspectorDetailRow(
                    document,
                    geoList,
                    "Marine Bathymetry",
                    `Upwelling ${Math.round((cell.marineUpwelling01 ?? 0) * 100)}% · Nutrient Flux ${Math.round((cell.shelfNutrient01 ?? 0) * 100)}%`,
                  );
                }
              } else {
                if (cell.soilDrainage01 !== undefined || cell.soilClay01 !== undefined) {
                  appendInspectorDetailRow(
                    document,
                    geoList,
                    "Soil Profile",
                    `Drainage ${Math.round((cell.soilDrainage01 ?? 0.5) * 100)}% · Clay ${Math.round((cell.soilClay01 ?? 0.35) * 100)}%`,
                  );
                }
                const hydroNotes: string[] = [];
                if (cell.riverNavigability && cell.riverNavigability !== "none") {
                  hydroNotes.push(formatGeoKeyLabel(cell.riverNavigability, "Channel"));
                }
                if (cell.isEstuary) hydroNotes.push("Estuary");
                if (cell.isLake) {
                  hydroNotes.push(
                    `${formatGeoKeyLabel(cell.lakeKind, "Lake")} Lake (${(cell.lakeSalinityPerMille ?? 0.2).toFixed(1)}‰)`,
                  );
                }
                if (cell.isSpringOasis) hydroNotes.push("Aquifer Spring Oasis");
                if (hydroNotes.length > 0) {
                  appendInspectorDetailRow(document, geoList, "Hydrology & Aquifer", hydroNotes.join(" · "));
                }
              }
            }

            const topo = getCachedTopology(snap.globeTopology.subdivision);
            const nbrIds = topo.cells[cell.cellId]?.neighbors ?? [];
            const knownById = new Map(allCells.map((c) => [c.cellId, c]));
            const surveyedNbrElevs: number[] = [];
            for (const nid of nbrIds) {
              const nc = knownById.get(nid);
              if (nc && (nc.tier === "surveyed" || nc.tier === "stale")) {
                surveyedNbrElevs.push(Math.round(nc.elevationM));
              }
            }
            if (surveyedNbrElevs.length > 0) {
              const minNbr = Math.min(...surveyedNbrElevs);
              const maxNbr = Math.max(...surveyedNbrElevs);
              appendInspectorDetailRow(
                document,
                geoList,
                "Interpolated Cell Slope",
                `Terrain slopes between ${minNbr}m and ${maxNbr}m across adjacent surveyed cells`,
              );
            }
            extras.appendChild(geoList);

            if (cell.tier === "surveyed" || cell.tier === "stale") {
              appendDamControlSection(document, extras, cell, spine);
            }
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

          const settlementIdFilter = found?.settlementId;
          const localFacs = selectFacilityRows(spine.store).filter(
            (f) => settlementIdFilter === undefined || f.settlementId === settlementIdFilter,
          );
          if (localFacs.length > 0) {
            const facHeading = document.createElement("strong");
            facHeading.textContent = "Workshops & Infrastructure";
            const facRow = document.createElement("div");
            facRow.className = "ng-hud-cluster";
            for (const f of localFacs.slice(0, 6)) {
              facRow.appendChild(
                createClickableToken(document, `${f.processName} (${f.assignedWorkers}w)`, () => {
                  spine.bus.emit({
                    kind: "FocusEntityInspector",
                    entityKind: "facility",
                    entityId: f.facilityId,
                  });
                }),
              );
            }
            extras.append(facHeading, facRow);
          }

          const connectedRoads = selectSurveyedRoads(spine.store).filter(
            (r) => r.fromCellId === cellId || r.toCellId === cellId,
          );
          if (connectedRoads.length > 0) {
            const roadHeading = document.createElement("strong");
            roadHeading.textContent = "Connected Roads";
            const roadRow = document.createElement("div");
            roadRow.className = "ng-hud-cluster";
            for (const r of connectedRoads) {
              const label = r.purposeLabel ? `${r.purposeLabel} (${r.infraKey})` : `${r.infraKey} #${r.edgeId}`;
              roadRow.appendChild(
                createClickableToken(document, label, () => {
                  spine.bus.emit({
                    kind: "FocusEntityInspector",
                    entityKind: "road",
                    entityId: r.edgeId,
                  });
                }),
              );
            }
            extras.append(roadHeading, roadRow);
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

const KNOWN_CARRIER_KINDS: ReadonlyArray<{ readonly kind: string; readonly label: string }> = [
  { kind: "PractitionerCohort", label: "Cohorts" },
  { kind: "MasterIndividual", label: "Specialists" },
  { kind: "WorkingPrototypeArtifact", label: "Prototypes" },
  { kind: "PhysicalTreatise", label: "Archives" },
];

function buildDisclosure(
  doc: Document,
  summaryTitle: string,
  badgeText: string,
  openByDefault: boolean,
): { readonly details: HTMLDetailsElement; readonly body: HTMLDivElement } {
  const details = doc.createElement("details");
  details.className = "ng-disclosure";
  if (openByDefault) details.open = true;

  const summary = doc.createElement("summary");
  const titleStrong = doc.createElement("strong");
  titleStrong.textContent = summaryTitle;
  const badge = doc.createElement("span");
  badge.className = "ng-prov-badge";
  badge.textContent = badgeText;
  summary.append(titleStrong, badge);

  const body = doc.createElement("div");
  body.className = "ng-disclosure-body";
  details.append(summary, body);
  return { details, body };
}

/**
 * Non-State Economy, Regional Price Spreads & Governance Reform Inspector (`UI-DAT-006.1`, `UI-DAT-006.2`).
 */
export function createInstitutionsInspectorWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "institutions_inspector",
    defaultTitle: "Estates, Markets & Governance",
    minWidth: 480,
    minHeight: 360,
    hotkey: "n",
    mountContent(container, _instanceKey, spine) {
      const tabGroup = createAccessibleTabGroup(document, {
        prefix: "institutions-inspector",
        ariaLabel: "Institutional and Economic Sections",
        tabs: [
          { key: "actors", label: "Estates & Actors" },
          { key: "markets", label: "Spot Prices & Congestion" },
          { key: "governance", label: "Governance & Policy" },
        ],
      });

      const actorsHost = tabGroup.panelInnerByKey.get("actors")!;
      const marketsHost = tabGroup.panelInnerByKey.get("markets")!;
      const govHost = tabGroup.panelInnerByKey.get("governance")!;

      const stateCompareBox = document.createElement("div");
      const actorListBox = document.createElement("div");
      actorsHost.append(stateCompareBox, actorListBox);

      const marketListBox = document.createElement("div");
      const congestionListBox = document.createElement("div");
      marketsHost.append(marketListBox, congestionListBox);

      const govSummaryBox = document.createElement("div");
      const govFormBox = document.createElement("div");
      govFormBox.className = "ng-responsive-grid";

      const modelSelect = document.createElement("select");
      modelSelect.className = "ng-select";
      modelSelect.setAttribute("data-gov-model-select", "true");
      modelSelect.setAttribute("aria-label", "Governance coordination model");
      const modelSubmitBtn = document.createElement("button");
      modelSubmitBtn.type = "button";
      modelSubmitBtn.className = "ng-btn ng-btn-primary";
      modelSubmitBtn.setAttribute("data-gov-model-submit", "true");
      modelSubmitBtn.textContent = "Reform Governance Model";

      const policySelect = document.createElement("select");
      policySelect.className = "ng-select";
      policySelect.setAttribute("data-econ-policy-select", "true");
      policySelect.setAttribute("aria-label", "Economic policy lever");
      const rateInput = document.createElement("input");
      rateInput.type = "number";
      rateInput.className = "ng-text-input";
      rateInput.min = "0";
      rateInput.max = "1";
      rateInput.step = "0.05";
      rateInput.placeholder = "Rate 0..1";
      rateInput.setAttribute("data-econ-rate-input", "true");
      rateInput.setAttribute("aria-label", "Optional tax or tribute rate override");
      const policySubmitBtn = document.createElement("button");
      policySubmitBtn.type = "button";
      policySubmitBtn.className = "ng-btn";
      policySubmitBtn.setAttribute("data-econ-policy-submit", "true");
      policySubmitBtn.textContent = "Enact Policy";

      const charterSettlementSelect = document.createElement("select");
      charterSettlementSelect.className = "ng-select";
      charterSettlementSelect.setAttribute("data-charter-settlement-select", "true");
      charterSettlementSelect.setAttribute("aria-label", "Target settlement for carrier charter");
      const charterCountInput = document.createElement("input");
      charterCountInput.type = "number";
      charterCountInput.className = "ng-text-input";
      charterCountInput.min = "1";
      charterCountInput.max = "100";
      charterCountInput.step = "1";
      charterCountInput.value = "2";
      charterCountInput.setAttribute("data-charter-count-input", "true");
      charterCountInput.setAttribute("aria-label", "Private carrier charter count");
      const charterSubmitBtn = document.createElement("button");
      charterSubmitBtn.type = "button";
      charterSubmitBtn.className = "ng-btn";
      charterSubmitBtn.setAttribute("data-charter-submit", "true");
      charterSubmitBtn.textContent = "Charter Private Carriers";

      const actionStatus = document.createElement("div");
      actionStatus.className = "ng-hud-stat";
      actionStatus.setAttribute("role", "status");

      govFormBox.append(
        modelSelect,
        modelSubmitBtn,
        policySelect,
        rateInput,
        policySubmitBtn,
        charterSettlementSelect,
        charterCountInput,
        charterSubmitBtn,
        actionStatus,
      );
      govHost.append(govSummaryBox, govFormBox);

      modelSubmitBtn.addEventListener("click", () => {
        if (!modelSelect.value) return;
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: { kind: "switchGovernanceModel", modelKey: modelSelect.value },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
        actionStatus.textContent = `Dispatched governance reform order: ${modelSelect.value}`;
      });

      policySubmitBtn.addEventListener("click", () => {
        if (!policySelect.value) return;
        const parsedRate = rateInput.value.trim().length > 0 ? Number(rateInput.value) : null;
        const rateOverride01 =
          parsedRate !== null && Number.isFinite(parsedRate)
            ? Math.max(0, Math.min(1, parsedRate))
            : null;
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: {
            kind: "enactEconomicPolicy",
            policyKey: policySelect.value,
            rateOverride01,
          },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
        actionStatus.textContent = `Dispatched policy decree: ${policySelect.value}`;
      });

      charterSubmitBtn.addEventListener("click", () => {
        const targetSettlement = Number.parseInt(charterSettlementSelect.value, 10);
        const count = Math.max(1, Math.min(1000, Number.parseInt(charterCountInput.value || "1", 10) || 1));
        if (!Number.isInteger(targetSettlement) || targetSettlement < 0) return;
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: { kind: "charterPrivateCarriers", targetSettlement, count },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
        actionStatus.textContent = `Chartered ${count} private carriers for settlement ${targetSettlement}`;
      });

      const render = () => {
        const snap = spine.store.getSnapshot();
        const currentTick = snap?.clockAndNotifications.tick ?? 0;
        const eco = selectNonStateEconomy(spine.store);
        const gov = selectGovernance(spine.store);
        const stateRows = selectLedgerRows(spine.store);
        const settlements = selectKnownSettlements(spine.store);

        const stateHeading = document.createElement("h4");
        stateHeading.textContent = "State Warehouses vs Non-State Actor Ledger";
        const stateSummary = document.createElement("div");
        stateSummary.className = "ng-hud-stat";
        const totalStateKg = stateRows.reduce((acc, r) => acc + r.reportedMassKg, 0);
        stateSummary.textContent = `State Official Ledgers: ${stateRows.length} entries (${Math.round(totalStateKg)} kg tracked across state stores) | Non-State Actors Observed: ${eco?.actors.length ?? 0}`;
        stateCompareBox.replaceChildren(stateHeading, stateSummary);

        const grouped = new Map<string, NonNullable<typeof eco>["actors"][number][]>();
        for (const actor of eco?.actors ?? []) {
          const list = grouped.get(actor.kind) ?? [];
          list.push(actor);
          grouped.set(actor.kind, list);
        }

        const disclosures: HTMLElement[] = [];
        if (grouped.size === 0) {
          const empty = document.createElement("p");
          empty.textContent = "No non-state estates, merchant houses, guilds, or syndicates observed yet.";
          disclosures.push(empty);
        } else {
          for (const [kind, list] of grouped.entries()) {
            const { details, body } = buildDisclosure(
              document,
              `${kind} (${list.length})`,
              `Non-State ${kind}`,
              true,
            );
            details.setAttribute("data-actor-kind-group", kind);
            const ul = document.createElement("ul");
            ul.className = "ng-ledger-list";
            for (const actor of list) {
              const li = document.createElement("li");
              li.className = "ng-ledger-row";
              const info = document.createElement("span");
              info.textContent = `${actor.name} (${actor.settlementName}) — Capital ${Math.round(actor.workingCapitalCoins)} · Stock ${Math.round(actor.heldMassKg)} kg (Hoarded ${Math.round(actor.hoardedMassKg)} kg) · Carriers ${actor.privateCarriersOwned} own / ${actor.charteredToStateCarriers} chartered · Compliance ${Math.round(actor.compliance01 * 100)}%`;
              const badge = createProvenanceBadgeElement(document, {
                currentTick,
                observedTick: actor.observedTick,
                confidence01: actor.confidence01,
                provenanceLabel: actor.provenanceLabel,
              });
              li.append(info, badge);
              ul.appendChild(li);
            }
            body.appendChild(ul);
            disclosures.push(details);
          }
        }
        actorListBox.replaceChildren(...disclosures);

        const marketHeading = document.createElement("h4");
        marketHeading.textContent = "Regional Spot Markets & Informal Price Spreads";
        const marketList = document.createElement("ul");
        marketList.className = "ng-ledger-list";
        const markets = eco?.informalMarkets ?? [];
        if (markets.length === 0) {
          const empty = document.createElement("li");
          empty.textContent = "No informal market pricing ledgers received yet.";
          marketList.appendChild(empty);
        } else {
          for (const m of markets) {
            const li = document.createElement("li");
            li.className = "ng-ledger-row";
            const info = document.createElement("span");
            info.textContent = `${m.settlementName} · ${m.commodityName} — Clearing Index ${m.clearingPriceIndex.toFixed(2)} · Spread +${Math.round(m.blackMarketPremium01 * 100)}% · Hoarded ${Math.round(m.hoardedReserveKg)} kg`;
            const badge = createProvenanceBadgeElement(document, {
              currentTick,
              observedTick: m.observedTick,
              confidence01: m.confidence01,
              provenanceLabel: m.provenanceLabel,
            });
            li.append(info, badge);
            marketList.appendChild(li);
          }
        }
        marketListBox.replaceChildren(marketHeading, marketList);

        const congHeading = document.createElement("h4");
        congHeading.textContent = "Shared Transport Corridor Bottlenecks";
        const congList = document.createElement("ul");
        congList.className = "ng-ledger-list";
        const congested = eco?.congestedEdges ?? [];
        if (congested.length === 0) {
          const empty = document.createElement("li");
          empty.textContent = "No overloaded shared logistics corridors reported.";
          congList.appendChild(empty);
        } else {
          for (const edge of congested) {
            const li = document.createElement("li");
            li.className = "ng-ledger-row";
            li.textContent = `Edge ${edge.edgeId} (Cell ${edge.fromCellId} -> Cell ${edge.toCellId}) — Flow ${edge.hourlyFlowTonnes.toFixed(1)}t / Cap ${edge.capacityTonnes.toFixed(1)}t · Congestion ${edge.congestionMultiplier.toFixed(2)}x`;
            congList.appendChild(li);
          }
        }
        congestionListBox.replaceChildren(congHeading, congList);

        if (gov) {
          const inertiaPct = Math.round(gov.adaptationProgress01 * 100);
          const heading = document.createElement("h4");
          heading.textContent = `Active Governance: ${gov.activeModelName}`;
          const stats = document.createElement("div");
          stats.className = "ng-hud-stat";
          stats.textContent = `Policy: ${gov.activePolicyName} (${Math.round(gov.taxOrTributeRate01 * 100)}%) · Reform Inertia ${inertiaPct}% · Skimming Leakage ${Math.round(gov.skimmingLeakageFraction * 100)}% · Capture Incidents ${gov.bureaucraticCaptureEvents} · Chartered Carriers ${gov.charteredCarriersTotal}`;
          const inertiaBar = document.createElement("div");
          inertiaBar.className = "ng-inertia-bar";
          inertiaBar.setAttribute("role", "progressbar");
          inertiaBar.setAttribute("aria-valuemin", "0");
          inertiaBar.setAttribute("aria-valuemax", "100");
          inertiaBar.setAttribute("aria-valuenow", String(inertiaPct));
          inertiaBar.setAttribute("aria-label", "Governance reform adaptation progress");
          const fill = document.createElement("div");
          fill.className = "ng-inertia-bar-fill";
          fill.style.width = `${inertiaPct}%`;
          inertiaBar.appendChild(fill);
          govSummaryBox.replaceChildren(heading, stats, inertiaBar);

          syncSelectOptions(
            modelSelect,
            gov.availableModels.map((m) => ({
              value: m.modelKey,
              label: `${m.displayName} (Attn ${m.baseAttentionCapacity})`,
            })),
          );
          syncSelectOptions(
            policySelect,
            gov.availablePolicies.map((p) => ({
              value: p.policyKey,
              label: `${p.displayName} (${Math.round(p.taxOrTributeRate01 * 100)}%)`,
            })),
          );
        } else {
          const wait = document.createElement("div");
          wait.className = "ng-hud-stat";
          wait.textContent = "Awaiting governance ledger telemetry.";
          govSummaryBox.replaceChildren(wait);
        }

        syncSelectOptions(
          charterSettlementSelect,
          settlements.map((s) => ({
            value: String(s.settlementId),
            label: `${s.settlementName} (Cell ${s.cellId})`,
          })),
        );
      };

      render();
      container.replaceChildren(tabGroup.root);
      const unsub = spine.store.subscribe(render);
      return unsub;
    },
  };
}

/**
 * Diegetic Empirical Archive, Demonstrated Know-How, Treatises & 4-Carrier Fragility Monitor
 * (`UI-DAT-007.1`, `UI-DAT-007.2`). Never renders unresearched future tech nodes or global science points.
 */
export function createKnowledgeArchiveWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "knowledge_archive",
    defaultTitle: "Empirical Archive & Fragility",
    minWidth: 680,
    minHeight: 580,
    hotkey: "k",
    mountContent(container, _instanceKey, spine) {
      const tabGroup = createAccessibleTabGroup(document, {
        prefix: "knowledge-archive",
        ariaLabel: "Empirical Knowledge Archive Sections",
        tabs: [
          { key: "capabilities", label: "Know-How & Fragility" },
          { key: "treatises", label: "Materials & Treatises" },
          { key: "patents", label: "Patents & Mode" },
        ],
      });

      const capHost = tabGroup.panelInnerByKey.get("capabilities")!;
      const treatiseHost = tabGroup.panelInnerByKey.get("treatises")!;
      const patentsHost = tabGroup.panelInnerByKey.get("patents")!;

      const patentSummaryBox = document.createElement("div");
      const modeFormBox = document.createElement("div");
      modeFormBox.className = "ng-responsive-grid";

      const modeSettlementSelect = document.createElement("select");
      modeSettlementSelect.className = "ng-select";
      modeSettlementSelect.setAttribute("data-innovation-settlement-select", "true");
      modeSettlementSelect.setAttribute("aria-label", "Settlement for innovation mode");

      const modeKeySelect = document.createElement("select");
      modeKeySelect.className = "ng-select";
      modeKeySelect.setAttribute("data-innovation-mode-select", "true");
      modeKeySelect.setAttribute("aria-label", "Innovation institution mode");

      const modeSubmitBtn = document.createElement("button");
      modeSubmitBtn.type = "button";
      modeSubmitBtn.className = "ng-btn ng-btn-primary";
      modeSubmitBtn.setAttribute("data-innovation-submit", "true");
      modeSubmitBtn.textContent = "Set Innovation Mode";

      const modeStatus = document.createElement("div");
      modeStatus.className = "ng-hud-stat";
      modeStatus.setAttribute("role", "status");

      modeFormBox.append(modeSettlementSelect, modeKeySelect, modeSubmitBtn, modeStatus);
      patentsHost.append(patentSummaryBox, modeFormBox);

      modeSubmitBtn.addEventListener("click", () => {
        const settlement = Number.parseInt(modeSettlementSelect.value, 10);
        if (!Number.isInteger(settlement) || settlement < 0 || !modeKeySelect.value) return;
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: { kind: "setInnovationMode", settlement, modeKey: modeKeySelect.value },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
        modeStatus.textContent = `Updated settlement ${settlement} innovation mode to ${modeKeySelect.value}`;
      });

      const render = () => {
        const snap = spine.store.getSnapshot();
        const currentTick = snap?.clockAndNotifications.tick ?? 0;
        const knowledge = selectKnowledge(spine.store);
        const ledgerRows = selectLedgerRows(spine.store);

        const capChildren: HTMLElement[] = [];
        const treatiseChildren: HTMLElement[] = [];
        const patentChildren: HTMLElement[] = [];

        const assayedHeading = document.createElement("h4");
        assayedHeading.textContent = "Empirically Assayed Stockpiles & Materials";
        const assayedList = document.createElement("ul");
        assayedList.className = "ng-ledger-list";
        for (const row of ledgerRows.slice(0, 10)) {
          const li = document.createElement("li");
          li.className = "ng-ledger-row";
          const txt = document.createElement("span");
          txt.textContent = `${row.settlementName} · ${row.commodityName} — Reported ${Math.round(row.reportedMassKg)} kg [${Math.round(row.minEstKg)}..${Math.round(row.maxEstKg)}]`;
          const badge = createProvenanceBadgeElement(document, {
            currentTick,
            observedTick: row.observedTick,
            confidence01: row.confidence01 ?? 0.85,
            provenanceLabel: row.provenanceLabel,
            reporterName: row.reporterName,
          });
          li.append(txt, badge);
          assayedList.appendChild(li);
        }
        treatiseChildren.push(assayedHeading, assayedList);

        if (!knowledge || knowledge.settlements.length === 0) {
          const p = document.createElement("p");
          p.textContent = "No demonstrated craft capabilities or treatises archived yet.";
          capChildren.push(p);
          patentSummaryBox.replaceChildren(p.cloneNode(true));
        } else {
          const allCarriers = knowledge.carriers;
          for (const st of knowledge.settlements) {
            const stCard = document.createElement("section");
            stCard.className = "ng-subordinate-card";
            stCard.setAttribute("data-knowledge-settlement", String(st.settlementId));

            const stHead = document.createElement("div");
            stHead.className = "ng-ledger-row";
            const stTitle = document.createElement("strong");
            stTitle.textContent = `${st.settlementName} — Mode: ${st.innovationModeKey}`;
            const stProv = createProvenanceBadgeElement(document, {
              currentTick,
              observedTick: st.observedTick,
              confidence01: st.confidence01,
              provenanceLabel: st.provenanceLabel,
            });
            stHead.append(stTitle, stProv);
            stCard.appendChild(stHead);

            const demonstrated = st.knowHow.filter((kh) => kh.knowHow01 > 0);
            if (demonstrated.length === 0) {
              const none = document.createElement("div");
              none.className = "ng-hud-stat";
              none.textContent = "No demonstrated physical components yet.";
              stCard.appendChild(none);
            } else {
              const ul = document.createElement("ul");
              ul.className = "ng-ledger-list";
              for (const kh of demonstrated) {
                const matching = allCarriers.filter(
                  (c) => c.settlementId === st.settlementId && c.componentKey === kh.componentKey,
                );
                const presentKindCount = KNOWN_CARRIER_KINDS.filter((spec) =>
                  matching.some((m) => m.carrierKind === spec.kind && m.retentionStrength01 > 0),
                ).length;
                const carrierBreakdown = KNOWN_CARRIER_KINDS.map((spec) => {
                  const count = matching.filter((m) => m.carrierKind === spec.kind).length;
                  return `${spec.label} ${count}`;
                }).join(" | ");

                const li = document.createElement("li");
                li.className = "ng-ledger-row";
                li.setAttribute("data-component-key", kh.componentKey);

                const descSpan = document.createElement("span");
                descSpan.textContent = `${kh.displayName} (Mastery ${Math.round(kh.knowHow01 * 100)}%) — Carriers: ${carrierBreakdown}`;

                const fragilityBadge = document.createElement("span");
                fragilityBadge.className = "ng-fragility-badge";
                if (presentKindCount <= 1) {
                  fragilityBadge.setAttribute("data-fragility-risk", "endangered");
                  fragilityBadge.textContent =
                    presentKindCount === 0
                      ? "Endangered: Zero Active Carriers"
                      : "Endangered: Single-Carrier Extinction Risk";
                } else {
                  fragilityBadge.setAttribute("data-fragility-risk", "replicated");
                  fragilityBadge.textContent = `Replicated (${presentKindCount}/4 Carriers)`;
                }

                li.append(descSpan, fragilityBadge);
                ul.appendChild(li);
              }
              stCard.appendChild(ul);
            }
            capChildren.push(stCard);

            const trCard = document.createElement("section");
            trCard.className = "ng-subordinate-card";
            const trHead = document.createElement("strong");
            trHead.textContent = `${st.settlementName} — Published Know-Why Treatises (Bottleneck ${Math.round(st.bottleneckPressure01 * 100)}% · Leisure ${Math.round(st.leisureSurplus01 * 100)}%)`;
            trCard.appendChild(trHead);
            const activeDomains = st.knowWhy.filter((d) => d.knowWhy01 > 0);
            const domUl = document.createElement("ul");
            domUl.className = "ng-ledger-list";
            for (const dom of activeDomains) {
              const li = document.createElement("li");
              li.textContent = `${dom.displayName}: ${Math.round(dom.knowWhy01 * 100)}% formalized`;
              domUl.appendChild(li);
            }
            if (activeDomains.length === 0) {
              const empty = document.createElement("li");
              empty.textContent = "Empirical practice only; zero formalized treatises published yet.";
              domUl.appendChild(empty);
            }
            trCard.appendChild(domUl);
            treatiseChildren.push(trCard);

            const patLine = document.createElement("div");
            patLine.className = "ng-subordinate-card";
            patLine.textContent = `${st.settlementName} — Active Patent Holder: ${st.activePatentHolderActorId ?? "None"} · Expiry Tick: ${st.patentExpiryTick ?? "N/A"} · Knowledge Regression Losses: ${st.regressionLosses}`;
            patentChildren.push(patLine);
          }

          syncSelectOptions(
            modeSettlementSelect,
            knowledge.settlements.map((s) => ({
              value: String(s.settlementId),
              label: s.settlementName,
            })),
          );
          syncSelectOptions(
            modeKeySelect,
            knowledge.availableModes.map((m) => ({
              value: m,
              label: m,
            })),
          );
          patentSummaryBox.replaceChildren(...patentChildren);
        }

        capHost.replaceChildren(...capChildren);
        treatiseHost.replaceChildren(...treatiseChildren);
      };

      render();
      container.replaceChildren(tabGroup.root);
      const unsub = spine.store.subscribe(render);
      return unsub;
    },
  };
}
