export interface SerializedWindowTabDto {
  readonly pluginId: string;
  readonly instanceKey: string;
  readonly pinned: boolean;
}

export interface SerializedWindowFrameDto {
  readonly frameId: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly collapsed: boolean;
  readonly activeTabIndex: number;
  readonly tabs: readonly SerializedWindowTabDto[];
}

export interface WindowLayoutDocumentDto {
  readonly schemaVersion: 1;
  readonly viewportWidth: number;
  readonly viewportHeight: number;
  readonly frames: readonly SerializedWindowFrameDto[];
}

export interface PluginMinBounds {
  readonly minWidth: number;
  readonly minHeight: number;
  readonly maxWidth?: number;
  readonly maxHeight?: number;
}

export const HUD_TOP_MARGIN_PX = 48;
export const MIN_HEADER_VISIBLE_X_PX = 80;
export const MIN_HEADER_VISIBLE_Y_PX = 36;

export const BUILTIN_LAYOUT_PRESETS: Readonly<Record<string, WindowLayoutDocumentDto>> = {
  "war-room": {
    schemaVersion: 1,
    viewportWidth: 1280,
    viewportHeight: 800,
    frames: [
      {
        frameId: "war-inspector",
        x: 16,
        y: 64,
        width: 360,
        height: 420,
        collapsed: false,
        activeTabIndex: 0,
        tabs: [
          { pluginId: "entity_inspector", instanceKey: "default", pinned: false },
          { pluginId: "dispatch_inbox", instanceKey: "default", pinned: false },
        ],
      },
      {
        frameId: "war-ledger",
        x: 880,
        y: 64,
        width: 380,
        height: 360,
        collapsed: false,
        activeTabIndex: 0,
        tabs: [{ pluginId: "physical_ledger", instanceKey: "default", pinned: false }],
      },
    ],
  },
  "economic-audit": {
    schemaVersion: 1,
    viewportWidth: 1280,
    viewportHeight: 800,
    frames: [
      {
        frameId: "audit-ledger",
        x: 20,
        y: 64,
        width: 560,
        height: 520,
        collapsed: false,
        activeTabIndex: 0,
        tabs: [
          { pluginId: "physical_ledger", instanceKey: "default", pinned: false },
          { pluginId: "shell_overview", instanceKey: "default", pinned: false },
        ],
      },
      {
        frameId: "audit-inspector",
        x: 880,
        y: 64,
        width: 376,
        height: 320,
        collapsed: false,
        activeTabIndex: 0,
        tabs: [{ pluginId: "entity_inspector", instanceKey: "default", pinned: false }],
      },
    ],
  },
  "minimalist-exploration": {
    schemaVersion: 1,
    viewportWidth: 1280,
    viewportHeight: 800,
    frames: [
      {
        frameId: "minimal-overview",
        x: 16,
        y: 64,
        width: 320,
        height: 200,
        collapsed: true,
        activeTabIndex: 0,
        tabs: [{ pluginId: "shell_overview", instanceKey: "default", pinned: false }],
      },
    ],
  },
};

function clamp(v: number, lo: number, hi: number): number {
  if (hi < lo) return lo;
  return Math.min(hi, Math.max(lo, v));
}

export function clampFramePosition(
  x: number,
  y: number,
  width: number,
  viewportW: number,
  viewportH: number,
  minTopMarginPx: number = HUD_TOP_MARGIN_PX,
): { readonly x: number; readonly y: number } {
  const minVisibleX = Math.min(MIN_HEADER_VISIBLE_X_PX, width);
  const minX = -(width - minVisibleX);
  const maxX = Math.max(0, viewportW - minVisibleX);
  const minY = Math.max(HUD_TOP_MARGIN_PX, Math.round(minTopMarginPx));
  const maxY = Math.max(minY, viewportH - MIN_HEADER_VISIBLE_Y_PX);
  return {
    x: Math.round(clamp(x, minX, maxX)),
    y: Math.round(clamp(y, minY, maxY)),
  };
}

export function serializeLayout(
  frames: readonly SerializedWindowFrameDto[],
  viewportWidth: number,
  viewportHeight: number,
): string {
  const doc: WindowLayoutDocumentDto = {
    schemaVersion: 1,
    viewportWidth: Math.max(320, Math.round(viewportWidth)),
    viewportHeight: Math.max(240, Math.round(viewportHeight)),
    frames,
  };
  return JSON.stringify(doc);
}

/**
 * Restores and scales serialized window layouts proportionally to the current
 * viewport resolution, guaranteeing zero orphaned offscreen frames (`UI-WIN-007`).
 */
export function deserializeAndClampLayout(
  rawJson: string,
  targetViewportWidth: number,
  targetViewportHeight: number,
  getBounds?: (pluginId: string) => PluginMinBounds | undefined,
  minTopMarginPx: number = HUD_TOP_MARGIN_PX,
): WindowLayoutDocumentDto | null {
  try {
    const parsed: unknown = JSON.parse(rawJson);
    if (typeof parsed !== "object" || parsed === null) return null;
    const obj = parsed as Record<string, unknown>;
    if (obj.schemaVersion !== 1 || !Array.isArray(obj.frames)) return null;

    const srcW =
      typeof obj.viewportWidth === "number" && obj.viewportWidth > 0 ? obj.viewportWidth : targetViewportWidth;
    const srcH =
      typeof obj.viewportHeight === "number" && obj.viewportHeight > 0 ? obj.viewportHeight : targetViewportHeight;

    const dstW = Math.max(320, targetViewportWidth);
    const dstH = Math.max(240, targetViewportHeight);
    const scaleX = dstW / srcW;
    const scaleY = dstH / srcH;
    const topMargin = Math.max(HUD_TOP_MARGIN_PX, Math.round(minTopMarginPx));

    const clampedFrames: SerializedWindowFrameDto[] = [];
    for (const rawFrame of obj.frames) {
      if (typeof rawFrame !== "object" || rawFrame === null) continue;
      const f = rawFrame as Record<string, unknown>;
      if (!Array.isArray(f.tabs) || f.tabs.length === 0) continue;

      const tabs: SerializedWindowTabDto[] = [];
      for (const rt of f.tabs) {
        if (typeof rt !== "object" || rt === null) continue;
        const t = rt as Record<string, unknown>;
        if (typeof t.pluginId !== "string") continue;
        tabs.push({
          pluginId: t.pluginId,
          instanceKey: typeof t.instanceKey === "string" ? t.instanceKey : "default",
          pinned: Boolean(t.pinned),
        });
      }
      if (tabs.length === 0) continue;

      let minW = 240;
      let minH = 160;
      let maxW = dstW - 16;
      let maxH = dstH - topMargin - 12;
      for (const tab of tabs) {
        const b = getBounds?.(tab.pluginId);
        if (b) {
          minW = Math.max(minW, b.minWidth);
          minH = Math.max(minH, b.minHeight);
          if (b.maxWidth !== undefined) maxW = Math.min(maxW, b.maxWidth);
          if (b.maxHeight !== undefined) maxH = Math.min(maxH, b.maxHeight);
        }
      }

      const rawWidth = typeof f.width === "number" ? f.width * scaleX : minW;
      const rawHeight = typeof f.height === "number" ? f.height * scaleY : minH;
      const width = Math.round(clamp(rawWidth, minW, Math.max(minW, maxW)));
      const height = Math.round(clamp(rawHeight, minH, Math.max(minH, maxH)));

      const scaledX = typeof f.x === "number" ? f.x * scaleX : 16;
      const scaledY = typeof f.y === "number" ? f.y * scaleY : topMargin + 12;
      const clampedPos = clampFramePosition(scaledX, scaledY, width, dstW, dstH, topMargin);

      clampedFrames.push({
        frameId: typeof f.frameId === "string" && f.frameId.length > 0 ? f.frameId : `frame-${clampedFrames.length + 1}`,
        x: clampedPos.x,
        y: clampedPos.y,
        width,
        height,
        collapsed: Boolean(f.collapsed),
        activeTabIndex:
          typeof f.activeTabIndex === "number"
            ? clamp(Math.floor(f.activeTabIndex), 0, tabs.length - 1)
            : 0,
        tabs,
      });
    }

    return {
      schemaVersion: 1,
      viewportWidth: dstW,
      viewportHeight: dstH,
      frames: clampedFrames,
    };
  } catch {
    return null;
  }
}
