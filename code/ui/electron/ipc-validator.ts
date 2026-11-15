export type ValidatedCommand =
  | { readonly kind: "setPaused"; readonly paused: boolean }
  | { readonly kind: "stepSingleTick" }
  | { readonly kind: "setSpeedMultiplier"; readonly multiplier: number }
  | { readonly kind: "pingBridge"; readonly tag: string }
  | {
      readonly kind: "issueMandate";
      readonly assignee: number;
      readonly objectiveKind: string;
      readonly targetCell: number;
      readonly priorityWeight: number;
      readonly authorityTier: string;
    }
  | {
      readonly kind: "resolveArbitrationProposal";
      readonly proposalId: number;
      readonly chosenOptionIndex: number;
    }
  | {
      readonly kind: "setPersonalAttentionZoom";
      readonly targetMandateId: number | null;
      readonly share01: number;
    }
  | {
      readonly kind: "relocateHq";
      readonly destinationCell: number;
    }
  | {
      readonly kind: "updateNotificationRule";
      readonly eventClassKey: string;
      readonly behavior: string;
    };

export type CommandParseResult =
  | { readonly ok: true; readonly command: ValidatedCommand }
  | { readonly ok: false; readonly errorCode: string; readonly message: string };

const ALLOWED_NOTIFICATION_BEHAVIORS = new Set<string>([
  "LogOnly",
  "BannerAlert",
  "DispatchPopup",
  "DispatchPopupAndAutoPause",
  "log_only",
  "banner_alert",
  "dispatch_popup",
  "dispatch_popup_and_auto_pause",
]);

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
    case "issueMandate":
      if (
        keys.length !== 6 ||
        typeof obj.assignee !== "number" ||
        !Number.isInteger(obj.assignee) ||
        obj.assignee < 0 ||
        typeof obj.objectiveKind !== "string" ||
        obj.objectiveKind.length === 0 ||
        obj.objectiveKind.length > 64 ||
        typeof obj.targetCell !== "number" ||
        !Number.isInteger(obj.targetCell) ||
        obj.targetCell < 0 ||
        typeof obj.priorityWeight !== "number" ||
        !Number.isInteger(obj.priorityWeight) ||
        obj.priorityWeight < 1 ||
        obj.priorityWeight > 10 ||
        typeof obj.authorityTier !== "string" ||
        obj.authorityTier.length === 0 ||
        obj.authorityTier.length > 64
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "issueMandate requires valid assignee, objectiveKind, targetCell, priorityWeight (1..10), and authorityTier",
        };
      }
      return {
        ok: true,
        command: {
          kind: "issueMandate",
          assignee: obj.assignee,
          objectiveKind: obj.objectiveKind,
          targetCell: obj.targetCell,
          priorityWeight: obj.priorityWeight,
          authorityTier: obj.authorityTier,
        },
      };
    case "resolveArbitrationProposal":
      if (
        keys.length !== 3 ||
        typeof obj.proposalId !== "number" ||
        !Number.isInteger(obj.proposalId) ||
        obj.proposalId < 1 ||
        typeof obj.chosenOptionIndex !== "number" ||
        !Number.isInteger(obj.chosenOptionIndex) ||
        obj.chosenOptionIndex < 0 ||
        obj.chosenOptionIndex > 16
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "resolveArbitrationProposal requires valid proposalId >= 1 and chosenOptionIndex 0..16",
        };
      }
      return {
        ok: true,
        command: {
          kind: "resolveArbitrationProposal",
          proposalId: obj.proposalId,
          chosenOptionIndex: obj.chosenOptionIndex,
        },
      };
    case "setPersonalAttentionZoom":
      if (
        keys.length !== 3 ||
        (obj.targetMandateId !== null &&
          (typeof obj.targetMandateId !== "number" ||
            !Number.isInteger(obj.targetMandateId) ||
            obj.targetMandateId < 1)) ||
        typeof obj.share01 !== "number" ||
        !Number.isFinite(obj.share01) ||
        obj.share01 < 0 ||
        obj.share01 > 1
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "setPersonalAttentionZoom requires targetMandateId (null or >= 1) and share01 in 0..1",
        };
      }
      return {
        ok: true,
        command: {
          kind: "setPersonalAttentionZoom",
          targetMandateId: obj.targetMandateId,
          share01: obj.share01,
        },
      };
    case "relocateHq":
      if (
        keys.length !== 2 ||
        typeof obj.destinationCell !== "number" ||
        !Number.isInteger(obj.destinationCell) ||
        obj.destinationCell < 0
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "relocateHq requires non-negative integer destinationCell",
        };
      }
      return {
        ok: true,
        command: {
          kind: "relocateHq",
          destinationCell: obj.destinationCell,
        },
      };
    case "updateNotificationRule":
      if (
        keys.length !== 3 ||
        typeof obj.eventClassKey !== "string" ||
        obj.eventClassKey.length === 0 ||
        obj.eventClassKey.length > 64 ||
        typeof obj.behavior !== "string" ||
        !ALLOWED_NOTIFICATION_BEHAVIORS.has(obj.behavior)
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "updateNotificationRule requires valid eventClassKey and behavior",
        };
      }
      return {
        ok: true,
        command: {
          kind: "updateNotificationRule",
          eventClassKey: obj.eventClassKey,
          behavior: obj.behavior,
        },
      };
    default:
      return { ok: false, errorCode: "UNAUTHORIZED_COMMAND", message: `Unknown command kind '${String(obj.kind)}'` };
  }
}
