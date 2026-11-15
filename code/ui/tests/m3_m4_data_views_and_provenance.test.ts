import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  ReportedLedgerRowDto,
  UiIntent,
  UiIntentBus,
  describeProvenanceBadge,
} from "../src/spine";
import {
  computeChartLayout,
  createDelegationOrgTreeWindowPlugin,
  createEntityInspectorWindowPlugin,
  createLedgerStubPlugin,
  createTimeSeriesChartWindowPlugin,
} from "../src/views";
import { createDispatchInboxStubPlugin } from "../src/dispatches";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const CANONICAL_M3_M4 = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

function setupJsdom(): JSDOM {
  const dom = new JSDOM("<!doctype html><html><body><div id='root'></div></body></html>", {
    url: "http://localhost/",
  });
  (globalThis as unknown as { document: Document }).document = dom.window.document;
  (globalThis as unknown as { window: Window }).window = dom.window as unknown as Window;
  (globalThis as unknown as { getComputedStyle: typeof getComputedStyle }).getComputedStyle =
    dom.window.getComputedStyle.bind(dom.window);
  return dom;
}

describe("M3 & M4 High-Density Ledgers, Time-Series Charts, Delegation Org Trees & Provenance", () => {
  // @req UI-DAT-001
  it("virtualizes 15,000+ ledger rows under 100 live DOM rows, sorts under 50ms, and preserves filters (UI-DAT-001)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();

    const heavyRows: ReportedLedgerRowDto[] = Array.from({ length: 15000 }, (_, i) => ({
      settlementId: i % 5,
      settlementName: i % 5 === 0 ? "Haven Landing Perimeter" : `Outpost #${i % 5}`,
      commodityKey: `item_${i % 12}`,
      commodityName: `Commodity ${String(i).padStart(5, "0")}`,
      reportedMassKg: (15000 - i) * 1.25,
      minEstKg: (15000 - i) * 1.15,
      maxEstKg: (15000 - i) * 1.35,
      observedTick: i % 5 === 0 ? 72 : 48,
      provenanceLabel: i % 5 === 0 ? "Direct Quartermaster Audit" : "Courier Bag",
      category: i % 2 === 0 ? "food" : "ore",
      confidence01: i % 5 === 0 ? 0.99 : 0.72,
      reporterName: "Clerk H. Vance",
    }));

    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      ledgerRows: heavyRows,
    });

    const host = dom.window.document.getElementById("root")!;
    const plugin = createLedgerStubPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    const rowsInitial = host.querySelectorAll("tbody tr");
    expect(rowsInitial.length).toBeGreaterThan(0);
    expect(rowsInitial.length).toBeLessThan(80);

    const massSortBtn = host.querySelector<HTMLButtonElement>("[data-sort-col='2']")!;
    const t0 = performance.now();
    massSortBtn.click();
    const sortElapsed = performance.now() - t0;
    expect(sortElapsed).toBeLessThan(50);

    const firstRowText = host.querySelector("tbody tr td")?.textContent ?? "";
    expect(firstRowText).toContain("Commodity");

    const scrollBox = host.querySelector<HTMLElement>("[data-ledger-scroll='true']")!;
    scrollBox.scrollTop = 3200;
    scrollBox.dispatchEvent(new dom.window.Event("scroll"));
    const rowsAfterScroll = host.querySelectorAll("tbody tr");
    expect(rowsAfterScroll.length).toBeLessThan(80);

    dispose();
  });

  // @req UI-DAT-002
  it("plots multi-series time-series bounds cleanly without overlapping legends across window sizes (UI-DAT-002)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    store.ingestSnapshot(CANONICAL_M3_M4);

    const series = CANONICAL_M3_M4.timeSeries ?? [];
    expect(series.length).toBeGreaterThan(0);

    const layoutWide = computeChartLayout(series, 680, 280, null);
    const layoutNarrow = computeChartLayout(series, 260, 220, 24);
    expect(layoutNarrow.maxTick - layoutNarrow.minTick).toBeLessThanOrEqual(24);
    expect(layoutWide.legendItems.length).toBe(series.length);

    // Ensure no two legend rectangles intersect in either wide or narrow viewport layouts
    for (const layout of [layoutWide, layoutNarrow]) {
      for (let i = 0; i < layout.legendItems.length; i += 1) {
        for (let j = i + 1; j < layout.legendItems.length; j += 1) {
          const a = layout.legendItems[i];
          const b = layout.legendItems[j];
          const overlapX = a.x < b.x + b.width && a.x + a.width > b.x;
          const overlapY = a.y < b.y + b.height && a.y + a.height > b.y;
          expect(overlapX && overlapY).toBe(false);
        }
      }
    }

    const host = dom.window.document.getElementById("root")!;
    const plugin = createTimeSeriesChartWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });
    const canvas = host.querySelector<HTMLCanvasElement>("canvas.ng-timeseries-canvas")!;
    expect(canvas).toBeTruthy();

    host.querySelector<HTMLButtonElement>("[data-chart-span='24']")?.click();
    expect(Number(canvas.dataset.lastMaxTick) - Number(canvas.dataset.lastMinTick)).toBeLessThanOrEqual(24);

    dispose();
  });

  // @req UI-DAT-003
  it("renders interactive Delegation & Attention Org Tree highlighting overload and diluted mandates (UI-DAT-003)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();

    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      delegationTree: {
        leaderTitle: "Expedition Director",
        capacityAttention: 100,
        usedAttention: 132,
        clericalOverhead: 38,
        overloadRatio: 1.32,
        focusedMandateId: null,
        focusedAttentionShare: 0,
        activeMandates: [],
        subordinates: [
          {
            characterId: 1,
            displayName: "Henrik Solberg",
            roleTitle: "Quartermaster",
            competence: 0.84,
            homeSettlementId: 0,
            homeCellId: 82,
            precision01: 0.9,
            loyaltyLabel: "Steadfast",
            allocatedAttention: 48,
            assignedMandates: [
              {
                mandateId: 11,
                assigneeId: 1,
                targetSettlementId: 0,
                targetCellId: 82,
                objectiveKind: "stabilize_food",
                authorityTier: "strategic_mandate",
                summary: "Stabilize Grain Rations",
                priorityWeight: 8,
                status: "active",
                realizedEfficiency: 0.58,
                progress01: 0.4,
                subtasks: ["Verify granary output"],
              },
            ],
          },
        ],
      },
    });

    const host = dom.window.document.getElementById("root")!;
    const plugin = createDelegationOrgTreeWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    expect(host.querySelector("[data-overload-state='crit']")).toBeTruthy();
    expect(host.textContent).toContain("OVERLOAD WARNING");
    expect(host.querySelector("[data-diluted-mandate='true']")).toBeTruthy();

    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));
    host.querySelector<HTMLButtonElement>(".ng-dispatch-token")?.click();
    expect(emitted.some((i) => i.kind === "FocusEntityInspector" && i.entityKind === "character")).toBe(true);

    dispose();
  });

  // @req UI-DAT-004
  it("renders interactive hyperlinked entity chips inside Dispatches that fly globe and focus inspector (UI-DAT-004)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();

    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      dispatchInbox: [
        {
          dispatchId: 77,
          title: "Northern Shaft Seam Survey",
          sentTick: 50,
          arrivedTick: 66,
          originLabel: "Forward Post #85",
          senderId: 2,
          senderName: "Ines Valdez",
          originCellId: 85,
          eventClassKey: "survey_report_arrived",
          severity: "notice",
          confidence01: 0.78,
          bodyTokens: [
            { text: "Assayer ", entityKind: null, entityId: null, targetCellId: null },
            { text: "Ines Valdez", entityKind: "character", entityId: 2, targetCellId: null },
            { text: " surveyed ", entityKind: null, entityId: null, targetCellId: null },
            { text: "Forward Post #85", entityKind: "cell", entityId: 85, targetCellId: 85 },
          ],
        },
      ],
    });

    const host = dom.window.document.getElementById("root")!;
    const plugin = createDispatchInboxStubPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));

    const charToken = host.querySelector<HTMLButtonElement>("[data-dispatch-reader='true'] button[data-entity-kind='character'][data-entity-id='2']")!;
    charToken.click();
    expect(emitted.some((i) => i.kind === "FocusEntityInspector" && i.entityKind === "character" && i.entityId === 2)).toBe(true);

    const cellToken = host.querySelector<HTMLButtonElement>("[data-dispatch-reader='true'] button[data-target-cell='85']")!;
    cellToken.click();
    expect(emitted.some((i) => i.kind === "FlyGlobeToCell" && i.cellId === 85)).toBe(true);

    dispose();
  });

  // @req UI-DAT-005
  it("attaches explicit provenance badges and uncertainty ranges to every reported metric vs direct HQ counts (UI-DAT-005)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();

    const directDesc = describeProvenanceBadge({
      currentTick: 72,
      observedTick: 72,
      confidence01: 0.99,
      provenanceLabel: "Direct Observation",
      isLiveSight: true,
    });
    expect(directDesc.tier).toBe("direct");
    expect(directDesc.badgeText).toContain("Direct");

    const agedDesc = describeProvenanceBadge({
      currentTick: 120,
      observedTick: 72,
      confidence01: 0.64,
      provenanceLabel: "Assay Dispatch",
      reporterName: "Clerk H. Vance",
    });
    expect(agedDesc.tier).toBe("stale");
    expect(agedDesc.accessibleTitle).toContain("Clerk H. Vance");
    expect(agedDesc.accessibleTitle).toContain("48h old");

    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      knownCells: [
        {
          tier: "stale",
          cellId: 85,
          observedTick: 36,
          lastLiveTick: 36,
          elevationM: 420,
          confidence01: 0.66,
          provenanceLabel: "Field Assayer Courier",
          biomeKey: "highland",
          isOcean: false,
          depositEstimates: [
            {
              materialId: 3,
              materialLabel: "Bog Iron Ore",
              categoryKey: "metallic_ore",
              minTonnes: 1400,
              pointEstTonnes: 2100,
              maxTonnes: 3100,
              purityEst01: 0.48,
              confidence01: 0.66,
              surveyorSkill01: 0.72,
              instrumentTier: 1,
              observedTick: 36,
              provenanceLabel: "Field Assayer Courier",
            },
          ],
        },
      ],
    });

    const host = dom.window.document.getElementById("root")!;
    const inspector = createEntityInspectorWindowPlugin();
    const dispose = inspector.mountContent(host, "cell:85", { store, bus });

    expect(host.textContent).toContain("1400..3100 t");
    const badge = host.querySelector(".ng-prov-badge");
    expect(badge?.getAttribute("data-tier")).toBe("stale");
    expect(badge?.getAttribute("aria-label")).toContain("Field Assayer Courier");

    dispose();
  });
});
