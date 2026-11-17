import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const UI_ROOT = path.resolve(__dirname, "..");
const ELECTRON_BIN = path.join(UI_ROOT, "node_modules", ".bin", "electron");
const MAIN_JS = path.join(UI_ROOT, "dist-electron", "main.js");
const ESLINT_BIN = path.join(UI_ROOT, "node_modules", ".bin", "eslint");

const tempDirs: string[] = [];
function makeTempDir(prefix: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir) fs.rmSync(dir, { recursive: true, force: true });
  }
});

interface SmokePayload {
  smokeMode: string;
  gpuProcessCrashCount: number;
  initial: {
    status: {
      ready: boolean;
      mode: string;
      gpuBackend: string;
      shaderCompiled: boolean;
      diagnosticCount: number;
      hasNodeRequire: boolean;
      hasProcessGlobal: boolean;
      hasSimBridge: boolean;
    };
    dialogOpen: boolean;
    dialogRows: string[];
    hudTitle: string;
    globeAvgRgb: number[];
  };
  ackGood: { accepted: boolean; errorCode: string | null; message: string };
  ackBad: { accepted: boolean; errorCode: string | null; message: string };
  recovered: {
    status: { ready: boolean; mode: string };
    dialogOpen: boolean;
    hudTitle: string;
  } | null;
  visibleSurface: {
    windowShown: boolean;
    compositedEmpty: boolean;
    compositedSize: { width: number; height: number };
  } | null;
}

const FATAL_GPU_STDERR_RE =
  /is not compatible with Vulkan|Failed to create vulkan surface|GetGeometry failed|XGetWindowAttributes failed|Failed to send GpuControl\.CreateCommandBuffer|ContextResult::kTransientFailure|ContextResult::kFatalFailure|DMABUF/i;

function runElectronSmoke(
  extraArgs: string[],
  options?: { visibleDesktop?: boolean },
): {
  durationMs: number;
  stderr: string;
  result: SmokePayload;
} {
  const start = Date.now();
  const env: NodeJS.ProcessEnv = { ...process.env };
  if (options?.visibleDesktop) {
    delete env.NG_HEADLESS_TEST;
  } else {
    env.NG_HEADLESS_TEST = "1";
  }
  const proc = spawnSync(ELECTRON_BIN, [MAIN_JS, ...extraArgs], {
    cwd: UI_ROOT,
    env,
    encoding: "utf8",
    timeout: 25000,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const stdout = proc.stdout ?? "";
  const stderr = proc.stderr ?? "";
  if (proc.status !== 0) {
    throw new Error(
      `Electron exited with status ${String(proc.status)}:\nSTDOUT:\n${stdout}\nSTDERR:\n${stderr}`,
    );
  }
  const line = stdout.split(/\r?\n/).find((l) => l.startsWith("SMOKE_RESULT:"));
  if (!line) {
    throw new Error(`Missing SMOKE_RESULT line in Electron output:\n${stdout}\nSTDERR:\n${stderr}`);
  }
  return {
    durationMs: Date.now() - start,
    stderr,
    result: JSON.parse(line.slice("SMOKE_RESULT:".length)) as SmokePayload,
  };
}

describe("Electron Security Sandbox, GPU Geodesic Globe & Mod Recovery", () => {
  // @req UI-SHL-001
  // @req UI-SHL-003
  it("boots Electron with contextIsolation, Chromium sandbox, Dawn WebGPU/WebGL2 shader compilation, and strict IPC validation (UI-SHL-001, UI-SHL-003)", () => {
    const { result, stderr } = runElectronSmoke(["--smoke-mode=healthy"]);
    const status = result.initial.status;
    expect(stderr).not.toMatch(FATAL_GPU_STDERR_RE);
    expect(result.gpuProcessCrashCount).toBe(0);

    // UI-SHL-001 assertions
    expect(status.ready).toBe(true);
    expect(status.mode).toBe("live");
    expect(["webgpu", "webgl2"]).toContain(status.gpuBackend);
    expect(status.shaderCompiled).toBe(true);
    expect(result.initial.hudTitle).toContain("The Unexplained Colony");

    // UI-SHL-003 sandbox & IPC assertions
    expect(status.hasNodeRequire).toBe(false);
    expect(status.hasProcessGlobal).toBe(false);
    expect(status.hasSimBridge).toBe(true);
    expect(result.ackGood.accepted).toBe(true);
    expect(result.ackGood.errorCode).toBeNull();
    expect(result.ackBad.accepted).toBe(false);
    expect(result.ackBad.errorCode).toBe("UNAUTHORIZED_COMMAND");

    // UI-SHL-003 build-time architectural boundary: sibling cartridge import rejected by ESLint.
    let lintFailedAsExpected = false;
    try {
      execFileSync(
        ESLINT_BIN,
        ["--stdin", "--stdin-filename", path.join(UI_ROOT, "src", "globe", "_tmp_boundary_check.ts")],
        {
          cwd: UI_ROOT,
          input: `import { ModDiagnosticDialog } from "../windows/index";\nconsole.log(ModDiagnosticDialog);\n`,
          encoding: "utf8",
          stdio: ["pipe", "pipe", "pipe"],
        },
      );
    } catch (err) {
      lintFailedAsExpected = true;
      const execErr = err as { stdout?: string; stderr?: string };
      const combined = `${execErr.stdout ?? ""}\n${execErr.stderr ?? ""}`;
      expect(combined).toContain("Cartridge 'globe' must never import sibling 'windows'");
    }
    expect(lintFailedAsExpected).toBe(true);
  });

  // @req UI-SHL-004
  it("opens non-crashing <dialog> diagnostics on corrupt mod pack within budget and recovers to default pack (UI-SHL-004)", () => {
    const badPackDir = makeTempDir("ng-corrupt-pack-");
    fs.writeFileSync(
      path.join(badPackDir, "pack.toml"),
      `schema_version = 1\npack_id = "corrupt_patch"\ndisplay_name = "Corrupt Smoke Pack"\nlayer = "UserMod"\nload_priority = 10\n`,
    );
    fs.mkdirSync(path.join(badPackDir, "commodities"), { recursive: true });
    fs.writeFileSync(
      path.join(badPackDir, "commodities", "broken_commodities.toml"),
      `[[commodities]]\nid = "wrought_iron"\n_patch = true\nstructural_strength = "not_a_float"\n`,
    );

    const { durationMs, result, stderr } = runElectronSmoke([
      "--smoke-mode=corrupt-mod",
      `--corrupt-pack=${badPackDir}`,
    ]);

    expect(stderr).not.toMatch(FATAL_GPU_STDERR_RE);
    expect(result.gpuProcessCrashCount).toBe(0);
    expect(durationMs).toBeLessThan(15000);
    expect(result.initial.status.ready).toBe(true);
    expect(result.initial.status.mode).toBe("mod_error");
    expect(result.initial.status.diagnosticCount).toBeGreaterThanOrEqual(1);
    expect(result.initial.dialogOpen).toBe(true);
    expect(result.initial.dialogRows.some((r) => r.includes("broken_commodities.toml"))).toBe(true);

    expect(result.recovered).not.toBeNull();
    expect(result.recovered?.status.mode).toBe("live");
    expect(result.recovered?.dialogOpen).toBe(false);
    expect(result.recovered?.hudTitle).toContain("The Unexplained Colony");
  });

  // @req UI-SHL-001
  it("presents a real visible compositor surface with hardware WebGL2 rendering and zero Vulkan/X11/Wayland command-buffer errors (UI-SHL-001)", () => {
    const hasDesktopDisplay =
      process.platform === "win32" ||
      process.platform === "darwin" ||
      Boolean(process.env.WAYLAND_DISPLAY || process.env.DISPLAY);

    const { result, stderr } = runElectronSmoke(
      hasDesktopDisplay
        ? ["--smoke-mode=healthy", "--smoke-visible"]
        : ["--ozone-platform=headless", "--enable-unsafe-swiftshader", "--smoke-mode=healthy", "--smoke-visible"],
      { visibleDesktop: hasDesktopDisplay },
    );

    expect(stderr).not.toMatch(FATAL_GPU_STDERR_RE);
    expect(result.gpuProcessCrashCount).toBe(0);
    expect(result.initial.status.ready).toBe(true);
    expect(result.initial.status.shaderCompiled).toBe(true);
    expect(result.initial.globeAvgRgb.some((v) => v > 0)).toBe(true);
    expect(result.visibleSurface).not.toBeNull();
    expect(result.visibleSurface?.windowShown).toBe(true);
    expect(result.visibleSurface?.compositedEmpty).toBe(false);
    expect(result.visibleSurface?.compositedSize.width).toBeGreaterThanOrEqual(640);
    expect(result.visibleSurface?.compositedSize.height).toBeGreaterThanOrEqual(400);
  });
});
