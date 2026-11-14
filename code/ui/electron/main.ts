import { app, BrowserWindow, ipcMain, session } from "electron";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { parsePlayerCommand } from "./ipc-validator";

// Enable Chromium Dawn WebGPU with Vulkan + WebGL2 fallback across Linux X11, Wayland & Windows.
app.commandLine.appendSwitch("enable-unsafe-webgpu");
app.commandLine.appendSwitch("enable-features", "Vulkan");
app.commandLine.appendSwitch("enable-webgl");
app.commandLine.appendSwitch("ignore-gpu-blocklist");
if (process.env.NG_SOFTWARE_GL === "1") {
  app.commandLine.appendSwitch("use-gl", "angle");
  app.commandLine.appendSwitch("use-angle", "swiftshader-webgl");
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

let activeExtraPacks: string[] = INITIAL_CORRUPT_PACK ? [INITIAL_CORRUPT_PACK] : [];
let currentTick = 0;
let isPaused = false;
let speedMultiplier = 1;

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

function computeInitialState(paths: RuntimePaths): unknown {
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

  const snapRes = runSimCli(paths.simCliBin, "dump-snapshot", packs, ["--ticks", String(currentTick), "--seed", "42"]);
  if (!snapRes.ok) {
    return {
      state: "mod_error",
      failure: {
        ok: false,
        diagnostics: [
          {
            severity: "Error",
            layer: "Snapshot",
            filePath: "sim-cli",
            line: 1,
            column: 1,
            fieldPath: "dump-snapshot",
            message: snapRes.stdout,
          },
        ],
      },
    };
  }
  const snapshot = JSON.parse(snapRes.stdout) as Record<string, unknown>;
  return { state: "ready", snapshot };
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
          "default-src 'self' file:; script-src 'self'; style-src 'self' file:; img-src 'self' file: data:; font-src 'self' file: data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ],
      },
    });
  });

  ipcMain.handle("sim:get-initial-state", () => computeInitialState(paths));

  ipcMain.handle("sim:reload-mods", (_ev, recoverToDefault: boolean) => {
    if (recoverToDefault) {
      activeExtraPacks = [];
    }
    return computeInitialState(paths);
  });

  ipcMain.handle("sim:send-command", (ev, rawCmd: unknown) => {
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
    if (cmd.kind === "setPaused") isPaused = cmd.paused;
    if (cmd.kind === "setSpeedMultiplier") speedMultiplier = cmd.multiplier;
    if (cmd.kind === "stepSingleTick") {
      currentTick += 1;
      const boot = computeInitialState(paths) as { state: string; snapshot?: unknown };
      if (boot.state === "ready" && boot.snapshot) {
        ev.sender.send("sim:snapshot", boot.snapshot);
      }
    }
    return {
      accepted: true,
      tick: currentTick,
      errorCode: null,
      message: `Executed ${cmd.kind} (paused=${isPaused}, speed=${speedMultiplier})`,
    };
  });

  ipcMain.handle("sim:list-skins", () => {
    const skinDir = path.join(paths.skinsDir, "cartographer-parchment");
    if (!fs.existsSync(skinDir)) return [];
    return [
      {
        skinId: "cartographer-parchment",
        displayName: "Cartographer's Parchment",
        stylesheetPath: path.join(skinDir, "theme.css"),
      },
    ];
  });

  ipcMain.handle("sim:read-skin-manifest", (_ev, skinId: string) => {
    const safeId = path.basename(skinId);
    const cssPath = path.join(paths.skinsDir, safeId, "theme.css");
    const cssText = fs.existsSync(cssPath) ? fs.readFileSync(cssPath, "utf8") : "";
    return {
      skinId: safeId,
      displayName: "Cartographer's Parchment",
      stylesheetHref: fs.existsSync(cssPath) ? pathToFileURL(cssPath).href : "",
      cssText,
      globePalette: {
        oceanShallowHex: "#87a3ad",
        oceanDeepHex: "#3d5966",
        landLowHex: "#d8caa6",
        landHighHex: "#8c7258",
        contourInkHex: "#3b2c1d",
        unexploredVellumHex: "#e6d8b9",
        parchmentGrainTexture: "textures/parchment_tile.png",
        hatchPatternTexture: "textures/ink_hatch_tile.png",
      },
    };
  });

  ipcMain.handle("sim:load-mock-fixture", () => undefined);

  const mainWindow = createMainWindow(paths);

  if (SMOKE_MODE !== null) {
    mainWindow.webContents.once("did-finish-load", async () => {
      try {
        for (let attempt = 0; attempt < 100; attempt++) {
          const ready = await mainWindow.webContents.executeJavaScript("Boolean(window.__ngBootStatus?.ready)");
          if (ready) break;
          await new Promise((r) => setTimeout(r, 40));
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
