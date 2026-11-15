# Module Design: Electron Shell, WebGPU Boot & Steam Packaging

**Module ID:** `UI-SHL-01`  
**Code Location:** `code/ui/electron/` and `code/ui/scripts/`  
**Milestone & Design Depth:** `M0: Full Detailed Design`  
**Satisfies Requirements:** `UI-LIC-001`, `UI-LIC-002`, `UI-LIC-003`, `UI-SHL-001`, `UI-SHL-003`, `UI-SHL-004` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - Compiled `player-view` + `sim-core` native library (via `napi-rs` addon or typed stdio/IPC daemon)
  - Outgoing `ValidatedPlayerCommand` payloads sent from renderer `SimBridgeApi`
  - External `package-lock.json`, font license manifests, and optional `--steam` packaging target flag
- **Provided Spine Ports (Outputs):**
  - Sandboxed `window.simBridge` implementation of `SimBridgeApi` injected via Electron `preload.ts`
  - Chromium Dawn WebGPU / WebGL2 canvas host window
  - Pre-globe structured Mod & Schema Diagnostic recovery overlay (`UI-SHL-004`)

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Hardened Electron `BrowserWindow` Configuration (`UI-SHL-003`)

```typescript
const mainWindow = new BrowserWindow({
  width: 1440,
  height: 900,
  backgroundColor: "#11100e",
  webPreferences: {
    preload: path.join(__dirname, "preload.js"),
    contextIsolation: true,
    nodeIntegration: false,
    sandbox: true,
    webSecurity: true,
  },
});
```

### 2.2 Linux Dawn/WebGPU Switch Matrix (`UI-SHL-001`)

- Launches Chromium with Vulkan Dawn flags enabled across Linux X11 and Wayland (`--enable-unsafe-webgpu`, `--use-vulkan=vulkan`, `--enable-features=Vulkan`) without touching host `WebKitGTK` system libraries.
- Probes `navigator.gpu.requestAdapter()` inside renderer bootstrap; if headless CI or legacy GPU hardware lacks Vulkan WebGPU support, falls back deterministically to Chromium's ANGLE WebGL2 backend and records active backend mode in startup telemetry.

### 2.3 Electron Builder Steam Packaging Manifest (`UI-LIC-003`)

```yaml
asar: true
asarUnpack:
  - "**/*.node"
  - "native/bin/**"
  - "native/lib/libsteam_api.so"
  - "native/lib/steam_api64.dll"
extraResources:
  - from: "../../dist/THIRD_PARTY_NOTICES.txt"
    to: "licenses/THIRD_PARTY_NOTICES.txt"
  - from: "../../mods/"
    to: "mods/"
  - from: "../../skins/"
    to: "skins/"
linux:
  target: ["dir"]
  executableName: "ng-colony"
win:
  target: ["dir"]
```

---

## 3. Algorithms, Diagnostic Recovery & License Gates

### 3.1 Preload Bridge & IPC Channel Validation (`UI-SHL-002`, `UI-SHL-003`)
1. `preload.ts` exposes strictly named methods (`subscribeSnapshots`, `sendPlayerCommand`, `listAvailableSkins`, `readSkinAssetManifest`, `loadMockFixture`) via `contextBridge.exposeInMainWorld("simBridge", ...)`.
2. Every `ipcMain.handle` route validates command payloads against strict TypeScript/Zod schemas before forwarding into Rust `sim-command` / `player-view`.
3. `window.require`, `global.process`, and Node filesystem APIs remain completely inaccessible from renderer DOM code.

### 3.2 Structured Mod & Schema Diagnostic Screen (`UI-SHL-004`)
1. At startup, the main process invokes `sim-data` validation across the selected mod load order.
2. If any `ModDiagnostic` errors return, Electron skips globe boot cleanly and sends a `ModValidationFailureState` payload to the lightweight diagnostic screen displaying:
   - Mod pack layer name and relative file path
   - Exact line and column number plus offending TOML/JSON field path
   - Human-readable constraint error and **Reload Mods** / **Revert to Default Pack** buttons

### 3.3 Automated NPM & Font License Gate (`UI-LIC-001`, `UI-LIC-002`)
1. CI runs `license-checker` / custom SPDX verifier over `package-lock.json`, permitting strictly `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `0BSD`, and `CC0-1.0` for runtime dependencies—blocking `GPL`, `AGPL`, static `LGPL`, `SSPL`, and paid commercial UI grid/chart packages.
2. Font manifest checker verifies every `.ttf`/`.woff2` font inside `skins/` pairs with an `OFL-1.1`, `Apache-2.0`, `MIT`, or `CC0-1.0` notice file.
3. Packaging step verifies `LICENSES.chromium.html` and `THIRD_PARTY_NOTICES.txt` sit in the output folder and checks `app.asar` contains zero `.node` or `.so`/`.dll` native binaries (`asarUnpack`).

---

## 4. Isolated Cartridge Verification Strategy

1. **NPM & Font Audits (`@req UI-LIC-001`, `@req UI-LIC-002`):** Runs CI SPDX scanner against production `package-lock.json` and skin font directories, verifying failure on poisoned fixture dependencies.
2. **Steam Package Structure Test (`@req UI-LIC-003`):** Inspects built artifact directory verifying unpacked `.node`/`.so` binaries, `LICENSES.chromium.html`, `THIRD_PARTY_NOTICES.txt`, and clean launch under Steam Linux Runtime container tests.
3. **Dawn WebGPU/WebGL2 Boot (`@req UI-SHL-001`):** Playwright Electron boot test asserts active GPU context creation (`WebGPU` or `WebGL2`) and zero renderer crashes across resize events.
4. **Security & Mod Diagnostic Tests (`@req UI-SHL-003`, `@req UI-SHL-004`):** Verifies `typeof window.require === "undefined"`, injects a broken TOML fixture, and asserts exact file/line/column diagnostic rendering within 2 seconds.

---

## 5. Lift-and-Shift Replacement Guide

1. Swapping `napi-rs` native bindings for a local socket/stdin Rust child process—or replacing `electron-builder` with `electron-forge`—touches strictly `code/ui/electron/` and `code/ui/scripts/`.
2. The renderer (`src/spine` and all view/globe cartridges) sees only `window.simBridge` and never changes.
