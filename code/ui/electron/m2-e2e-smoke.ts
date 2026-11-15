import type { BrowserWindow } from "electron";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function runM2E2ESmokePhase(
  mainWindow: BrowserWindow,
  phase: string,
): Promise<Record<string, unknown>> {
  for (let i = 0; i < 120; i += 1) {
    const ready = await mainWindow.webContents.executeJavaScript(
      "Boolean(window.__ngBootStatus?.ready && window.__ngM2Runtime)",
    );
    if (ready) break;
    await delay(40);
  }

  if (phase === "m2-phase2") {
    return (await mainWindow.webContents.executeJavaScript(`(async () => {
      const rt = window.__ngM2Runtime;
      await new Promise((r) => setTimeout(r, 80));
      const presets = rt.windowManager.listPresetNames();
      const applied = rt.windowManager.applyNamedPreset("e2e-saved-desk");
      return {
        phase: "m2-phase2",
        activeSkinId: rt.skinManager.getActiveBundle()?.skinId ?? "",
        documentSkinAttr: document.documentElement.getAttribute("data-active-skin") ?? "",
        audioSettings: rt.audioEngine.getSettings(),
        audioNodes: rt.audioEngine.getWebAudioNodeValues(),
        presets,
        appliedSavedPreset: applied,
        restoredFrameCount: rt.windowManager.listFrames().length,
        panelBg: getComputedStyle(document.querySelector(".ng-hud-bar")).backgroundColor,
      };
    })()`)) as Record<string, unknown>;
  }

  // Phase 1: Real GPU WebGL2 render + mouse drag + wheel occlusion + live skin swap + persisted settings.
  const preDrag = (await mainWindow.webContents.executeJavaScript(`(() => {
    const rt = window.__ngM2Runtime;
    const hqCell = rt.globe.topology.cells[0];
    rt.globe.camera.flyToUnitVector(hqCell.centerUnit, 0.25, 0, true);
    rt.globe.stepRender(16);
    const sample = rt.globe.captureFrameSample(48);
    const strideBytes = [];
    if (sample) {
      for (let i = 0; i < sample.rgba.length; i += 32) {
        strideBytes.push(sample.rgba[i], sample.rgba[i + 1], sample.rgba[i + 2]);
      }
    }
    const hud = document.querySelector(".ng-hud-bar");
    return {
      skinId: rt.skinManager.getActiveBundle()?.skinId ?? "",
      hudBg: hud ? getComputedStyle(hud).backgroundColor : "",
      hudFont: hud ? getComputedStyle(hud).fontFamily : "",
      focusStart: rt.globe.camera.getFocusUnit(),
      avgRgbStart: sample ? sample.averageRgb : [0, 0, 0],
      rgbaStart: strideBytes,
    };
  })()`)) as Record<string, unknown>;

  // Send genuine Chromium pointer drag downward to orbit over poles (`UI-GLB-001`).
  mainWindow.webContents.sendInputEvent({ type: "mouseDown", x: 640, y: 380, button: "left", clickCount: 1 });
  for (let step = 1; step <= 6; step += 1) {
    mainWindow.webContents.sendInputEvent({ type: "mouseMove", x: 640, y: 380 + step * 35 });
    await delay(15);
  }
  mainWindow.webContents.sendInputEvent({ type: "mouseUp", x: 640, y: 590, button: "left", clickCount: 1 });
  await delay(60);

  // Send genuine wheel zooms to surface vs orbital & measure GPU shader blend pixel shift (`UI-GLB-002`).
  const gpuZoomAndDrag = (await mainWindow.webContents.executeJavaScript(`(() => {
    const rt = window.__ngM2Runtime;
    const focusAfterDrag = rt.globe.camera.getFocusUnit();
    rt.globe.stepRender(16);
    const afterDragSample = rt.globe.captureFrameSample(48);

    const hqCell = rt.globe.topology.cells[0];
    rt.globe.camera.flyToUnitVector(hqCell.centerUnit, 0.05, 0, true);
    rt.globe.stepRender(16);
    const surfaceBlend = rt.globe.status.cartographicBlend;
    const surfaceSample = rt.globe.captureFrameSample(48);

    rt.globe.camera.setAltitudeNorm(0.95);
    rt.globe.stepRender(16);
    const orbitalBlend = rt.globe.status.cartographicBlend;
    const orbitalSample = rt.globe.captureFrameSample(48);

    const stride = (s) => {
      const out = [];
      if (!s) return out;
      for (let i = 0; i < s.rgba.length; i += 32) {
        out.push(s.rgba[i], s.rgba[i + 1], s.rgba[i + 2]);
      }
      return out;
    };

    return {
      focusAfterDrag,
      rgbaAfterDrag: stride(afterDragSample),
      surfaceBlend,
      orbitalBlend,
      rgbaSurface: stride(surfaceSample),
      rgbaOrbital: stride(orbitalSample),
    };
  })()`)) as Record<string, unknown>;

  // Click HUD +1 Hour button & inspect windows + 9-slice + wheel occlusion.
  const winSetup = (await mainWindow.webContents.executeJavaScript(`(async () => {
    const rt = window.__ngM2Runtime;
    const initialTick = rt.store.getSnapshot()?.clockAndNotifications.tick ?? 0;
    document.getElementById("hud-step-btn")?.click();
    await new Promise((r) => setTimeout(r, 140));
    const afterStepTick = rt.store.getSnapshot()?.clockAndNotifications.tick ?? 0;

    const smallId = rt.windowManager.openWindow("entity_inspector", { x: 40, y: 100, width: 320, height: 220 });
    const smallEl = document.getElementById(smallId);
    const smallCs = smallEl ? getComputedStyle(smallEl) : null;

    const largeId = rt.windowManager.openWindow("physical_ledger", { x: 400, y: 100, width: 840, height: 560 });
    const largeEl = document.getElementById(largeId);
    const largeCs = largeEl ? getComputedStyle(largeEl) : null;

    // Put filter text & scroll inside ledger, collapse & expand
    const inputEl = largeEl?.querySelector("[data-ledger-filter='true']");
    if (inputEl) inputEl.value = "grain";
    rt.windowManager.toggleCollapse(largeId);
    rt.windowManager.toggleCollapse(largeId);
    const preservedFilter = largeEl?.querySelector("[data-ledger-filter='true']")?.value ?? "";

    const altBeforeWheelOverWindow = rt.globe.camera.getAltitudeNorm();
    return {
      initialTick,
      afterStepTick,
      smallId,
      largeId,
      smallBorderImage: smallCs?.borderImageSource ?? "",
      smallBorderSlice: smallCs?.borderImageSlice ?? "",
      smallBorderWidth: smallCs?.borderTopWidth ?? "",
      largeBorderImage: largeCs?.borderImageSource ?? "",
      largeBorderWidth: largeCs?.borderTopWidth ?? "",
      preservedFilter,
      altBeforeWheelOverWindow,
    };
  })()`)) as Record<string, unknown>;

  // Send genuine wheel event OVER the open ledger window at (600, 300) -> globe zoom MUST NOT change (`UI-WIN-005`).
  mainWindow.webContents.sendInputEvent({
    type: "mouseWheel",
    x: 600,
    y: 300,
    deltaX: 0,
    deltaY: -240,
    wheelTicksX: 0,
    wheelTicksY: -2,
  });
  await delay(60);

  // Switch skin live -> industrial-brass, inspect GPU pixel change & DOM style update, save preset & audio settings.
  const skinAndAudio = (await mainWindow.webContents.executeJavaScript(`(async () => {
    const rt = window.__ngM2Runtime;
    const altAfterWheelOverWindow = rt.globe.camera.getAltitudeNorm();

    rt.globe.stepRender(16);
    const beforeSkinSample = rt.globe.captureFrameSample(24);

    await rt.skinManager.switchSkin("industrial-brass", true, false);
    await new Promise((r) => setTimeout(r, 80));
    rt.globe.stepRender(16);
    const afterSkinSample = rt.globe.captureFrameSample(24);

    const hud = document.querySelector(".ng-hud-bar");
    const hudBgBrass = hud ? getComputedStyle(hud).backgroundColor : "";
    const hudFontBrass = hud ? getComputedStyle(hud).fontFamily : "";

    rt.windowManager.saveNamedPreset("e2e-saved-desk");

    const audioWinId = rt.windowManager.openWindow("audio_mixer", { x: 80, y: 360, width: 380, height: 250 });
    const slider = document.querySelector("#" + audioWinId + " #audio-slider-master");
    if (slider) {
      slider.value = "35";
      slider.dispatchEvent(new Event("input", { bubbles: true }));
    }
    const muteAmbBtn = document.querySelector("#" + audioWinId + " [data-audio-mute='ambience']");
    muteAmbBtn?.click();

    await new Promise((r) => setTimeout(r, 80));
    const currentPrefs = await window.simBridge.readUserPreferences();

    return {
      altAfterWheelOverWindow,
      openWindowsAfterSkinSwap: rt.windowManager.listFrames().length,
      skinAfterSwap: rt.skinManager.getActiveBundle()?.skinId ?? "",
      hudBgBrass,
      hudFontBrass,
      avgRgbBeforeSkin: beforeSkinSample ? beforeSkinSample.averageRgb : [0, 0, 0],
      avgRgbAfterSkin: afterSkinSample ? afterSkinSample.averageRgb : [0, 0, 0],
      rgbaBeforeSkin: beforeSkinSample ? Array.from(beforeSkinSample.rgba.slice(0, 64)) : [],
      rgbaAfterSkin: afterSkinSample ? Array.from(afterSkinSample.rgba.slice(0, 64)) : [],
      audioNodes: rt.audioEngine.getWebAudioNodeValues(),
      persistedPrefs: currentPrefs,
    };
  })()`)) as Record<string, unknown>;

  return {
    phase: "m2-phase1",
    preDrag,
    gpuZoomAndDrag,
    winSetup,
    skinAndAudio,
  };
}
