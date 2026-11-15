import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import { describe, expect, it } from "vitest";
import {
  DiegeticSnapshot,
  DiegeticStore,
  MockSnapshotPlayer,
  SkinManifestBundle,
  UiIntent,
  UiIntentBus,
  UserPreferencesDto,
} from "../src/spine";
import {
  createArbitrationWindowPlugin,
  createAttentionZoomWindowPlugin,
  createHqRelocationPlannerWindowPlugin,
  createMandateComposerWindowPlugin,
  createNotificationMatrixWindowPlugin,
  mountCommandHud,
} from "../src/hud";
import { mountAudioController } from "../src/audio";
import { DesktopWindowManager } from "../src/windows";
import { PluginRegistry } from "../src/spine";

const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m3_m4_snapshot.json");
const CANONICAL_M3_M4 = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot;

function setupJsdom(): JSDOM {
  const dom = new JSDOM("<!doctype html><html><body><div id='root'></div></body></html>", {
    url: "http://localhost/",
  });
  (globalThis as unknown as { document: Document }).document = dom.window.document;
  (globalThis as unknown as { window: Window }).window = dom.window as unknown as Window;
  return dom;
}

describe("M4 Mandate Composer, Arbitration, Attention Zoom, HQ Relocation, Notifications & Foley", () => {
  // @req UI-HUD-002
  it("previews Attention overload warning and outbound courier ETA prior to dispatching IssueMandate (UI-HUD-002)", async () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    const mock = new MockSnapshotPlayer([
      {
        ...CANONICAL_M3_M4,
        delegationTree: {
          ...CANONICAL_M3_M4.delegationTree,
          capacityAttention: 100,
          usedAttention: 96,
        },
      },
    ]);
    store.ingestSnapshot((await mock.getInitialState() as { state: "ready"; snapshot: DiegeticSnapshot }).snapshot);

    const emitted: UiIntent[] = [];
    bus.subscribe((i) => {
      emitted.push(i);
      if (i.kind === "DispatchSimCommand") {
        void mock.sendPlayerCommand(i.command);
      }
    });

    const host = dom.window.document.getElementById("root")!;
    const plugin = createMandateComposerWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    const preview = host.querySelector<HTMLOutputElement>("[data-mandate-preview='true']")!;
    expect(preview.textContent).toContain("Overload Warning");
    expect(preview.textContent).toContain("Courier ETA:");

    host.querySelector<HTMLFormElement>("form")!.dispatchEvent(new dom.window.Event("submit", { cancelable: true }));
    expect(mock.recordedCommands.some((c) => c.kind === "issueMandate")).toBe(true);
    expect(emitted.some((i) => i.kind === "PlayUiFoley" && i.cueId === "mandate_seal")).toBe(true);

    dispose();
  });

  // @req UI-HUD-003
  it("renders Arbitration options and turnaround ETA, submits ResolveArbitrationProposal, and closes window (UI-HUD-003)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();

    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      openProposals: [
        {
          proposalId: 19,
          settlementId: 0,
          originCellId: 82,
          raisedById: 2,
          raisedByName: "Ines Valdez",
          title: "Flooded Main Shaft Pumping Dispute",
          summary: "Coal pumps and timber bracings compete for spare artificer teams.",
          createdTick: 70,
          deadlineTick: 94,
          options: [
            {
              index: 0,
              label: "Prioritize Dewatering",
              description: "Divert smiths to repair drainage pumps.",
              attentionCost: 8,
            },
            {
              index: 1,
              label: "Timber Shore Fallback",
              description: "Seal upper drift and preserve timber stores.",
              attentionCost: 4,
            },
          ],
        },
      ],
    });

    const host = dom.window.document.getElementById("root")!;
    const registry = new PluginRegistry();
    registry.registerWindowPlugin(createArbitrationWindowPlugin());
    const wm = new DesktopWindowManager(host, registry, { store, bus });
    const winId = wm.openWindow("arbitration_panel")!;

    const radioOpt1 = host.querySelector<HTMLInputElement>("input[type='radio'][value='1']")!;
    radioOpt1.checked = true;
    radioOpt1.dispatchEvent(new dom.window.Event("change"));

    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));

    host.querySelector<HTMLButtonElement>("[data-resolve-proposal='19']")!.click();
    expect(
      emitted.some(
        (i) =>
          i.kind === "DispatchSimCommand" &&
          i.command.kind === "resolveArbitrationProposal" &&
          i.command.proposalId === 19 &&
          i.command.chosenOptionIndex === 1,
      ),
    ).toBe(true);
    expect(wm.getFrameSnapshot(winId)).toBeNull();

    wm.dispose();
  });

  // @req UI-HUD-004
  it("toggles Personal Attention Zoom allocation and surfaces dilution warnings on other branches (UI-HUD-004)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    const mock = new MockSnapshotPlayer([
      {
        ...CANONICAL_M3_M4,
        delegationTree: {
          ...CANONICAL_M3_M4.delegationTree,
          focusedMandateId: null,
          focusedAttentionShare: 0,
          activeMandates: [
            {
              mandateId: 101,
              assigneeId: 1,
              targetSettlementId: 0,
              targetCellId: 82,
              objectiveKind: "stabilize_food",
              authorityTier: "strategic_mandate",
              summary: "Haven Harvest Drive",
              priorityWeight: 7,
              status: "active",
              realizedEfficiency: 0.92,
              progress01: 0.5,
              subtasks: [],
            },
            {
              mandateId: 102,
              assigneeId: 2,
              targetSettlementId: 0,
              targetCellId: 85,
              objectiveKind: "survey_frontier",
              authorityTier: "expeditionary",
              summary: "North Ridge Survey",
              priorityWeight: 5,
              status: "active",
              realizedEfficiency: 0.88,
              progress01: 0.3,
              subtasks: [],
            },
          ],
        },
      },
    ]);

    bus.subscribe((i) => {
      if (i.kind === "DispatchSimCommand") {
        void mock.sendPlayerCommand(i.command).then(() => {
          void mock.subscribeSnapshots((s) => store.ingestSnapshot(s))();
        });
      }
    });
    mock.subscribeSnapshots((s) => store.ingestSnapshot(s));

    const host = dom.window.document.getElementById("root")!;
    const plugin = createAttentionZoomWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    const focusBtn = host.querySelector<HTMLButtonElement>("[data-focus-mandate='102']")!;
    focusBtn.click();

    expect(
      mock.recordedCommands.some(
        (c) => c.kind === "setPersonalAttentionZoom" && c.targetMandateId === 102,
      ),
    ).toBe(true);
    const warnText = host.querySelector("[data-dilution-warning='true']")?.textContent ?? "";
    expect(warnText).toContain("Oversight Dilution Warning");

    dispose();
  });

  // @req UI-HUD-005
  it("previews HQ relocation route highlight and Haven courier latency delta before issuing RelocateHq (UI-HUD-005)", () => {
    const dom = setupJsdom();
    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    store.ingestSnapshot(CANONICAL_M3_M4);

    const routeIntents: number[] = [];
    const commands: string[] = [];
    bus.subscribe((i) => {
      if (i.kind === "PreviewHqLatencyRoute" && i.destinationCellId !== null) {
        routeIntents.push(i.destinationCellId);
      } else if (i.kind === "DispatchSimCommand") {
        commands.push(i.command.kind);
      }
    });

    const host = dom.window.document.getElementById("root")!;
    const plugin = createHqRelocationPlannerWindowPlugin();
    const dispose = plugin.mountContent(host, "default", { store, bus });

    expect(routeIntents.length).toBeGreaterThan(0);
    const preview = host.querySelector("[data-hq-latency-preview='true']")?.textContent ?? "";
    expect(preview).toContain("Travel");
    expect(preview).toContain("Haven Courier Latency");

    host.querySelector<HTMLFormElement>("form")!.dispatchEvent(new dom.window.Event("submit", { cancelable: true }));
    expect(commands).toContain("relocateHq");

    dispose();
  });

  // @req UI-HUD-006
  it("persists Notification Matrix rule changes across restarts and updates HUD auto-pause banner (UI-HUD-006)", async () => {
    const dom = setupJsdom();
    let storedPrefs: UserPreferencesDto = {
      audioMixer: {
        masterVolume: 0.85,
        musicVolume: 0.7,
        ambienceVolume: 0.8,
        foleyVolume: 0.75,
        masterMuted: false,
        musicMuted: false,
        ambienceMuted: false,
        foleyMuted: false,
      },
      namedLayoutPresets: {},
      notificationRules: {},
    };

    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    store.ingestSnapshot(CANONICAL_M3_M4);

    const hudHost = dom.window.document.createElement("div");
    const winHost = dom.window.document.createElement("div");
    dom.window.document.getElementById("root")!.append(hudHost, winHost);

    const unsubHud = mountCommandHud(hudHost, store, bus);
    const matrix = createNotificationMatrixWindowPlugin();
    const unsubMatrix = matrix.mountContent(winHost, "default", {
      store,
      bus,
      readPreferences: async () => storedPrefs,
      writePreferences: async (next) => {
        storedPrefs = next;
      },
    });

    const sel = winHost.querySelector<HTMLSelectElement>("[data-notif-rule='winter_crop_blight']")!;
    sel.value = "DispatchPopupAndAutoPause";
    sel.dispatchEvent(new dom.window.Event("change"));
    await new Promise((r) => setTimeout(r, 20));

    expect(storedPrefs.notificationRules?.winter_crop_blight).toBe("DispatchPopupAndAutoPause");

    // Verify HUD Auto-Pause banner surfaces when an event triggers auto-pause
    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      clockAndNotifications: {
        ...CANONICAL_M3_M4.clockAndNotifications,
        paused: true,
        autoPauseReason: "Winter Crop Blight Escalation",
      },
    });
    const banner = dom.window.document.getElementById("hud-autopause-banner")!;
    expect(banner.hidden).toBe(false);
    expect(banner.textContent).toContain("Winter Crop Blight Escalation");

    unsubMatrix();
    unsubHud();
  });

  // @req UI-AUD-003
  it("triggers skin-mapped UI foley cues on dispatch arrival, seals, and tabs with debounce throttling (UI-AUD-003)", () => {
    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    store.ingestSnapshot(CANONICAL_M3_M4);

    const audio = mountAudioController(bus, store);
    const skinBundle: SkinManifestBundle = {
      skinId: "cartographer-parchment",
      displayName: "Cartographer",
      stylesheetHref: "",
      cssText: "",
      globePalette: {
        oceanShallowRgb: [135, 163, 173],
        oceanDeepRgb: [61, 89, 102],
        landLowRgb: [115, 138, 87],
        landMidRgb: [176, 162, 116],
        landHighRgb: [140, 114, 88],
        peakSnowRgb: [244, 239, 228],
        riverWaterRgb: [54, 108, 133],
        roadOchreRgb: [166, 89, 40],
        settlementMarkerRgb: [139, 46, 34],
        contourInkRgb: [59, 44, 29],
        compassRoseRgb: [110, 85, 57],
        unexploredVellumRgb: [230, 216, 185],
        parchmentGrainTextureUrl: "",
        hatchPatternTextureUrl: "",
        terrainBiomeTextureUrl: "",
      },
      audioManifest: {
        schemaVersion: 1,
        skinId: "cartographer-parchment",
        ambientSurface: {
          fadeStartAltitude: 0.2,
          fadeEndAltitude: 0.72,
          windLoopUrl: "skin://wind",
          rainLoopUrl: "skin://rain",
          settlementLoopUrl: "skin://town",
        },
        ambientMapStudy: {
          fadeStartAltitude: 0.25,
          fadeEndAltitude: 0.78,
          studyLoopUrl: "skin://study",
        },
        music: { crossfadeMs: 2500, stemUrls: ["skin://music"] },
        uiFoley: {
          cooldownMs: 80,
          cueUrls: {
            mandate_seal: "skin://seal",
            dispatch_seal_break: "skin://open",
            ledger_page_turn: "skin://turn",
            alert_bell: "skin://bell",
          },
        },
      },
    };
    bus.emit({ kind: "SkinBundleUpdated", bundle: skinBundle });

    // Cooldown debounce throttles duplicate rapid triggers within cooldownMs
    const first = audio.triggerFoleyCue("mandate_seal", 1000);
    const blocked = audio.triggerFoleyCue("mandate_seal", 1025);
    const allowedAgain = audio.triggerFoleyCue("mandate_seal", 1120);
    expect(first).toBe(true);
    expect(blocked).toBe(false);
    expect(allowedAgain).toBe(true);

    // Incoming dispatch automatically rings alert_bell cue
    store.ingestSnapshot({
      ...CANONICAL_M3_M4,
      dispatchInbox: [
        ...CANONICAL_M3_M4.dispatchInbox,
        {
          dispatchId: 501,
          title: "Frontier Courier Arrived",
          sentTick: 70,
          arrivedTick: 73,
          originLabel: "Forward Post",
        },
      ],
    });
    expect(audio.getPlayedFoleyLog().some((r) => r.cueId === "alert_bell")).toBe(true);

    audio.dispose();
  });
});
