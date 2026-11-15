export type ValidatedCommand =
  | { readonly kind: "setPaused"; readonly paused: boolean }
  | { readonly kind: "stepSingleTick" }
  | { readonly kind: "setSpeedMultiplier"; readonly multiplier: number }
  | { readonly kind: "pingBridge"; readonly tag: string };

export type CommandParseResult =
  | { readonly ok: true; readonly command: ValidatedCommand }
  | { readonly ok: false; readonly errorCode: string; readonly message: string };

/** Strictly validates outbound command payloads received across Electron IPC (`UI-SHL-002`). */
export function parsePlayerCommand(payload: unknown): CommandParseResult {
  if (typeof payload !== "object" || payload === null) {
    return { ok: false, errorCode: "INVALID_PAYLOAD", message: "Command must be a non-null object" };
  }
  const obj = payload as Record<string, unknown>;
  const keys = Object.keys(obj);
  switch (obj.kind) {
    case "setPaused":
      if (typeof obj.paused !== "boolean" || keys.length !== 2) {
        return { ok: false, errorCode: "INVALID_ARGUMENT", message: "setPaused requires boolean 'paused'" };
      }
      return { ok: true, command: { kind: "setPaused", paused: obj.paused } };
    case "stepSingleTick":
      if (keys.length !== 1) {
        return { ok: false, errorCode: "INVALID_ARGUMENT", message: "stepSingleTick takes no extra fields" };
      }
      return { ok: true, command: { kind: "stepSingleTick" } };
    case "setSpeedMultiplier":
      if (
        typeof obj.multiplier !== "number" ||
        !Number.isInteger(obj.multiplier) ||
        obj.multiplier < 1 ||
        (obj.multiplier > 64 && obj.multiplier !== 168) ||
        keys.length !== 2
      ) {
        return {
          ok: false,
          errorCode: "OUT_OF_RANGE",
          message: "multiplier must be integer 1..64 or 168",
        };
      }
      return { ok: true, command: { kind: "setSpeedMultiplier", multiplier: obj.multiplier } };
    case "pingBridge":
      if (typeof obj.tag !== "string" || obj.tag.length === 0 || obj.tag.length > 64 || keys.length !== 2) {
        return { ok: false, errorCode: "INVALID_ARGUMENT", message: "pingBridge requires non-empty string 'tag'" };
      }
      return { ok: true, command: { kind: "pingBridge", tag: obj.tag } };
    default:
      return { ok: false, errorCode: "UNAUTHORIZED_COMMAND", message: `Unknown command kind '${String(obj.kind)}'` };
  }
}
