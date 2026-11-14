# Module Design: Altitude-Reactive Web Audio Mixer & Skin Foley Engine

**Module ID:** `UI-AUD-09`  
**Code Location:** `apps/ui-shell/src/audio/`  
**Milestone & Design Depth:** `M2–M4: Full Detailed Design`  
**Satisfies Requirements:** `UI-AUD-001`, `UI-AUD-002`, `UI-AUD-003`, `UI-AUD-004` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `UiIntentBus` (`PlayUiFoley`, `SwitchActiveSkin`)
  - Continuous `cameraAltitudeNorm` (`0.0` surface -> `1.0` orbital map table) published by `UI-GLB-02`
  - `DiegeticStore` (`selectObservedWeather` around current camera focus cell, strictly clipped to observed cells!)
  - External `audio_manifest.toml` from active skin / scenario mod
- **Provided Spine Ports (Outputs):**
  - Mixed audio stream routed to browser Web Audio `AudioContext.destination`
  - Persistent `AudioMixerSettings` state (`masterVolume`, `musicVolume`, `ambienceVolume`, `foleyVolume`, per-channel mute flags) bound to settings window

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Web Audio Node Graph

```text
[MusicStemPlayer]      --> [MusicBusGain]    --\
[SurfaceWeatherLoops]  --> [SurfaceEnvGain]  ---> [AmbienceBusGain] ---> [MasterGain] --> Destination
[StudyRoomAmbiance]    --> [OrbitalEnvGain]  --/                         /
[FoleyVoicePool]       --> [FoleyBusGain]    ---------------------------/
```

### 2.2 External Audio Manifest Schema (`audio_manifest.toml`)

```toml
schema_version = 1

[ambience]
surface_wind_loop = "ambience/wind_loop.ogg"
surface_rain_loop = "ambience/rain_loop.ogg"
surface_thunder_oneshots = ["ambience/thunder_01.ogg", "ambience/thunder_02.ogg"]
orbital_study_room_loop = "ambience/study_hearth_clock_loop.ogg"

[music]
peacetime_stems = ["music/colony_theme_01.ogg"]

[foley]
window_open = { clip = "foley/parchment_unroll.ogg", cooldown_ms = 120, gain = 0.7 }
window_close = { clip = "foley/parchment_fold.ogg", cooldown_ms = 120, gain = 0.7 }
dispatch_arrived = { clip = "foley/courier_bell_quill.ogg", cooldown_ms = 400, gain = 0.85 }
proposal_stamped = { clip = "foley/wax_seal_press.ogg", cooldown_ms = 250, gain = 0.9 }
clock_step = { clip = "foley/chronometer_tick.ogg", cooldown_ms = 60, gain = 0.6 }
```

Zero `.ogg`, `.wav`, or `.mp3` literal file paths ever appear inside TypeScript source code (`UI-AUD-001`).

---

## 3. Algorithms, Altitude Cross-Fade & Debounced Foley

### 3.1 Equal-Power Camera-Altitude Environmental Cross-Fade (`UI-AUD-002`)
Given `altitude = clamp(cameraAltitudeNorm, 0.0, 1.0)`:
1. Compute `t = smoothstep(0.15, 0.75, altitude)`:
   - `surfaceGain = Math.cos(t * Math.PI * 0.5)` (`1.0` near ground -> `0.0` at high map table)
   - `orbitalGain = Math.sin(t * Math.PI * 0.5)` (`0.0` near ground -> `1.0` at high map table)
2. Modulate individual surface stems by **observed** local weather at camera target (`observedPrecip`, `observedWind`, `observedSettlementActivity`), clamping smooth linear ramps (`gain.setTargetAtTime(..., currentTime, 0.15)`) so audio never pops or clips (`surfaceGain^2 + orbitalGain^2 == 1.0`).
3. If the camera hovers over an `Unexplored` cell, surface weather loop falls back cleanly to neutral ocean/wind hush so sound never leaks hidden storm knowledge (`ENG-EPI-001` alignment!).

### 3.2 Cooldown-Throttled Skin Foley (`UI-AUD-003`)
1. When `PlayUiFoley { cueId }` arrives on `UiIntentBus`, verify `nowMs - lastPlayedAtMs[cueId] >= entry.cooldown_ms` and enforce active polyphony cap (`<= 6` concurrent foley voices) before scheduling `AudioBufferSourceNode`.
2. Switching skins via `SwitchActiveSkin` reloads `audio_manifest.toml` asynchronously and swaps clip buffers cleanly without interrupting music playback.

### 3.3 Persistent Volume & Mute Mixer (`UI-AUD-004`)
Updates apply smoothly (`20ms` ramp) to `MasterGain`, `MusicBusGain`, `AmbienceBusGain`, and `FoleyBusGain` (setting muted buses to `0.0` gain while preserving slider values in local user preferences storage).

---

## 4. Isolated Cartridge Verification Strategy

1. **Zero Hardcoded Audio Paths (`@req UI-AUD-001`):** Static scan asserts zero audio extension literals inside `apps/ui-shell/src/`, and manifest unit tests confirm resilience against missing optional foley keys.
2. **Equal-Power Altitude Cross-Fade (`@req UI-AUD-002`):** Vitest sweeps `cameraAltitudeNorm` from `0.0` to `1.0` against mock gain nodes asserting monotonic surface fade-out, orbital fade-in, constant total power (`<= 1.0`), and zero storm leakage over `Unexplored` cells.
3. **Foley Debounce & Skin Swap (`@req UI-AUD-003`):** Fires 20 rapid `clock_step` intents within `50ms` and asserts cooldown throttling restricts playback cleanly.
4. **Mixer Persistence (`@req UI-AUD-004`):** Playwright adjusts sliders and mute toggles, reloads app fixture, and asserts restored Web Audio gain node values.

---

## 5. Lift-and-Shift Replacement Guide

1. Swapping Web Audio API for an external spatial WASM audio synthesizer or FMOD/Howler wrapper modifies strictly `apps/ui-shell/src/audio/`.
2. All other UI modules simply emit `PlayUiFoley` intents onto `UiIntentBus`.
