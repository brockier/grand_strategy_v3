import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { JSDOM } from "jsdom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  AudioManifestDto,
  DiegeticSnapshot,
  DiegeticStore,
  MockSnapshotPlayer,
  PluginRegistry,
  SkinManifestBundle,
  UiIntent,
  UiIntentBus,
  ValidatedPlayerCommand,
} from "../src/spine";
import { mountCommandHud } from "../src/hud";
import {
  AssetResolver,
  SkinManager,
  createSkinSwitcherWindowPlugin,
} from "../src/skins";
import {
  AudioMixerEngine,
  createAudioSettingsWindowPlugin,
} from "../src/audio";
import { DesktopWindowManager } from "../src/windows";
import {
  createEntityInspectorWindowPlugin,
  createLedgerStubPlugin,
} from "../src/views";

const UI_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(UI_ROOT, "../..");
const ESLINT_BIN = path.join(UI_ROOT, "node_modules", ".bin", "eslint");
const FIXTURE_PATH = path.resolve(__dirname, "fixtures/colony_m2_snapshot.json");
const M2_SNAPSHOTS = JSON.parse(fs.readFileSync(FIXTURE_PATH, "utf8")) as DiegeticSnapshot[];

function makeAudioManifest(skinId: string, cooldownMs: number): AudioManifestDto {
  return {
    schemaVersion: 1,
    skinId,
    ambientSurface: {
      fadeStartAltitude: 0.2,
      fadeEndAltitude: 0.72,
      windLoopUrl: `file:///skins/${skinId}/audio/wind_surface.wav`,
      rainLoopUrl: `file:///skins/${skinId}/audio/rain_surface.wav`,
      settlementLoopUrl: `file:///skins/${skinId}/audio/workshop_bustle.wav`,
    },
    ambientMapStudy: {
      fadeStartAltitude: 0.25,
      fadeEndAltitude: 0.78,
      studyLoopUrl: `file:///skins/${skinId}/audio/study_room_clock.wav`,
    },
    music: {
      crossfadeMs: 1000,
      stemUrls: [`file:///skins/${skinId}/audio/music_colony_calm.wav`],
    },
    uiFoley: {
      cooldownMs,
      cueUrls: {
        window_open: `file:///skins/${skinId}/audio/foley_open.wav`,
        window_close: `file:///skins/${skinId}/audio/foley_close.wav`,
        clock_step: `file:///skins/${skinId}/audio/foley_step.wav`,
      },
    },
  };
}

function loadRealSkinCss(skinId: string): string {
  return fs.readFileSync(path.join(REPO_ROOT, "skins", skinId, "theme.css"), "utf8");
}

function makeMockSkinBundle(skinId: string, displayName: string): SkinManifestBundle {
  return {
    skinId,
    displayName,
    stylesheetHref: `file://${path.join(REPO_ROOT, "skins", skinId, "theme.css")}`,
    cssText: loadRealSkinCss(skinId),
    globePalette:
      skinId === "industrial-brass"
        ? {
            oceanShallowRgb: [42, 73, 92],
            oceanDeepRgb: [18, 33, 43],
            landLowRgb: [78, 92, 82],
            landMidRgb: [114, 120, 106],
            landHighRgb: [141, 121, 86],
            peakSnowRgb: [214, 221, 227],
            riverWaterRgb: [69, 148, 184],
            roadOchreRgb: [228, 122, 48],
            settlementMarkerRgb: [212, 166, 68],
            contourInkRgb: [212, 166, 68],
            compassRoseRgb: [228, 122, 48],
            unexploredVellumRgb: [36, 44, 53],
            parchmentGrainTextureUrl: "",
            hatchPatternTextureUrl: "",
            terrainBiomeTextureUrl: "",
          }
        : {
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
    audioManifest: makeAudioManifest(skinId, 60),
    assetManifest: {
      schemaVersion: 1,
      fallbacks: {
        portraitUrl: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "fallback_silhouette_portrait.svg")}`,
        iconUrl: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "fallback_commodity.svg")}`,
        dispatchPlateUrl: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "fallback_dispatch_plate.svg")}`,
      },
      portraits: [
        {
          id: "steward_adult",
          url: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "portrait_steward_adult.png")}`,
          ageBracket: "adult",
          vocation: "agronomist",
          eraTag: "colony_early",
          healthBand: "healthy",
        },
        {
          id: "artificer_senior",
          url: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "portrait_artificer_senior.png")}`,
          ageBracket: "senior",
          vocation: "artificer",
          eraTag: "colony_early",
          healthBand: "weathered",
        },
      ],
      icons: [
        {
          key: "raw_grain",
          category: "commodity",
          url: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "icon_grain.png")}`,
        },
      ],
      dispatchPlates: [
        {
          key: "expedition_dispatch",
          url: `file://${path.join(REPO_ROOT, "skins", skinId, "assets", "dispatch_header_seal.png")}`,
        },
      ],
    },
  };
}

describe("M2 Command HUD, External Skins, Asset Resolver & Web Audio Mixer", () => {
  let dom: JSDOM;
  let backup: { window: unknown; document: unknown; HTMLElement: unknown };

  beforeEach(() => {
    dom = new JSDOM("<!doctype html><html><head></head><body><div id='app-shell'></div></body></html>");
    backup = {
      window: globalThis.window,
      document: globalThis.document,
      HTMLElement: globalThis.HTMLElement,
    };
    (globalThis as unknown as { window: unknown }).window = dom.window;
    (globalThis as unknown as { document: unknown }).document = dom.window.document;
    (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = dom.window.HTMLElement;
  });

  afterEach(() => {
    (globalThis as unknown as { window: unknown }).window = backup.window;
    (globalThis as unknown as { document: unknown }).document = backup.document;
    (globalThis as unknown as { HTMLElement: unknown }).HTMLElement = backup.HTMLElement;
  });

  // @req UI-HUD-001
  it("renders persistent HUD clock, HQ fly-to anchor, attention capacity, speed tiers, +1 Hour step, and auto-pause banner (UI-HUD-001)", () => {
    const store = new DiegeticStore();
    const bus = new UiIntentBus();
    const host = dom.window.document.getElementById("app-shell") as HTMLElement;

    const cleanup = mountCommandHud(host, store, bus);
    store.ingestSnapshot(M2_SNAPSHOTS[0]);

    const emitted: UiIntent[] = [];
    bus.subscribe((i) => emitted.push(i));

    expect(dom.window.document.getElementById("hud-scenario-title")?.textContent).toContain("The Unexplained Colony");
    expect(dom.window.document.getElementById("hud-clock-text")?.textContent).toContain("Year 1 Day 1 00:00");
    expect(dom.window.document.getElementById("hud-attention-badge")?.textContent).toContain("2.0/4.0");
    expect(dom.window.document.getElementById("hud-autopause-banner")?.hidden).toBe(true);

    // Click HQ link -> FlyGlobeToCell(0) + FocusEntityInspector
    dom.window.document.getElementById("hud-hq-link")?.click();
    expect(emitted.some((i) => i.kind === "FlyGlobeToCell" && i.cellId === 0)).toBe(true);
    expect(emitted.some((i) => i.kind === "FocusEntityInspector")).toBe(true);

    // Click Pause, +1 Hour, and 168x speed button
    dom.window.document.getElementById("hud-pause-btn")?.click();
    dom.window.document.getElementById("hud-step-btn")?.click();
    dom.window.document.querySelector<HTMLButtonElement>("[data-speed-tier='168']")?.click();

    const dispatched: ValidatedPlayerCommand[] = emitted
      .filter((i): i is Extract<UiIntent, { kind: "DispatchSimCommand" }> => i.kind === "DispatchSimCommand")
      .map((i) => i.command);
    expect(dispatched).toEqual([
      { kind: "setPaused", paused: true },
      { kind: "stepSingleTick" },
      { kind: "setSpeedMultiplier", multiplier: 168 },
    ]);

    // Ingest tick 12 auto-paused snapshot -> reveals clear auto-pause reason banner
    store.ingestSnapshot(M2_SNAPSHOTS[1]);
    const banner = dom.window.document.getElementById("hud-autopause-banner")!;
    expect(banner.hidden).toBe(false);
    expect(banner.textContent).toContain("Severe Coastal Gale Over Perimeter");
    expect(dom.window.document.getElementById("hud-pause-btn")?.getAttribute("aria-pressed")).toBe("true");
    cleanup();
  });

  // @req UI-SKN-001
  // @req UI-AUD-001
  it("enforces zero hardcoded hex colors, rgb(), or audio extensions anywhere in code/ui/src/ (UI-SKN-001, UI-AUD-001)", () => {
    const srcDir = path.join(UI_ROOT, "src");
    const tsFiles: string[] = [];
    const walk = (dir: string) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, entry.name);
        if (entry.isDirectory()) walk(p);
        else if (entry.isFile() && p.endsWith(".ts")) tsFiles.push(p);
      }
    };
    walk(srcDir);
    expect(tsFiles.length).toBeGreaterThan(10);

    for (const file of tsFiles) {
      const content = fs.readFileSync(file, "utf8");
      expect(content).not.toMatch(/#[0-9a-fA-F]{6}\b/);
      expect(content).not.toMatch(/\b(?:rgb|rgba|hsl|hsla)\s*\(/i);
      expect(content).not.toMatch(/\.(wav|ogg|mp3|flac)\b/i);
    }

    // ESLint explicitly rejects hardcoded hex color & audio filenames in renderer source
    let hexRejected = false;
    try {
      execFileSync(
        ESLINT_BIN,
        ["--stdin", "--stdin-filename", path.join(srcDir, "hud", "_tmp_color_check.ts")],
        { cwd: UI_ROOT, input: `export const BAD = "#ff00ff";\n`, encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] },
      );
    } catch {
      hexRejected = true;
    }
    expect(hexRejected).toBe(true);
  });

  // @req UI-SKN-002
  // @req UI-SKN-003
  it("defines distortion-free 9-slice frame CSS rules and switches external skins live without disrupting windows or simulation (UI-SKN-002, UI-SKN-003)", async () => {
    const parchment = makeMockSkinBundle("cartographer-parchment", "Cartographer's Parchment");
    const brass = makeMockSkinBundle("industrial-brass", "Industrial Brass & Slate");
    const player = new MockSnapshotPlayer(M2_SNAPSHOTS, [parchment, brass]);
    const store = new DiegeticStore();
    store.ingestSnapshot(M2_SNAPSHOTS[0]);
    const bus = new UiIntentBus();

    const skinManager = new SkinManager(player, bus);
    await skinManager.initialize("cartographer-parchment");

    const registry = new PluginRegistry();
    registry.registerWindowPlugin(createLedgerStubPlugin());
    registry.registerWindowPlugin(createSkinSwitcherWindowPlugin(skinManager));
    registry.registerWindowPlugin(createEntityInspectorWindowPlugin());

    const host = dom.window.document.getElementById("app-shell") as HTMLElement;
    const manager = new DesktopWindowManager(host, registry, {
      store,
      bus,
      assetResolver: skinManager.assetResolver,
    });

    const ledgerWin = manager.openWindow("physical_ledger", { x: 40, y: 80, width: 420, height: 260 })!;
    const switcherWin = manager.openWindow("skin_appearance", { x: 520, y: 80, width: 340, height: 220 })!;

    // Verify 9-slice border-image CSS tokens exist in both skins (`UI-SKN-002`)
    for (const css of [parchment.cssText, brass.cssText]) {
      expect(css).toContain("--skin-frame-image:");
      expect(css).toContain("--skin-frame-slice: 16;");
      expect(css).toContain("border-image-slice: var(--skin-frame-slice) fill;");
      expect(css).toContain("border-image-repeat: round;");
    }

    // Preserve filter state before live switch (`UI-SKN-003`)
    const filterInput = dom.window.document.querySelector<HTMLInputElement>(`#${ledgerWin} [data-ledger-filter='true']`)!;
    filterInput.value = "grain";

    await skinManager.switchSkin("industrial-brass");
    expect(dom.window.document.documentElement.getAttribute("data-active-skin")).toBe("industrial-brass");
    expect(dom.window.document.getElementById("active-skin-inline-css")?.textContent).toContain("#222830");
    expect(manager.listFrames()).toHaveLength(2);
    expect(manager.getFrameSnapshot(ledgerWin)?.x).toBe(40);
    expect(manager.getFrameSnapshot(switcherWin)?.x).toBe(520);
    expect(filterInput.value).toBe("grain");
    expect(store.getSnapshot()?.clockAndNotifications.tick).toBe(0);

    // Hot-reload signal re-applies updated skin stylesheet cleanly
    player.emitHotReload("industrial-brass");
    expect(skinManager.getActiveBundle()?.skinId).toBe("industrial-brass");

    manager.dispose();
    skinManager.dispose();
  });

  // @req UI-SKN-005
  it("resolves AI portraits by tags, commodity icons, dispatch plates, and safe CC0 fallbacks on missing keys (UI-SKN-005)", () => {
    const resolver = new AssetResolver();
    const parchment = makeMockSkinBundle("cartographer-parchment", "Cartographer's Parchment");
    resolver.updateFromSkin("cartographer-parchment", parchment.assetManifest, ["cartographer-parchment"]);

    const stewardUrl = resolver.resolvePortrait({ characterId: 1, vocation: "agronomist", ageBracket: "adult" });
    expect(stewardUrl).toContain("portrait_steward_adult.png");

    const artificerUrl = resolver.resolvePortrait({ characterId: 2, vocation: "artificer", ageBracket: "senior" });
    expect(artificerUrl).toContain("portrait_artificer_senior.png");

    const unknownPortraitFallback = resolver.resolvePortrait({ characterId: 99, vocation: "astronaut", ageBracket: "ancient" });
    expect(unknownPortraitFallback).toContain("fallback_silhouette_portrait.svg");

    expect(resolver.resolveIcon("raw_grain")).toContain("icon_grain.png");
    expect(resolver.resolveIcon("missing_exotic_ore")).toContain("fallback_commodity.svg");
    expect(resolver.resolveDispatchPlate("expedition_dispatch")).toContain("dispatch_header_seal.png");
    expect(resolver.resolveDispatchPlate("unknown_plate")).toContain("fallback_dispatch_plate.svg");
  });

  // @req UI-AUD-002
  // @req UI-AUD-004
  it("cross-fades surface/orbital ambience without clipping or leaking unexplored weather, and syncs audio mixer sliders & persistence (UI-AUD-002, UI-AUD-004)", async () => {
    const parchment = makeMockSkinBundle("cartographer-parchment", "Cartographer's Parchment");
    const player = new MockSnapshotPlayer(M2_SNAPSHOTS, [parchment]);
    const store = new DiegeticStore();
    store.ingestSnapshot(M2_SNAPSHOTS[0]);
    const bus = new UiIntentBus();

    const engine = new AudioMixerEngine(bus, { store, bridge: player });
    const skinManager = new SkinManager(player, bus);
    await skinManager.initialize("cartographer-parchment");

    // Step camera altitude monotonically from surface (0.0) over surveyed cell 0 (precip=14.5mm/hr) to orbital (1.0)
    const surfaceGains: number[] = [];
    const orbitalGains: number[] = [];
    for (let i = 0; i <= 10; i += 1) {
      const alt = i / 10;
      engine.setAltitudeAndCell(alt, 0);
      const mix = engine.getEffectiveGains();
      expect(mix.surfaceEnv).toBeGreaterThanOrEqual(0);
      expect(mix.surfaceEnv).toBeLessThanOrEqual(1);
      expect(mix.orbitalEnv).toBeGreaterThanOrEqual(0);
      expect(mix.orbitalEnv).toBeLessThanOrEqual(1);
      expect(mix.surfaceEnv + mix.orbitalEnv).toBeLessThanOrEqual(1.0001);
      surfaceGains.push(mix.surfaceEnv);
      orbitalGains.push(mix.orbitalEnv);
    }
    expect(surfaceGains[0]).toBeGreaterThan(0.5);
    expect(surfaceGains[10]).toBe(0);
    expect(orbitalGains[0]).toBe(0);
    expect(orbitalGains[10]).toBeGreaterThan(0.5);

    // At surface altitude over surveyed cell 0 (rain + settlement) -> rain/settlement gains active
    engine.setAltitudeAndCell(0.05, 0);
    const wetMix = engine.getEffectiveGains();
    expect(wetMix.rainGain).toBeGreaterThan(0.2);
    expect(wetMix.settlementGain).toBeGreaterThan(0.2);

    // Epistemic air-gap (`UI-AUD-002`): panning over Unexplored cell 5 MUST silence rain & settlement cues
    engine.setAltitudeAndCell(0.05, 5);
    const unexploredMix = engine.getEffectiveGains();
    expect(unexploredMix.rainGain).toBe(0);
    expect(unexploredMix.settlementGain).toBe(0);

    // Audio mixer UI window syncs `--control-value`, mutes channels cleanly, and persists via bridge (`UI-AUD-004`)
    const registry = new PluginRegistry();
    registry.registerWindowPlugin(createAudioSettingsWindowPlugin(engine));
    const host = dom.window.document.getElementById("app-shell") as HTMLElement;
    const manager = new DesktopWindowManager(host, registry, { store, bus });
    const winId = manager.openWindow("audio_mixer")!;

    const masterSlider = dom.window.document.querySelector<HTMLInputElement>(`#${winId} #audio-slider-master`)!;
    expect(masterSlider.style.getPropertyValue("--control-value")).toBe("85");

    masterSlider.value = "40";
    masterSlider.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    expect(masterSlider.style.getPropertyValue("--control-value")).toBe("40");
    expect(engine.getEffectiveGains().master).toBeCloseTo(0.4, 3);

    const musicMuteBtn = dom.window.document.querySelector<HTMLButtonElement>(`#${winId} [data-audio-mute='music']`)!;
    musicMuteBtn.click();
    expect(musicMuteBtn.getAttribute("aria-pressed")).toBe("true");
    expect(engine.getEffectiveGains().music).toBe(0);

    // Flush async persistence microtask
    await new Promise((r) => setTimeout(r, 15));
    const savedPrefs = await player.readUserPreferences();
    expect(savedPrefs.audioMixer.masterVolume).toBeCloseTo(0.4, 3);
    expect(savedPrefs.audioMixer.musicMuted).toBe(true);

    manager.dispose();
    engine.dispose();
    skinManager.dispose();
  });
});
