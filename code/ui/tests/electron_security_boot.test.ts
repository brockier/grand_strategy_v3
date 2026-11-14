import { execFileSync } from "node:child_process";
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
  };
  ackGood: { accepted: boolean; errorCode: string | null; message: string };
  ackBad: { accepted: boolean; errorCode: string | null; message: string };
  recovered: {
    status: { ready: boolean; mode: string };
    dialogOpen: boolean;
    hudTitle: string;
  } | null;
}

function runElectronSmoke(extraArgs: string[]): { durationMs: number; result: SmokePayload } {
  const start = Date.now();
  const stdout = execFileSync(ELECTRON_BIN, [MAIN_JS, ...extraArgs], {
    cwd: UI_ROOT,
    env: { ...process.env, NG_HEADLESS_TEST: "1" },
    encoding: "utf8",
    timeout: 25000,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const line = stdout.split(/\r?\n/).find((l) => l.startsWith("SMOKE_RESULT:"));
  if (!line) {
    throw new Error(`Missing SMOKE_RESULT line in Electron output:\n${stdout}`);
  }
  return {
    durationMs: Date.now() - start,
    result: JSON.parse(line.slice("SMOKE_RESULT:".length)) as SmokePayload,
  };
}

describe("Electron Security Sandbox, GPU Geodesic Globe & Mod Recovery", () => {
  // @req UI-SHL-001
  // @req UI-SHL-003
  it("boots Electron with contextIsolation, Chromium sandbox, Dawn WebGPU/WebGL2 shader compilation, and strict IPC validation (UI-SHL-001, UI-SHL-003)", () => {
    const { result } = runElectronSmoke(["--smoke-mode=healthy"]);
    const status = result.initial.status;

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

    const { durationMs, result } = runElectronSmoke([
      "--smoke-mode=corrupt-mod",
      `--corrupt-pack=${badPackDir}`,
    ]);

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
});
