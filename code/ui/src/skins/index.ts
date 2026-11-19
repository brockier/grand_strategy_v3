import {
  DEFAULT_AUDIO_MIXER_SETTINGS,
  SimBridgeApi,
  SkinDescriptor,
  SkinManifestBundle,
  UiIntentBus,
  WindowContentPlugin,
} from "../spine";
import { AssetResolver } from "./asset-resolver";

export { AssetResolver, attachSafeImageSource } from "./asset-resolver";

function rebaseCssRelativeUrls(cssText: string, stylesheetHref: string): string {
  if (
    !stylesheetHref ||
    (!stylesheetHref.startsWith("file://") &&
      !stylesheetHref.startsWith("http://") &&
      !stylesheetHref.startsWith("https://"))
  ) {
    return cssText;
  }
  return cssText.replace(/url\((['"]?)(\.[^)'"]+)\1\)/g, (_m, q, rel) => {
    try {
      const abs = new URL(rel, stylesheetHref).href;
      return `url(${q}${abs}${q})`;
    } catch {
      return _m;
    }
  });
}

/**
 * Applies an external CSS skin bundle atomically via `<link id="active-skin-css">`
 * and synchronized inline style block (`UI-SKN-001`, `UI-SKN-003`).
 */
export function applySkinBundle(bundle: SkinManifestBundle, bustCache = false): void {
  const href = bustCache && bundle.stylesheetHref
    ? `${bundle.stylesheetHref}${bundle.stylesheetHref.includes("?") ? "&" : "?"}t=${Date.now()}`
    : bundle.stylesheetHref;

  let link = document.getElementById("active-skin-css") as HTMLLinkElement | null;
  if (!link) {
    link = document.createElement("link");
    link.id = "active-skin-css";
    link.rel = "stylesheet";
    document.head.appendChild(link);
  }
  if (href) {
    link.href = href;
  }

  let inlineStyle = document.getElementById("active-skin-inline-css") as HTMLStyleElement | null;
  if (bundle.cssText.trim().length > 0) {
    if (!inlineStyle) {
      inlineStyle = document.createElement("style");
      inlineStyle.id = "active-skin-inline-css";
      document.head.appendChild(inlineStyle);
    }
    inlineStyle.textContent = rebaseCssRelativeUrls(bundle.cssText, bundle.stylesheetHref);
  } else if (inlineStyle) {
    inlineStyle.textContent = "";
  }

  document.documentElement.setAttribute("data-active-skin", bundle.skinId);
}

/**
 * Manages runtime external skin discovery, atomic CSS/token hot-swapping,
 * asset manifest updates, and dev-mode hot reload (`UI-SKN-001`, `UI-SKN-003`, `UI-SKN-004`).
 */
export class SkinManager {
  private readonly bridge: SimBridgeApi;
  private readonly bus: UiIntentBus;
  public readonly assetResolver: AssetResolver;
  private availableSkins: SkinDescriptor[] = [];
  private currentBundle: SkinManifestBundle | null = null;
  private readonly cleanups: Array<() => void> = [];

  constructor(bridge: SimBridgeApi, bus: UiIntentBus, assetResolver = new AssetResolver()) {
    this.bridge = bridge;
    this.bus = bus;
    this.assetResolver = assetResolver;

    const unsubBus = this.bus.subscribe((intent) => {
      if (intent.kind === "SwitchActiveSkin") {
        void this.switchSkin(intent.skinId, true);
      }
    });
    this.cleanups.push(unsubBus);

    if (this.bridge.subscribeSkinHotReload) {
      const unsubHot = this.bridge.subscribeSkinHotReload((changedSkinId) => {
        if (!this.currentBundle || this.currentBundle.skinId === changedSkinId) {
          void this.switchSkin(changedSkinId, false, true);
        }
      });
      this.cleanups.push(unsubHot);
    }
  }

  public getAvailableSkins(): readonly SkinDescriptor[] {
    return this.availableSkins;
  }

  public getActiveBundle(): SkinManifestBundle | null {
    return this.currentBundle;
  }

  public async initialize(preferredSkinId?: string): Promise<SkinManifestBundle | null> {
    this.availableSkins = [...(await this.bridge.listAvailableSkins())];
    if (this.availableSkins.length === 0) return null;

    const match =
      preferredSkinId && this.availableSkins.some((s) => s.skinId === preferredSkinId)
        ? preferredSkinId
        : this.availableSkins[0].skinId;
    return this.switchSkin(match, false, false);
  }

  public async switchSkin(
    skinId: string,
    persistPreference = true,
    bustCache = false,
  ): Promise<SkinManifestBundle> {
    if (this.availableSkins.length === 0) {
      this.availableSkins = [...(await this.bridge.listAvailableSkins())];
    }
    const bundle = await this.bridge.readSkinAssetManifest(skinId);
    this.currentBundle = bundle;
    this.assetResolver.updateFromSkin(
      bundle.skinId,
      bundle.assetManifest,
      this.availableSkins.map((s) => s.skinId),
    );
    applySkinBundle(bundle, bustCache);
    this.bus.emit({ kind: "SkinBundleUpdated", bundle });

    if (persistPreference && this.bridge.writeUserPreferences) {
      try {
        const existing = this.bridge.readUserPreferences
          ? await this.bridge.readUserPreferences()
          : { audioMixer: { ...DEFAULT_AUDIO_MIXER_SETTINGS }, namedLayoutPresets: {} };
        await this.bridge.writeUserPreferences({
          ...existing,
          activeSkinId: bundle.skinId,
        });
      } catch {
        // Non-fatal persistence safeguard
      }
    }
    return bundle;
  }

  public dispose(): void {
    while (this.cleanups.length > 0) {
      this.cleanups.pop()?.();
    }
  }
}

export function createSkinSwitcherWindowPlugin(skinManager: SkinManager): WindowContentPlugin {
  return {
    pluginId: "skin_appearance",
    defaultTitle: "Cartography Skins & Appearance",
    minWidth: 320,
    minHeight: 220,
    hotkey: "y",
    mountContent(container, _instanceKey, spine) {
      const wrap = document.createElement("div");
      wrap.className = "ng-responsive-grid";
      wrap.setAttribute("role", "radiogroup");
      wrap.setAttribute("aria-label", "Select interface skin pack");

      const render = () => {
        const activeId = skinManager.getActiveBundle()?.skinId ?? spine.assetResolver?.getActiveSkinId() ?? "";
        const skins = skinManager.getAvailableSkins();
        const nodes: HTMLElement[] = [];

        for (const skin of skins) {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "ng-btn";
          btn.setAttribute("role", "radio");
          btn.setAttribute("data-skin-choice", skin.skinId);
          btn.setAttribute("aria-checked", skin.skinId === activeId ? "true" : "false");
          if (skin.skinId === activeId) btn.classList.add("ng-btn-primary");
          btn.textContent = skin.displayName;
          btn.addEventListener("click", () => {
            spine.bus.emit({ kind: "SwitchActiveSkin", skinId: skin.skinId });
          });
          nodes.push(btn);
        }

        wrap.replaceChildren(...nodes);
      };

      render();
      container.replaceChildren(wrap);
      return spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") render();
      });
    },
  };
}
