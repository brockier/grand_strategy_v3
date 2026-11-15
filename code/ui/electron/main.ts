import { app, BrowserWindow, ipcMain, session, type WebContents } from "electron";
import { execFileSync, spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { pathToFileURL } from "node:url";
import { parsePlayerCommand } from "./ipc-validator";
import { runM2E2ESmokePhase } from "./m2-e2e-smoke";
import { runM3M4E2ESmokePhase } from "./m3-m4-e2e-smoke";
import { runM5M6M7E2ESmoke } from "./m5-m6-m7-e2e-smoke";

// Enable Chromium Dawn WebGPU with Vulkan + WebGL2 fallback across Linux X11, Wayland & Windows.
app.commandLine.appendSwitch("enable-unsafe-webgpu");
app.commandLine.appendSwitch("enable-features", "Vulkan");
app.commandLine.appendSwitch("enable-webgl");
app.commandLine.appendSwitch("ignore-gpu-blocklist");
if (process.env.NG_SOFTWARE_GL === "1") {
  app.commandLine.appendSwitch("use-gl", "angle");
  app.commandLine.appendSwitch("use-angle", "swiftshader-webgl");
  app.commandLine.appendSwitch("enable-unsafe-swiftshader");
}

function getArgValue(prefix: string): string | null {
  for (const arg of process.argv) {
    if (arg.startsWith(prefix)) {
      return arg.slice(prefix.length);
    }
  }
  return null;
}

const SMOKE_MODE = getArgValue("--smoke-mode=");
const INITIAL_CORRUPT_PACK = getArgValue("--corrupt-pack=");

interface RuntimePaths {
  readonly indexHtml: string;
  readonly preloadJs: string;
  readonly modsDir: string;
  readonly skinsDir: string;
  readonly simCliBin: string;
}

function resolveRuntimePaths(): RuntimePaths {
  const isPackedAsar = __dirname.includes("app.asar") || app.isPackaged;
  if (isPackedAsar) {
    const unpackedBase = path.dirname(__dirname).replace("app.asar", "app.asar.unpacked");
    return {
      indexHtml: path.join(__dirname, "../dist/index.html"),
      preloadJs: path.join(__dirname, "preload.js"),
      modsDir: path.join(process.resourcesPath, "mods"),
      skinsDir: path.join(process.resourcesPath, "skins"),
      simCliBin:
        process.env.NG_SIM_CLI_PATH ??
        path.join(unpackedBase, "native", "bin", process.platform === "win32" ? "sim-cli.exe" : "sim-cli"),
    };
  }
  return {
    indexHtml: path.join(__dirname, "../dist/index.html"),
    preloadJs: path.join(__dirname, "preload.js"),
    modsDir: path.resolve(__dirname, "../../../mods"),
    skinsDir: path.resolve(__dirname, "../../../skins"),
    simCliBin:
      process.env.NG_SIM_CLI_PATH ??
      path.resolve(__dirname, "../../engine/target/debug", process.platform === "win32" ? "sim-cli.exe" : "sim-cli"),
  };
}

interface ServeStdioResponse {
  readonly ok: boolean;
  readonly tick?: number;
  readonly snapshot?: Record<string, unknown>;
  readonly errorCode?: string;
  readonly message?: string;
}

class SimStdioSession {
  private proc: ChildProcessWithoutNullStreams;
  private rl: readline.Interface;
  private waiters: Array<{
    resolve: (resp: ServeStdioResponse) => void;
    reject: (err: Error) => void;
  }> = [];
  private requestChain: Promise<unknown> = Promise.resolve();
  private closed = false;
  private stderrTail = "";

  constructor(simCliBin: string, packDirs: string[], seed = 42) {
    const args = ["serve-stdio"];
    for (const p of packDirs) {
      args.push("--pack", p);
    }
    args.push("--seed", String(seed));
    this.proc = spawn(simCliBin, args, { stdio: ["pipe", "pipe", "pipe"] });
    this.proc.stderr.setEncoding("utf8");
    this.proc.stderr.on("data", (chunk: string) => {
      this.stderrTail = (this.stderrTail + chunk).slice(-2048);
    });
    this.rl = readline.createInterface({ input: this.proc.stdout });
    this.rl.on("line", (line) => {
      const trimmed = line.trim();
      if (!trimmed) return;
      const waiter = this.waiters.shift();
      if (!waiter) return;
      try {
        const parsed = JSON.parse(trimmed) as ServeStdioResponse;
        waiter.resolve(parsed);
      } catch (err) {
        waiter.reject(new Error(`Malformed sim-cli stdio JSON: ${String(err)} (${trimmed})`));
      }
    });
    this.proc.on("exit", (code, signal) => {
      this.closed = true;
      while (this.waiters.length > 0) {
        const waiter = this.waiters.shift();
        waiter?.reject(
          new Error(
            `sim-cli serve-stdio exited prematurely (code=${String(code)}, signal=${String(signal)}): ${this.stderrTail}`,
          ),
        );
      }
    });
  }

  request(payload: unknown): Promise<ServeStdioResponse> {
    const exec = async (): Promise<ServeStdioResponse> => {
      if (this.closed) {
        throw new Error("sim-cli serve-stdio session is closed");
      }
      return new Promise<ServeStdioResponse>((resolve, reject) => {
        this.waiters.push({ resolve, reject });
        this.proc.stdin.write(`${JSON.stringify(payload)}\n`, "utf8", (err) => {
          if (err) {
            const idx = this.waiters.findIndex((w) => w.reject === reject);
            if (idx >= 0) this.waiters.splice(idx, 1);
            reject(err);
          }
        });
      });
    };
    const next = this.requestChain.then(exec, exec);
    this.requestChain = next.catch(() => undefined);
    return next;
  }

  async dispose(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    try {
      this.proc.stdin.write(`${JSON.stringify({ op: "quit" })}\n`, "utf8");
      this.proc.stdin.end();
    } catch {
      // Ignore write failures during shutdown
    }
    this.rl.close();
    setTimeout(() => {
      if (!this.proc.killed) {
        this.proc.kill();
      }
    }, 150).unref();
  }
}

let activeExtraPacks: string[] = INITIAL_CORRUPT_PACK ? [INITIAL_CORRUPT_PACK] : [];
let currentTick = 0;
let isPaused = true;
let speedMultiplier = 1;
let cachedSnapshot: Record<string, unknown> | null = null;
let activeSession: SimStdioSession | null = null;
let clockInterval: NodeJS.Timeout | null = null;
let clockStepInFlight = false;
let mainWebContents: WebContents | null = null;

type RgbTuple = [number, number, number];

interface ParsedTomlDoc {
  scalars: Record<string, unknown>;
  tables: Record<string, Record<string, unknown>>;
  arraysOfTables: Record<string, Record<string, unknown>[]>;
}

function unquoteTomlValue(raw: string): unknown {
  const s = raw.trim();
  if (s.startsWith('"') && s.endsWith('"')) {
    return s.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }
  if (s === "true") return true;
  if (s === "false") return false;
  if (s.startsWith("[") && s.endsWith("]")) {
    const inner = s.slice(1, -1).trim();
    if (!inner) return [];
    return inner.split(",").map((part) => unquoteTomlValue(part));
  }
  const num = Number(s);
  if (!Number.isNaN(num) && s.length > 0) return num;
  return s;
}

function stripTomlComment(line: string): string {
  let inQuotes = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === '"' && (i === 0 || line[i - 1] !== "\\")) {
      inQuotes = !inQuotes;
    } else if (ch === "#" && !inQuotes) {
      return line.slice(0, i);
    }
  }
  return line;
}

function parseSimpleToml(text: string): ParsedTomlDoc {
  const doc: ParsedTomlDoc = { scalars: {}, tables: {}, arraysOfTables: {} };
  let target: Record<string, unknown> = doc.scalars;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = stripTomlComment(rawLine).trim();
    if (!line) continue;
    if (line.startsWith("[[") && line.endsWith("]]")) {
      const name = line.slice(2, -2).trim();
      if (!doc.arraysOfTables[name]) doc.arraysOfTables[name] = [];
      const row: Record<string, unknown> = {};
      doc.arraysOfTables[name].push(row);
      target = row;
      continue;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      const name = line.slice(1, -1).trim();
      if (!doc.tables[name]) doc.tables[name] = {};
      target = doc.tables[name];
      continue;
    }
    const eq = line.indexOf("=");
    if (eq > 0) {
      const key = line.slice(0, eq).trim();
      const val = line.slice(eq + 1);
      target[key] = unquoteTomlValue(val);
    }
  }
  return doc;
}

function hexToRgbTuple(hex: unknown, fallback: RgbTuple): RgbTuple {
  if (typeof hex !== "string") return fallback;
  const cleaned = hex.trim().replace(/^#/, "");
  if (cleaned.length !== 6) return fallback;
  const r = Number.parseInt(cleaned.slice(0, 2), 16);
  const g = Number.parseInt(cleaned.slice(2, 4), 16);
  const b = Number.parseInt(cleaned.slice(4, 6), 16);
  if ([r, g, b].some((v) => Number.isNaN(v))) return fallback;
  return [r, g, b];
}

function resolveSkinFileUrl(skinDir: string, rel: unknown): string {
  if (typeof rel !== "string" || rel.trim().length === 0) return "";
  const abs = path.resolve(skinDir, rel);
  return pathToFileURL(abs).href;
}

function discoverSkinDescriptors(skinsDir: string): Array<{ skinId: string; displayName: string; stylesheetPath: string }> {
  if (!fs.existsSync(skinsDir)) return [];
  const entries = fs.readdirSync(skinsDir, { withFileTypes: true }).filter((d) => d.isDirectory());
  const descriptors: Array<{ skinId: string; displayName: string; stylesheetPath: string }> = [];
  for (const dir of entries) {
    const dirPath = path.join(skinsDir, dir.name);
    const manifestPath = path.join(dirPath, "skin_manifest.toml");
    const themeCss = path.join(dirPath, "theme.css");
    if (!fs.existsSync(manifestPath) || !fs.existsSync(themeCss)) continue;
    const parsed = parseSimpleToml(fs.readFileSync(manifestPath, "utf8"));
    const skinId = typeof parsed.scalars.skin_id === "string" ? parsed.scalars.skin_id : dir.name;
    const displayName = typeof parsed.scalars.display_name === "string" ? parsed.scalars.display_name : skinId;
    const stylesheetRel = typeof parsed.scalars.stylesheet === "string" ? parsed.scalars.stylesheet : "theme.css";
    descriptors.push({
      skinId,
      displayName,
      stylesheetPath: path.join(dirPath, stylesheetRel),
    });
  }
  descriptors.sort((a, b) => {
    if (a.skinId === "cartographer-parchment") return -1;
    if (b.skinId === "cartographer-parchment") return 1;
    return a.skinId.localeCompare(b.skinId);
  });
  return descriptors;
}

function readSkinManifestBundle(skinsDir: string, skinId: string) {
  const safeId = path.basename(skinId);
  const skinDir = path.join(skinsDir, safeId);
  const manifestPath = path.join(skinDir, "skin_manifest.toml");
  const parsedManifest = fs.existsSync(manifestPath)
    ? parseSimpleToml(fs.readFileSync(manifestPath, "utf8"))
    : { scalars: {}, tables: {}, arraysOfTables: {} };

  const displayName =
    typeof parsedManifest.scalars.display_name === "string"
      ? parsedManifest.scalars.display_name
      : safeId;
  const stylesheetRel =
    typeof parsedManifest.scalars.stylesheet === "string"
      ? parsedManifest.scalars.stylesheet
      : "theme.css";
  const cssPath = path.join(skinDir, stylesheetRel);
  const cssText = fs.existsSync(cssPath) ? fs.readFileSync(cssPath, "utf8") : "";

  const gp = parsedManifest.tables.globe_palette ?? {};
  const globePalette = {
    oceanShallowRgb: hexToRgbTuple(gp.ocean_shallow_hex, [135, 163, 173]),
    oceanDeepRgb: hexToRgbTuple(gp.ocean_deep_hex, [61, 89, 102]),
    landLowRgb: hexToRgbTuple(gp.land_low_hex, [115, 138, 87]),
    landMidRgb: hexToRgbTuple(gp.land_mid_hex, [176, 162, 116]),
    landHighRgb: hexToRgbTuple(gp.land_high_hex, [140, 114, 88]),
    peakSnowRgb: hexToRgbTuple(gp.peak_snow_hex, [244, 239, 228]),
    riverWaterRgb: hexToRgbTuple(gp.river_water_hex, [54, 108, 133]),
    roadOchreRgb: hexToRgbTuple(gp.road_ochre_hex, [166, 89, 40]),
    settlementMarkerRgb: hexToRgbTuple(gp.settlement_marker_hex, [139, 46, 34]),
    contourInkRgb: hexToRgbTuple(gp.contour_ink_hex, [59, 44, 29]),
    compassRoseRgb: hexToRgbTuple(gp.compass_rose_hex, [110, 85, 57]),
    unexploredVellumRgb: hexToRgbTuple(gp.unexplored_vellum_hex, [230, 216, 185]),
    parchmentGrainTextureUrl: resolveSkinFileUrl(skinDir, gp.parchment_grain_texture),
    hatchPatternTextureUrl: resolveSkinFileUrl(skinDir, gp.hatch_pattern_texture),
    terrainBiomeTextureUrl: resolveSkinFileUrl(skinDir, gp.terrain_biome_texture),
  };

  const audioRel =
    typeof parsedManifest.scalars.audio_manifest === "string"
      ? parsedManifest.scalars.audio_manifest
      : "audio/audio_manifest.toml";
  const audioPath = path.join(skinDir, audioRel);
  let audioManifest: Record<string, unknown> | undefined;
  if (fs.existsSync(audioPath)) {
    const a = parseSimpleToml(fs.readFileSync(audioPath, "utf8"));
    const surf = a.tables["channels.ambient_surface"] ?? {};
    const study = a.tables["channels.ambient_map_study"] ?? {};
    const music = a.tables["channels.music"] ?? {};
    const foley = a.tables["channels.ui_foley"] ?? {};
    const stemsRaw = Array.isArray(music.stems) ? music.stems : [];
    const cueUrls: Record<string, string> = {};
    for (const [k, v] of Object.entries(foley)) {
      if (k !== "cooldown_ms" && typeof v === "string") {
        cueUrls[k] = resolveSkinFileUrl(skinDir, v);
      }
    }
    audioManifest = {
      schemaVersion: typeof a.scalars.schema_version === "number" ? a.scalars.schema_version : 1,
      skinId: typeof a.scalars.skin_id === "string" ? a.scalars.skin_id : safeId,
      ambientSurface: {
        fadeStartAltitude: typeof surf.fade_start_altitude === "number" ? surf.fade_start_altitude : 0.2,
        fadeEndAltitude: typeof surf.fade_end_altitude === "number" ? surf.fade_end_altitude : 0.72,
        windLoopUrl: resolveSkinFileUrl(skinDir, surf.wind_loop),
        rainLoopUrl: resolveSkinFileUrl(skinDir, surf.rain_loop),
        settlementLoopUrl: resolveSkinFileUrl(skinDir, surf.settlement_loop),
      },
      ambientMapStudy: {
        fadeStartAltitude: typeof study.fade_start_altitude === "number" ? study.fade_start_altitude : 0.25,
        fadeEndAltitude: typeof study.fade_end_altitude === "number" ? study.fade_end_altitude : 0.78,
        studyLoopUrl: resolveSkinFileUrl(skinDir, study.study_loop),
      },
      music: {
        crossfadeMs: typeof music.crossfade_ms === "number" ? music.crossfade_ms : 1000,
        stemUrls: stemsRaw.filter((u): u is string => typeof u === "string").map((u) => resolveSkinFileUrl(skinDir, u)),
      },
      uiFoley: {
        cooldownMs: typeof foley.cooldown_ms === "number" ? foley.cooldown_ms : 60,
        cueUrls,
      },
    };
  }

  const assetRel =
    typeof parsedManifest.scalars.asset_manifest === "string"
      ? parsedManifest.scalars.asset_manifest
      : "assets/asset_manifest.toml";
  const assetPath = path.join(skinDir, assetRel);
  let assetManifest: Record<string, unknown> | undefined;
  if (fs.existsSync(assetPath)) {
    const m = parseSimpleToml(fs.readFileSync(assetPath, "utf8"));
    const fb = m.tables.fallbacks ?? {};
    const portraits = (m.arraysOfTables.portraits ?? []).map((p) => ({
      id: String(p.id ?? ""),
      url: resolveSkinFileUrl(skinDir, p.rel_path),
      ageBracket: String(p.age_bracket ?? "adult"),
      vocation: String(p.vocation ?? "general"),
      eraTag: String(p.era_tag ?? "colony_early"),
      healthBand: String(p.health_band ?? "healthy"),
    }));
    const icons = (m.arraysOfTables.icons ?? []).map((ic) => ({
      key: String(ic.key ?? ""),
      category: String(ic.category ?? "commodity"),
      url: resolveSkinFileUrl(skinDir, ic.rel_path),
    }));
    const dispatchPlates = (m.arraysOfTables.dispatch_plates ?? []).map((dp) => ({
      key: String(dp.key ?? ""),
      url: resolveSkinFileUrl(skinDir, dp.rel_path),
    }));
    assetManifest = {
      schemaVersion: typeof m.scalars.schema_version === "number" ? m.scalars.schema_version : 1,
      fallbacks: {
        portraitUrl: resolveSkinFileUrl(skinDir, fb.portrait),
        iconUrl: resolveSkinFileUrl(skinDir, fb.icon),
        dispatchPlateUrl: resolveSkinFileUrl(skinDir, fb.dispatch_plate),
      },
      portraits,
      icons,
      dispatchPlates,
    };
  }

  return {
    skinId: safeId,
    displayName,
    stylesheetHref: fs.existsSync(cssPath) ? pathToFileURL(cssPath).href : "",
    cssText,
    globePalette,
    ...(audioManifest ? { audioManifest } : {}),
    ...(assetManifest ? { assetManifest } : {}),
  };
}

function resolveUserPrefsPath(): string {
  if (process.env.NG_USER_PREFS_PATH && process.env.NG_USER_PREFS_PATH.trim().length > 0) {
    return process.env.NG_USER_PREFS_PATH;
  }
  return path.join(app.getPath("userData"), "user_preferences.json");
}

function defaultUserPreferences() {
  return {
    activeSkinId: "cartographer-parchment",
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
}

function readUserPreferences() {
  const prefsPath = resolveUserPrefsPath();
  const defaults = defaultUserPreferences();
  try {
    if (!fs.existsSync(prefsPath)) return defaults;
    const raw = JSON.parse(fs.readFileSync(prefsPath, "utf8")) as Record<string, unknown>;
    const mixerRaw = (typeof raw.audioMixer === "object" && raw.audioMixer !== null ? raw.audioMixer : {}) as Record<
      string,
      unknown
    >;
    const clamp01 = (v: unknown, fb: number) =>
      typeof v === "number" && Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : fb;
    const boolOr = (v: unknown, fb: boolean) => (typeof v === "boolean" ? v : fb);
    const presetsRaw =
      typeof raw.namedLayoutPresets === "object" && raw.namedLayoutPresets !== null
        ? (raw.namedLayoutPresets as Record<string, unknown>)
        : {};
    const namedLayoutPresets: Record<string, string> = {};
    for (const [k, v] of Object.entries(presetsRaw)) {
      if (typeof v === "string") namedLayoutPresets[k] = v;
    }
    const rulesRaw =
      typeof raw.notificationRules === "object" && raw.notificationRules !== null
        ? (raw.notificationRules as Record<string, unknown>)
        : {};
    const notificationRules: Record<string, string> = {};
    for (const [k, v] of Object.entries(rulesRaw)) {
      if (typeof v === "string") notificationRules[k] = v;
    }
    return {
      activeSkinId: typeof raw.activeSkinId === "string" ? raw.activeSkinId : defaults.activeSkinId,
      audioMixer: {
        masterVolume: clamp01(mixerRaw.masterVolume, defaults.audioMixer.masterVolume),
        musicVolume: clamp01(mixerRaw.musicVolume, defaults.audioMixer.musicVolume),
        ambienceVolume: clamp01(mixerRaw.ambienceVolume, defaults.audioMixer.ambienceVolume),
        foleyVolume: clamp01(mixerRaw.foleyVolume, defaults.audioMixer.foleyVolume),
        masterMuted: boolOr(mixerRaw.masterMuted, false),
        musicMuted: boolOr(mixerRaw.musicMuted, false),
        ambienceMuted: boolOr(mixerRaw.ambienceMuted, false),
        foleyMuted: boolOr(mixerRaw.foleyMuted, false),
      },
      ...(typeof raw.activeLayoutJson === "string" ? { activeLayoutJson: raw.activeLayoutJson } : {}),
      namedLayoutPresets,
      notificationRules,
    };
  } catch {
    return defaults;
  }
}

function writeUserPreferences(raw: unknown): void {
  const prefsPath = resolveUserPrefsPath();
  const dir = path.dirname(prefsPath);
  fs.mkdirSync(dir, { recursive: true });
  const incoming = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const existing = readUserPreferences();
  const merged = {
    ...existing,
    ...(typeof incoming.activeSkinId === "string" ? { activeSkinId: path.basename(incoming.activeSkinId) } : {}),
    ...(typeof incoming.activeLayoutJson === "string" ? { activeLayoutJson: incoming.activeLayoutJson } : {}),
    audioMixer: {
      ...existing.audioMixer,
      ...(typeof incoming.audioMixer === "object" && incoming.audioMixer !== null
        ? (incoming.audioMixer as Record<string, unknown>)
        : {}),
    },
    namedLayoutPresets: {
      ...existing.namedLayoutPresets,
      ...(typeof incoming.namedLayoutPresets === "object" && incoming.namedLayoutPresets !== null
        ? (incoming.namedLayoutPresets as Record<string, string>)
        : {}),
    },
    notificationRules: {
      ...existing.notificationRules,
      ...(typeof incoming.notificationRules === "object" && incoming.notificationRules !== null
        ? (incoming.notificationRules as Record<string, string>)
        : {}),
    },
  };
  fs.writeFileSync(prefsPath, JSON.stringify(merged, null, 2), "utf8");
}

function activePackPaths(paths: RuntimePaths): string[] {
  return [
    path.join(paths.modsDir, "core-schemas"),
    path.join(paths.modsDir, "baseline-era"),
    path.join(paths.modsDir, "unexplained-colony"),
    ...activeExtraPacks,
  ];
}

function runSimCli(simCliBin: string, subcommand: string, packDirs: string[], extraArgs: string[] = []): { ok: boolean; stdout: string } {
  const args = [subcommand];
  for (const p of packDirs) {
    args.push("--pack", p);
  }
  args.push(...extraArgs);
  try {
    const out = execFileSync(simCliBin, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    return { ok: true, stdout: out };
  } catch (err) {
    const execErr = err as { stdout?: Buffer | string; stderr?: Buffer | string };
    const text = typeof execErr.stdout === "string" && execErr.stdout.trim().length > 0
      ? execErr.stdout
      : Buffer.isBuffer(execErr.stdout) && execErr.stdout.length > 0
        ? execErr.stdout.toString("utf8")
        : String(execErr.stderr ?? err);
    return { ok: false, stdout: text };
  }
}

let lastAutoPauseReason: string | null = null;
const dismissedAutoPauseReasons = new Set<string>();

async function disposeActiveSession(): Promise<void> {
  if (clockInterval) {
    clearInterval(clockInterval);
    clockInterval = null;
  }
  clockStepInFlight = false;
  if (activeSession) {
    const old = activeSession;
    activeSession = null;
    await old.dispose();
  }
}

function normalizeSnapshot(snapshot: Record<string, unknown>): Record<string, unknown> {
  const persistedPrefs = readUserPreferences();
  const persistedEntries = Object.entries(persistedPrefs.notificationRules);
  if (persistedEntries.length > 0) {
    const existingRules = Array.isArray(snapshot.notificationRules)
      ? (snapshot.notificationRules as Array<Record<string, unknown>>)
      : [];
    for (const [eventClassKey, behavior] of persistedEntries) {
      const found = existingRules.find((r) => r.eventClassKey === eventClassKey);
      if (found) {
        found.behavior = behavior;
      } else {
        existingRules.push({ eventClassKey, displayName: eventClassKey, behavior });
      }
    }
    snapshot.notificationRules = existingRules;
  }

  const succession = snapshot.succession as Record<string, unknown> | null | undefined;
  const pending = succession?.pendingTransition as Record<string, unknown> | null | undefined;
  const awaitingSuccession = Boolean(pending && pending.awaitingPlayerAcknowledgement === true);

  const clk = snapshot.clockAndNotifications as Record<string, unknown> | undefined;
  if (clk && typeof clk === "object") {
    if (typeof clk.tick === "number") {
      currentTick = clk.tick;
    }
    const reason =
      typeof clk.autoPauseReason === "string" && clk.autoPauseReason.length > 0
        ? clk.autoPauseReason
        : awaitingSuccession
          ? "succession_transition"
          : null;
    if (reason && !dismissedAutoPauseReasons.has(reason)) {
      if (reason !== lastAutoPauseReason || awaitingSuccession) {
        isPaused = true;
        lastAutoPauseReason = reason;
      }
      clk.autoPauseReason = reason;
      clk.paused = isPaused;
    } else if (reason && dismissedAutoPauseReasons.has(reason) && !awaitingSuccession) {
      clk.autoPauseReason = null;
      clk.paused = isPaused;
    } else {
      clk.paused = isPaused;
    }
    clk.speedMultiplier = speedMultiplier;
  }
  cachedSnapshot = snapshot;
  return snapshot;
}

function ensureLiveClockRunning(): void {
  if (clockInterval) return;
  clockInterval = setInterval(() => {
    if (isPaused || clockStepInFlight || !activeSession) return;
    if (!mainWebContents || mainWebContents.isDestroyed()) return;
    clockStepInFlight = true;
    const ticks = Math.max(1, Math.min(24, speedMultiplier));
    void activeSession
      .request({ op: "step", ticks })
      .then((resp) => {
        if (resp.ok && resp.snapshot && mainWebContents && !mainWebContents.isDestroyed()) {
          const normalized = normalizeSnapshot(resp.snapshot);
          mainWebContents.send("sim:snapshot", normalized);
        }
      })
      .catch(() => {
        // Ignore transient errors on shutdown
      })
      .finally(() => {
        clockStepInFlight = false;
      });
  }, 150);
  clockInterval.unref();
}

async function startOrGetSessionState(paths: RuntimePaths): Promise<unknown> {
  const packs = activePackPaths(paths);
  const valRes = runSimCli(paths.simCliBin, "validate-mods", packs);
  if (!valRes.ok) {
    try {
      const parsed = JSON.parse(valRes.stdout) as unknown;
      return { state: "mod_error", failure: parsed };
    } catch {
      return {
        state: "mod_error",
        failure: {
          ok: false,
          diagnostics: [
            {
              severity: "Error",
              layer: "EngineHost",
              filePath: packs.join(", "),
              line: 1,
              column: 1,
              fieldPath: "startup",
              message: valRes.stdout,
            },
          ],
        },
      };
    }
  }

  try {
    if (!activeSession) {
      activeSession = new SimStdioSession(paths.simCliBin, packs, 42);
      const persistedPrefs = readUserPreferences();
      for (const [eventClassKey, behavior] of Object.entries(persistedPrefs.notificationRules)) {
        if (typeof behavior === "string") {
          await activeSession.request({
            op: "command",
            command: { kind: "updateNotificationRule", eventClassKey, behavior },
          });
        }
      }
      ensureLiveClockRunning();
    }
    const resp = await activeSession.request({ op: "snapshot" });
    if (!resp.ok || !resp.snapshot) {
      throw new Error(resp.message ?? "Failed to read initial snapshot from sim-cli serve-stdio");
    }
    const snapshot = normalizeSnapshot(resp.snapshot);
    return { state: "ready", snapshot };
  } catch (err) {
    return {
      state: "mod_error",
      failure: {
        ok: false,
        diagnostics: [
          {
            severity: "Error",
            layer: "Snapshot",
            filePath: "sim-cli serve-stdio",
            line: 1,
            column: 1,
            fieldPath: "serve-stdio",
            message: String(err),
          },
        ],
      },
    };
  }
}

function createMainWindow(paths: RuntimePaths): BrowserWindow {
  const headless = process.env.NG_HEADLESS_TEST === "1" || SMOKE_MODE !== null;
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    show: !headless,
    backgroundColor: "#11100e",
    webPreferences: {
      preload: paths.preloadJs,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      backgroundThrottling: !headless,
    },
  });
  void win.loadFile(paths.indexHtml);
  return win;
}

app.whenReady().then(() => {
  const paths = resolveRuntimePaths();

  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    callback({
      responseHeaders: {
        ...details.responseHeaders,
        "Content-Security-Policy": [
          "default-src 'self' file:; connect-src 'self' file: data:; media-src 'self' file: data:; script-src 'self'; style-src 'self' file: 'unsafe-inline'; img-src 'self' file: data:; font-src 'self' file: data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ],
      },
    });
  });

  ipcMain.handle("sim:get-initial-state", () => startOrGetSessionState(paths));

  ipcMain.handle("sim:reload-mods", async (_ev, recoverToDefault: boolean) => {
    await disposeActiveSession();
    if (recoverToDefault) {
      activeExtraPacks = [];
    }
    currentTick = 0;
    isPaused = true;
    speedMultiplier = 1;
    lastAutoPauseReason = null;
    dismissedAutoPauseReasons.clear();
    return startOrGetSessionState(paths);
  });

  ipcMain.handle("sim:send-command", async (ev, rawCmd: unknown) => {
    const parsed = parsePlayerCommand(rawCmd);
    if (!parsed.ok) {
      return {
        accepted: false,
        tick: currentTick,
        errorCode: parsed.errorCode,
        message: parsed.message,
      };
    }
    const cmd = parsed.command;
    if (cmd.kind === "setPaused") {
      isPaused = cmd.paused;
      if (!isPaused && lastAutoPauseReason && lastAutoPauseReason !== "succession_transition") {
        dismissedAutoPauseReasons.add(lastAutoPauseReason);
      }
    }
    if (cmd.kind === "setSpeedMultiplier") {
      speedMultiplier = cmd.multiplier;
    }
    if (cmd.kind === "updateNotificationRule") {
      writeUserPreferences({ notificationRules: { [cmd.eventClassKey]: cmd.behavior } });
    }
    if (cmd.kind === "acknowledgeSuccession") {
      dismissedAutoPauseReasons.add("succession_transition");
      if (lastAutoPauseReason === "succession_transition") {
        lastAutoPauseReason = null;
      }
    }

    if (!activeSession) {
      // Fixture/fallback path if no engine stdio session is active
      if (cachedSnapshot) {
        const updated = JSON.parse(JSON.stringify(cachedSnapshot)) as Record<string, unknown>;
        if (cmd.kind === "acknowledgeSuccession") {
          const succ = updated.succession as Record<string, unknown> | undefined;
          if (succ) succ.pendingTransition = null;
          const clk = updated.clockAndNotifications as Record<string, unknown> | undefined;
          if (clk) clk.autoPauseReason = null;
        }
        const normalized = normalizeSnapshot(updated);
        ev.sender.send("sim:snapshot", normalized);
      }
      return {
        accepted: true,
        tick: currentTick,
        errorCode: null,
        message: `Executed ${cmd.kind} in fixture state`,
      };
    }

    try {
      const resp =
        cmd.kind === "stepSingleTick"
          ? await activeSession.request({ op: "step", ticks: 1 })
          : await activeSession.request({ op: "command", command: cmd });
      if (!resp.ok) {
        return {
          accepted: false,
          tick: resp.tick ?? currentTick,
          errorCode: resp.errorCode ?? "INVALID_COMMAND",
          message: resp.message ?? "Command rejected by engine",
        };
      }
      if (resp.snapshot) {
        const normalized = normalizeSnapshot(resp.snapshot);
        ev.sender.send("sim:snapshot", normalized);
      }
      if (cmd.kind === "setPaused" && !isPaused) {
        ensureLiveClockRunning();
      }
      return {
        accepted: true,
        tick: resp.tick ?? currentTick,
        errorCode: null,
        message: `Executed ${cmd.kind} (paused=${isPaused}, speed=${speedMultiplier})`,
      };
    } catch (err) {
      return {
        accepted: false,
        tick: currentTick,
        errorCode: "SESSION_ERROR",
        message: String(err),
      };
    }
  });

  ipcMain.handle("sim:list-skins", () => discoverSkinDescriptors(paths.skinsDir));

  ipcMain.handle("sim:read-skin-manifest", (_ev, skinId: string) =>
    readSkinManifestBundle(paths.skinsDir, skinId),
  );

  ipcMain.handle("sim:read-user-prefs", () => readUserPreferences());

  ipcMain.handle("sim:write-user-prefs", (_ev, rawPrefs: unknown) => {
    writeUserPreferences(rawPrefs);
  });

  ipcMain.handle("sim:load-mock-fixture", (ev, fixtureJson: string) => {
    try {
      const parsed: unknown = JSON.parse(fixtureJson);
      const snap = Array.isArray(parsed) ? parsed[0] : parsed;
      if (typeof snap === "object" && snap !== null) {
        isPaused = true;
        lastAutoPauseReason = null;
        dismissedAutoPauseReasons.clear();
        const normalized = normalizeSnapshot(snap as Record<string, unknown>);
        ev.sender.send("sim:snapshot", normalized);
      }
    } catch {
      // Ignore malformed fixture strings cleanly
    }
  });

  const mainWindow = createMainWindow(paths);
  mainWebContents = mainWindow.webContents;

  // Hot-reload CSS/skins in non-smoke dev runs when skin manifests or stylesheets change on disk.
  const skinWatchers: fs.FSWatcher[] = [];
  if (SMOKE_MODE === null && !app.isPackaged && fs.existsSync(paths.skinsDir)) {
    let debounceTimer: NodeJS.Timeout | null = null;
    for (const skin of discoverSkinDescriptors(paths.skinsDir)) {
      const skinDir = path.dirname(skin.stylesheetPath);
      try {
        const w = fs.watch(skinDir, () => {
          if (debounceTimer) clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            if (!mainWindow.isDestroyed()) {
              mainWindow.webContents.send("sim:skin-hot-reload", skin.skinId);
            }
          }, 120);
        });
        skinWatchers.push(w);
      } catch {
        // Filesystem watcher unavailable on some mounts; manual switch remains functional.
      }
    }
  }
  app.on("before-quit", () => {
    for (const w of skinWatchers) w.close();
    void disposeActiveSession();
  });

  if (SMOKE_MODE !== null) {
    mainWindow.webContents.once("did-finish-load", async () => {
      try {
        for (let attempt = 0; attempt < 120; attempt++) {
          const ready = await mainWindow.webContents.executeJavaScript("Boolean(window.__ngBootStatus?.ready)");
          if (ready) break;
          await new Promise((r) => setTimeout(r, 40));
        }

        if (SMOKE_MODE.startsWith("m2-")) {
          const m2Result = await runM2E2ESmokePhase(mainWindow, SMOKE_MODE);
          console.log("M2_SMOKE_RESULT:" + JSON.stringify(m2Result));
          await disposeActiveSession();
          app.exit(0);
          return;
        }
        if (SMOKE_MODE.startsWith("m3-m4")) {
          const m34Result = await runM3M4E2ESmokePhase(mainWindow, SMOKE_MODE);
          console.log("M3_M4_SMOKE_RESULT:" + JSON.stringify(m34Result));
          await disposeActiveSession();
          app.exit(0);
          return;
        }
        if (SMOKE_MODE.startsWith("m5-m6-m7")) {
          const m567Result = await runM5M6M7E2ESmoke(mainWindow, SMOKE_MODE);
          console.log("M5_M6_M7_SMOKE_RESULT:" + JSON.stringify(m567Result));
          await disposeActiveSession();
          app.exit(0);
          return;
        }

        const initial = await mainWindow.webContents.executeJavaScript(`(() => {
          const dlg = document.getElementById("mod-diagnostic-dialog");
          const rows = Array.from(document.querySelectorAll("#mod-diagnostic-dialog tbody tr")).map(tr => tr.textContent);
          return {
            status: window.__ngBootStatus,
            dialogOpen: Boolean(dlg && dlg.hasAttribute("open")),
            dialogRows: rows,
            hudTitle: document.getElementById("hud-scenario-title")?.textContent ?? ""
          };
        })()`);

        const ackGood = await mainWindow.webContents.executeJavaScript(
          `window.__ngSendTestCommand({ kind: "pingBridge", tag: "smoke-ipc" })`,
        );
        const ackBad = await mainWindow.webContents.executeJavaScript(
          `window.__ngSendTestCommand({ kind: "evilExploitCmd" })`,
        );

        let recovered = null;
        if (SMOKE_MODE === "corrupt-mod") {
          await mainWindow.webContents.executeJavaScript(`window.__ngTriggerRecoverDefault()`);
          recovered = await mainWindow.webContents.executeJavaScript(`(() => {
            const dlg = document.getElementById("mod-diagnostic-dialog");
            return {
              status: window.__ngBootStatus,
              dialogOpen: Boolean(dlg && dlg.hasAttribute("open")),
              hudTitle: document.getElementById("hud-scenario-title")?.textContent ?? ""
            };
          })()`);
        }

        console.log("SMOKE_RESULT:" + JSON.stringify({ smokeMode: SMOKE_MODE, initial, ackGood, ackBad, recovered }));
        app.exit(0);
      } catch (err) {
        console.error("SMOKE_ERROR:", err);
        app.exit(1);
      }
    });
  }
});

app.on("window-all-closed", () => {
  app.quit();
});
