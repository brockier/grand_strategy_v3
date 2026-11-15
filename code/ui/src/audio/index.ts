import {
  AudioManifestDto,
  AudioMixerSettings,
  AudioPreferencesPort,
  DEFAULT_AUDIO_MIXER_SETTINGS,
  DiegeticStoreAccess,
  SimBridgeApi,
  UiIntentBus,
  UserPreferencesDto,
  WindowContentPlugin,
} from "../spine";

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.min(1, Math.max(0, v));
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  const span = edge1 - edge0;
  if (Math.abs(span) < 1e-6) return x >= edge1 ? 1 : 0;
  const t = clamp01((x - edge0) / span);
  return t * t * (3 - 2 * t);
}

export interface ComputedAudioMix {
  readonly master: number;
  readonly music: number;
  readonly ambience: number;
  readonly foley: number;
  readonly surfaceEnv: number;
  readonly orbitalEnv: number;
  readonly rainGain: number;
  readonly windGain: number;
  readonly settlementGain: number;
}

export interface FoleyPlaybackRecord {
  readonly cueId: string;
  readonly resolvedUrl: string;
  readonly timestampMs: number;
}

/**
 * External-manifest-driven Web Audio mixer & zoom-altitude environmental synthesizer
 * (`UI-AUD-001`, `UI-AUD-002`, `UI-AUD-004`). Contains zero hardcoded audio file literals.
 */
export class AudioMixerEngine implements AudioPreferencesPort {
  private readonly bus: UiIntentBus;
  private readonly store?: DiegeticStoreAccess;
  private readonly bridge?: SimBridgeApi;
  private settings: AudioMixerSettings;
  private manifest?: AudioManifestDto;
  private altitudeNorm = 0.62;
  private focusCellId = 0;

  private audioCtx: AudioContext | null = null;
  private masterGainNode: GainNode | null = null;
  private musicGainNode: GainNode | null = null;
  private ambienceGainNode: GainNode | null = null;
  private foleyGainNode: GainNode | null = null;

  private readonly lastCuePlayMs = new Map<string, number>();
  private readonly playedFoleyLog: FoleyPlaybackRecord[] = [];
  private readonly settingsListeners = new Set<() => void>();
  private readonly cleanups: Array<() => void> = [];
  private lastSeenDispatchCount: number | null = null;

  constructor(
    bus: UiIntentBus,
    options: {
      readonly store?: DiegeticStoreAccess;
      readonly bridge?: SimBridgeApi;
      readonly initialSettings?: Partial<AudioMixerSettings>;
    } = {},
  ) {
    this.bus = bus;
    this.store = options.store;
    this.bridge = options.bridge;
    this.settings = {
      ...DEFAULT_AUDIO_MIXER_SETTINGS,
      ...(options.initialSettings ?? {}),
    };

    this.initWebAudioGraphIfSupported();

    const unsubBus = this.bus.subscribe((intent) => {
      if (intent.kind === "SkinBundleUpdated") {
        this.manifest = intent.bundle.audioManifest;
        this.syncAudioNodes();
      } else if (intent.kind === "UpdateCameraTelemetry") {
        this.altitudeNorm = clamp01(intent.telemetry.altitudeNorm);
        this.focusCellId = intent.telemetry.focusCellId;
        this.syncAudioNodes();
      } else if (intent.kind === "PlayUiFoley") {
        this.triggerFoleyCue(intent.cueId);
      }
    });
    this.cleanups.push(unsubBus);

    if (this.store) {
      this.cleanups.push(
        this.store.subscribe((snap) => {
          this.syncAudioNodes();
          const count = snap.dispatchInbox.length;
          if (this.lastSeenDispatchCount !== null && count > this.lastSeenDispatchCount) {
            this.triggerFoleyCue("alert_bell");
          }
          this.lastSeenDispatchCount = count;
        }),
      );
    }

    this.syncAudioNodes();
  }

  private initWebAudioGraphIfSupported(): void {
    if (typeof window === "undefined") return;
    const AudioCtxCtor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtxCtor) return;
    try {
      const ctx = new AudioCtxCtor();
      const master = ctx.createGain();
      const music = ctx.createGain();
      const ambience = ctx.createGain();
      const foley = ctx.createGain();
      music.connect(master);
      ambience.connect(master);
      foley.connect(master);
      master.connect(ctx.destination);

      this.audioCtx = ctx;
      this.masterGainNode = master;
      this.musicGainNode = music;
      this.ambienceGainNode = ambience;
      this.foleyGainNode = foley;
    } catch {
      // Headless environments without audio backend retain deterministic gain model.
    }
  }

  public getManifest(): AudioManifestDto | undefined {
    return this.manifest;
  }

  public getPlayedFoleyLog(): readonly FoleyPlaybackRecord[] {
    return this.playedFoleyLog;
  }

  public getSettings(): AudioMixerSettings {
    return { ...this.settings };
  }

  public subscribeSettings(listener: () => void): () => void {
    this.settingsListeners.add(listener);
    return () => {
      this.settingsListeners.delete(listener);
    };
  }

  public setAltitudeAndCell(altitudeNorm: number, focusCellId = this.focusCellId): void {
    this.altitudeNorm = clamp01(altitudeNorm);
    this.focusCellId = focusCellId;
    this.syncAudioNodes();
  }

  public updateSettings(patch: Partial<AudioMixerSettings>, persist = true): void {
    const next: AudioMixerSettings = {
      masterVolume: patch.masterVolume !== undefined ? clamp01(patch.masterVolume) : this.settings.masterVolume,
      musicVolume: patch.musicVolume !== undefined ? clamp01(patch.musicVolume) : this.settings.musicVolume,
      ambienceVolume: patch.ambienceVolume !== undefined ? clamp01(patch.ambienceVolume) : this.settings.ambienceVolume,
      foleyVolume: patch.foleyVolume !== undefined ? clamp01(patch.foleyVolume) : this.settings.foleyVolume,
      masterMuted: patch.masterMuted ?? this.settings.masterMuted,
      musicMuted: patch.musicMuted ?? this.settings.musicMuted,
      ambienceMuted: patch.ambienceMuted ?? this.settings.ambienceMuted,
      foleyMuted: patch.foleyMuted ?? this.settings.foleyMuted,
    };
    this.settings = next;
    this.syncAudioNodes();
    for (const listener of this.settingsListeners) listener();

    if (persist && this.bridge?.writeUserPreferences) {
      void this.persistToBridge(next);
    }
  }

  private async persistToBridge(next: AudioMixerSettings): Promise<void> {
    if (!this.bridge?.writeUserPreferences) return;
    try {
      const existing: UserPreferencesDto = this.bridge.readUserPreferences
        ? await this.bridge.readUserPreferences()
        : { audioMixer: next, namedLayoutPresets: {} };
      await this.bridge.writeUserPreferences({
        ...existing,
        audioMixer: next,
      });
    } catch {
      // Non-fatal persistence safeguard
    }
  }

  public getEffectiveGains(): ComputedAudioMix {
    const master = this.settings.masterMuted ? 0 : clamp01(this.settings.masterVolume);
    const music = this.settings.musicMuted ? 0 : clamp01(this.settings.musicVolume) * master;
    const ambience = this.settings.ambienceMuted ? 0 : clamp01(this.settings.ambienceVolume) * master;
    const foley = this.settings.foleyMuted ? 0 : clamp01(this.settings.foleyVolume) * master;

    const surfStart = this.manifest?.ambientSurface.fadeStartAltitude ?? 0.2;
    const surfEnd = this.manifest?.ambientSurface.fadeEndAltitude ?? 0.72;
    const studyStart = this.manifest?.ambientMapStudy.fadeStartAltitude ?? 0.25;
    const studyEnd = this.manifest?.ambientMapStudy.fadeEndAltitude ?? 0.78;

    const rawSurface = 1 - smoothstep(surfStart, surfEnd, this.altitudeNorm);
    const rawOrbital = smoothstep(studyStart, studyEnd, this.altitudeNorm);
    const normFactor = Math.max(1, rawSurface + rawOrbital);
    const surfaceEnv = clamp01((rawSurface / normFactor) * ambience);
    const orbitalEnv = clamp01((rawOrbital / normFactor) * ambience);

    // Epistemic guard (`UI-AUD-002`): never trigger rainfall or settlement audio over Unexplored/Stale cells.
    let rainIntensity = 0;
    let settlementIntensity = 0;
    const snap = this.store?.getSnapshot();
    if (snap) {
      const knownCell = snap.knownCells.find((c) => c.cellId === this.focusCellId);
      if (knownCell && knownCell.tier === "surveyed") {
        const weather = snap.observedWeather.find((w) => w.cellId === this.focusCellId);
        if (weather) {
          rainIntensity = clamp01(weather.precipMmHr / 20);
        }
        const hasSettlement = (snap.knownSettlements ?? []).some((s) => s.cellId === this.focusCellId);
        if (hasSettlement) {
          settlementIntensity = 0.85;
        }
      }
    }

    const windGain = clamp01(surfaceEnv * 0.75);
    const rainGain = clamp01(surfaceEnv * rainIntensity);
    const settlementGain = clamp01(surfaceEnv * settlementIntensity);

    return {
      master,
      music,
      ambience,
      foley,
      surfaceEnv,
      orbitalEnv,
      rainGain,
      windGain,
      settlementGain,
    };
  }

  private syncAudioNodes(): void {
    const gains = this.getEffectiveGains();
    if (this.masterGainNode) this.masterGainNode.gain.value = gains.master;
    if (this.musicGainNode) this.musicGainNode.gain.value = gains.music;
    if (this.ambienceGainNode) this.ambienceGainNode.gain.value = gains.ambience;
    if (this.foleyGainNode) this.foleyGainNode.gain.value = gains.foley;
  }

  public getWebAudioNodeValues(): {
    readonly hasContext: boolean;
    readonly master: number;
    readonly music: number;
    readonly ambience: number;
    readonly foley: number;
  } {
    const fallback = this.getEffectiveGains();
    return {
      hasContext: this.audioCtx !== null,
      master: this.masterGainNode ? this.masterGainNode.gain.value : fallback.master,
      music: this.musicGainNode ? this.musicGainNode.gain.value : fallback.music,
      ambience: this.ambienceGainNode ? this.ambienceGainNode.gain.value : fallback.ambience,
      foley: this.foleyGainNode ? this.foleyGainNode.gain.value : fallback.foley,
    };
  }

  public triggerFoleyCue(cueId: string, nowMs = Date.now()): boolean {
    const resolvedUrl = this.manifest?.uiFoley.cueUrls[cueId] ?? "";
    if (!resolvedUrl) return false;
    const cooldown = this.manifest?.uiFoley.cooldownMs ?? 60;
    const prev = this.lastCuePlayMs.get(cueId) ?? -Infinity;
    if (nowMs - prev < cooldown) return false;

    this.lastCuePlayMs.set(cueId, nowMs);
    this.playedFoleyLog.push({ cueId, resolvedUrl, timestampMs: nowMs });
    if (this.playedFoleyLog.length > 64) this.playedFoleyLog.shift();

    if (this.audioCtx && this.foleyGainNode && this.audioCtx.state === "running") {
      try {
        const osc = this.audioCtx.createOscillator();
        const env = this.audioCtx.createGain();
        osc.type = "triangle";
        let freq = 420;
        if (cueId === "window_close") freq = 240;
        else if (cueId === "mandate_seal") freq = 310;
        else if (cueId === "dispatch_seal_break") freq = 360;
        else if (cueId === "ledger_page_turn") freq = 510;
        else if (cueId === "alert_bell") freq = 660;
        osc.frequency.value = freq;
        env.gain.setValueAtTime(0.08, this.audioCtx.currentTime);
        env.gain.exponentialRampToValueAtTime(0.001, this.audioCtx.currentTime + 0.045);
        osc.connect(env);
        env.connect(this.foleyGainNode);
        osc.start();
        osc.stop(this.audioCtx.currentTime + 0.05);
      } catch {
        // Guard against audio graph disposal races
      }
    }
    return true;
  }

  public dispose(): void {
    while (this.cleanups.length > 0) {
      this.cleanups.pop()?.();
    }
    if (this.audioCtx && typeof this.audioCtx.close === "function") {
      void this.audioCtx.close();
    }
  }
}

export function mountAudioController(
  bus: UiIntentBus,
  store?: DiegeticStoreAccess,
  bridge?: SimBridgeApi,
  initialSettings?: Partial<AudioMixerSettings>,
): AudioMixerEngine & (() => void) {
  const engine = new AudioMixerEngine(bus, { store, bridge, initialSettings });
  const handle = Object.assign(() => engine.dispose(), {
    getSettings: () => engine.getSettings(),
    subscribeSettings: (listener: () => void) => engine.subscribeSettings(listener),
    updateSettings: (patch: Partial<AudioMixerSettings>) => engine.updateSettings(patch),
    getEffectiveGains: () => engine.getEffectiveGains(),
    setAltitudeAndCell: (alt: number, cellId?: number) => engine.setAltitudeAndCell(alt, cellId),
    getManifest: () => engine.getManifest(),
    getPlayedFoleyLog: () => engine.getPlayedFoleyLog(),
    getWebAudioNodeValues: () => engine.getWebAudioNodeValues(),
    triggerFoleyCue: (cueId: string, nowMs?: number) => engine.triggerFoleyCue(cueId, nowMs),
    dispose: () => engine.dispose(),
  });
  return handle as unknown as AudioMixerEngine & (() => void);
}

/** Accessible Audio Mixer floating window with CSS `--control-value` synced range sliders (`UI-AUD-004`). */
export function createAudioSettingsWindowPlugin(mixer: AudioMixerEngine): WindowContentPlugin {
  return {
    pluginId: "audio_mixer",
    defaultTitle: "Audio & Soundscape Mixer",
    minWidth: 360,
    minHeight: 240,
    hotkey: "m",
    mountContent(container, _instanceKey, _spine) {
      const form = document.createElement("form");
      form.addEventListener("submit", (e) => e.preventDefault());

      const channels: Array<{
        readonly id: string;
        readonly label: string;
        readonly volKey: "masterVolume" | "musicVolume" | "ambienceVolume" | "foleyVolume";
        readonly muteKey: "masterMuted" | "musicMuted" | "ambienceMuted" | "foleyMuted";
      }> = [
        { id: "master", label: "Master Volume", volKey: "masterVolume", muteKey: "masterMuted" },
        { id: "music", label: "Music Stems", volKey: "musicVolume", muteKey: "musicMuted" },
        { id: "ambience", label: "World Ambience", volKey: "ambienceVolume", muteKey: "ambienceMuted" },
        { id: "foley", label: "UI Foley", volKey: "foleyVolume", muteKey: "foleyMuted" },
      ];

      const updaters: Array<() => void> = [];

      for (const ch of channels) {
        const row = document.createElement("div");
        row.className = "ng-mixer-row";

        const inputId = `audio-slider-${ch.id}`;
        const label = document.createElement("label");
        label.htmlFor = inputId;
        label.textContent = ch.label;

        const range = document.createElement("input");
        range.type = "range";
        range.id = inputId;
        range.className = "ng-range-slider";
        range.min = "0";
        range.max = "100";
        range.step = "1";
        range.setAttribute("data-audio-channel", ch.id);

        const out = document.createElement("output");
        out.htmlFor = inputId;

        const muteBtn = document.createElement("button");
        muteBtn.type = "button";
        muteBtn.className = "ng-btn";
        muteBtn.setAttribute("data-audio-mute", ch.id);

        const syncRow = () => {
          const s = mixer.getSettings();
          const pct = Math.round(s[ch.volKey] * 100);
          range.value = String(pct);
          range.style.setProperty("--control-value", range.value);
          out.textContent = `${pct}%`;
          const muted = s[ch.muteKey];
          muteBtn.setAttribute("aria-pressed", muted ? "true" : "false");
          muteBtn.textContent = muted ? "Muted" : "Mute";
        };

        range.addEventListener("input", () => {
          range.style.setProperty("--control-value", range.value);
          const v = Number(range.value) / 100;
          mixer.updateSettings({ [ch.volKey]: v });
        });

        muteBtn.addEventListener("click", () => {
          const cur = mixer.getSettings()[ch.muteKey];
          mixer.updateSettings({ [ch.muteKey]: !cur });
        });

        syncRow();
        updaters.push(syncRow);
        row.append(label, range, out, muteBtn);
        form.appendChild(row);
      }

      container.replaceChildren(form);
      return mixer.subscribeSettings(() => {
        for (const fn of updaters) fn();
      });
    },
  };
}
