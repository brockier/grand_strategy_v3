export const BASE_WINDOW_WIDTH = 1280;
export const BASE_WINDOW_HEIGHT = 800;
export const TARGET_WINDOW_WIDTH = BASE_WINDOW_WIDTH * 2;
export const TARGET_WINDOW_HEIGHT = BASE_WINDOW_HEIGHT * 2;

export interface DisplayWorkAreaSize {
  readonly width: number;
  readonly height: number;
}

/**
 * Computes the initial desktop window dimensions, targeting twice the baseline
 * 1280x800 viewport while clamping cleanly to the active display work area.
 */
export function computeInitialWindowSize(
  workArea: DisplayWorkAreaSize | null | undefined,
): { readonly width: number; readonly height: number } {
  if (!workArea || workArea.width <= 0 || workArea.height <= 0) {
    return { width: TARGET_WINDOW_WIDTH, height: TARGET_WINDOW_HEIGHT };
  }
  return {
    width: Math.min(TARGET_WINDOW_WIDTH, workArea.width),
    height: Math.min(TARGET_WINDOW_HEIGHT, workArea.height),
  };
}
