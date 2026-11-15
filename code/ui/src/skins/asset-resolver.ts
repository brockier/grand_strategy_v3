import {
  AssetManifestDto,
  AssetUrlResolverPort,
  PortraitLookupQuery,
} from "../spine";

const EMERGENCY_PORTRAIT_SVG =
  "data:image/svg+xml;charset=utf-8," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64"><circle cx="32" cy="24" r="12" fill="currentColor"/><path d="M12 58c0-11 9-18 20-18s20 7 20 18z" fill="currentColor"/></svg>',
  );

const EMERGENCY_ICON_SVG =
  "data:image/svg+xml;charset=utf-8," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32"><rect x="4" y="4" width="24" height="24" rx="4" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="16" cy="16" r="5" fill="currentColor"/></svg>',
  );

const EMERGENCY_PLATE_SVG =
  "data:image/svg+xml;charset=utf-8," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 32" width="96" height="32"><rect x="2" y="2" width="92" height="28" fill="none" stroke="currentColor" stroke-width="2"/></svg>',
  );

/**
 * Attaches an asset URL onto an `<img>` element with deterministic one-shot
 * fallback substitution if the primary resource fails to load (`UI-SKN-005`).
 */
export function attachSafeImageSource(
  img: HTMLImageElement,
  primaryUrl: string,
  fallbackUrl: string,
  altText: string,
): void {
  img.alt = altText;
  let fellBack = false;
  img.addEventListener("error", () => {
    if (!fellBack && fallbackUrl && img.src !== fallbackUrl) {
      fellBack = true;
      img.src = fallbackUrl;
    }
  });
  img.src = primaryUrl || fallbackUrl;
}

/**
 * Tag-scored resolver mapping character attributes, commodity keys, and dispatch
 * plate keys to active skin assets with guaranteed `CC0-1.0` fallbacks (`UI-SKN-005`).
 */
export class AssetResolver implements AssetUrlResolverPort {
  private activeSkinId = "cartographer-parchment";
  private skinIds: readonly string[] = ["cartographer-parchment"];
  private manifest?: AssetManifestDto;

  public updateFromSkin(
    skinId: string,
    manifest: AssetManifestDto | undefined,
    availableSkinIds?: readonly string[],
  ): void {
    this.activeSkinId = skinId;
    this.manifest = manifest;
    if (availableSkinIds && availableSkinIds.length > 0) {
      this.skinIds = [...availableSkinIds];
    }
  }

  public getAvailableSkinIds(): readonly string[] {
    return this.skinIds;
  }

  public getActiveSkinId(): string {
    return this.activeSkinId;
  }

  public getPortraitFallbackUrl(): string {
    return this.manifest?.fallbacks.portraitUrl || EMERGENCY_PORTRAIT_SVG;
  }

  public getIconFallbackUrl(): string {
    return this.manifest?.fallbacks.iconUrl || EMERGENCY_ICON_SVG;
  }

  public getDispatchPlateFallbackUrl(): string {
    return this.manifest?.fallbacks.dispatchPlateUrl || EMERGENCY_PLATE_SVG;
  }

  public resolvePortrait(query: PortraitLookupQuery): string {
    const portraits = this.manifest?.portraits ?? [];
    if (portraits.length === 0) {
      return this.getPortraitFallbackUrl();
    }

    const hasExplicitTags = Boolean(
      query.vocation || query.ageBracket || query.eraTag || query.healthBand,
    );

    let bestScore = -1;
    let candidates: typeof portraits[number][] = [];

    for (const p of portraits) {
      let score = 0;
      if (query.vocation && p.vocation === query.vocation) score += 4;
      if (query.ageBracket && p.ageBracket === query.ageBracket) score += 3;
      if (query.eraTag && p.eraTag === query.eraTag) score += 2;
      if (query.healthBand && p.healthBand === query.healthBand) score += 1;

      if (score > bestScore) {
        bestScore = score;
        candidates = [p];
      } else if (score === bestScore) {
        candidates.push(p);
      }
    }

    if (hasExplicitTags && bestScore <= 0) {
      return this.getPortraitFallbackUrl();
    }
    const pool = candidates.length > 0 ? candidates : portraits;
    const idx = Math.abs(Math.floor(query.characterId)) % pool.length;
    return pool[idx]?.url || this.getPortraitFallbackUrl();
  }

  public resolveIcon(key: string): string {
    const icons = this.manifest?.icons ?? [];
    const found = icons.find((ic) => ic.key === key);
    return found?.url || this.getIconFallbackUrl();
  }

  public resolveDispatchPlate(key: string): string {
    const plates = this.manifest?.dispatchPlates ?? [];
    const found = plates.find((dp) => dp.key === key);
    return found?.url || this.getDispatchPlateFallbackUrl();
  }
}
