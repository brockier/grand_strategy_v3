export {
  ModDiagnosticDialog,
  createPlaceholderWindowPlugin,
} from "./diagnostic-dialog";
export type { DiagnosticRecoveryActions } from "./diagnostic-dialog";

export {
  BUILTIN_LAYOUT_PRESETS,
  clampFramePosition,
  deserializeAndClampLayout,
  serializeLayout,
} from "./layout-serializer";
export type {
  PluginMinBounds,
  SerializedWindowFrameDto,
  SerializedWindowTabDto,
  WindowLayoutDocumentDto,
} from "./layout-serializer";

export { DesktopWindowManager } from "./window-manager";
export type {
  FrameStateSnapshot,
  OpenWindowOptions,
  ResizeDirection,
} from "./window-manager";
