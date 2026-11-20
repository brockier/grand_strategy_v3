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
    }
  | {
      readonly kind: "switchGovernanceModel";
      readonly modelKey: string;
    }
  | {
      readonly kind: "enactEconomicPolicy";
      readonly policyKey: string;
      readonly rateOverride01: number | null;
    }
  | {
      readonly kind: "charterPrivateCarriers";
      readonly targetSettlement: number;
      readonly count: number;
    }
  | {
      readonly kind: "setInnovationMode";
      readonly settlement: number;
      readonly modeKey: string;
    }
  | {
      readonly kind: "issueTheaterDirective";
      readonly theaterId: number;
      readonly commander: number;
      readonly objectiveCell: number;
      readonly supplyPriority01: number;
    }
  | {
      readonly kind: "issueOperationalBattlePlan";
      readonly formationId: number;
      readonly targetCell: number;
      readonly doctrineKey: string;
      readonly noRetreat: boolean;
    }
  | { readonly kind: "acknowledgeSuccession" }
  | {
      readonly kind: "constructDam";
      readonly targetCell: number;
      readonly ruleCurve: string;
    }
  | {
      readonly kind: "setDamRuleCurve";
      readonly targetCell: number;
      readonly ruleCurve: string;
    };

export type CommandParseResult =
  | { readonly ok: true; readonly command: ValidatedCommand }
  | { readonly ok: false; readonly errorCode: string; readonly message: string };

const ALLOWED_DAM_RULE_CURVES = new Set<string>([
  "balanced_power",
  "flood_control",
  "irrigation_supply",
  "BalancedPower",
  "FloodControl",
  "IrrigationSupply",
]);

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
    case "switchGovernanceModel":
      if (
        keys.length !== 2 ||
        typeof obj.modelKey !== "string" ||
        obj.modelKey.length === 0 ||
        obj.modelKey.length > 64
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "switchGovernanceModel requires non-empty string 'modelKey' (1..64)",
        };
      }
      return {
        ok: true,
        command: {
          kind: "switchGovernanceModel",
          modelKey: obj.modelKey,
        },
      };
    case "enactEconomicPolicy": {
      const allowedKeys = keys.every((k) => k === "kind" || k === "policyKey" || k === "rateOverride01");
      if (
        !allowedKeys ||
        (keys.length !== 2 && keys.length !== 3) ||
        typeof obj.policyKey !== "string" ||
        obj.policyKey.length === 0 ||
        obj.policyKey.length > 64
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "enactEconomicPolicy requires non-empty string 'policyKey' and optional 'rateOverride01'",
        };
      }
      if (
        obj.rateOverride01 !== undefined &&
        obj.rateOverride01 !== null &&
        (typeof obj.rateOverride01 !== "number" ||
          !Number.isFinite(obj.rateOverride01) ||
          obj.rateOverride01 < 0 ||
          obj.rateOverride01 > 1)
      ) {
        return {
          ok: false,
          errorCode: "OUT_OF_RANGE",
          message: "enactEconomicPolicy rateOverride01 must be finite 0..1 or null",
        };
      }
      return {
        ok: true,
        command: {
          kind: "enactEconomicPolicy",
          policyKey: obj.policyKey,
          rateOverride01: obj.rateOverride01 === undefined ? null : (obj.rateOverride01 as number | null),
        },
      };
    }
    case "charterPrivateCarriers":
      if (
        keys.length !== 3 ||
        typeof obj.targetSettlement !== "number" ||
        !Number.isInteger(obj.targetSettlement) ||
        obj.targetSettlement < 0 ||
        typeof obj.count !== "number" ||
        !Number.isInteger(obj.count) ||
        obj.count < 1 ||
        obj.count > 1000
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "charterPrivateCarriers requires targetSettlement >= 0 and integer count 1..1000",
        };
      }
      return {
        ok: true,
        command: {
          kind: "charterPrivateCarriers",
          targetSettlement: obj.targetSettlement,
          count: obj.count,
        },
      };
    case "setInnovationMode":
      if (
        keys.length !== 3 ||
        typeof obj.settlement !== "number" ||
        !Number.isInteger(obj.settlement) ||
        obj.settlement < 0 ||
        typeof obj.modeKey !== "string" ||
        obj.modeKey.length === 0 ||
        obj.modeKey.length > 64
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "setInnovationMode requires settlement >= 0 and non-empty string 'modeKey' (1..64)",
        };
      }
      return {
        ok: true,
        command: {
          kind: "setInnovationMode",
          settlement: obj.settlement,
          modeKey: obj.modeKey,
        },
      };
    case "issueTheaterDirective":
      if (
        keys.length !== 5 ||
        typeof obj.theaterId !== "number" ||
        !Number.isInteger(obj.theaterId) ||
        obj.theaterId < 0 ||
        typeof obj.commander !== "number" ||
        !Number.isInteger(obj.commander) ||
        obj.commander < 0 ||
        typeof obj.objectiveCell !== "number" ||
        !Number.isInteger(obj.objectiveCell) ||
        obj.objectiveCell < 0 ||
        typeof obj.supplyPriority01 !== "number" ||
        !Number.isFinite(obj.supplyPriority01) ||
        obj.supplyPriority01 < 0 ||
        obj.supplyPriority01 > 1
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "issueTheaterDirective requires theaterId >= 0, commander >= 0, objectiveCell >= 0, supplyPriority01 0..1",
        };
      }
      return {
        ok: true,
        command: {
          kind: "issueTheaterDirective",
          theaterId: obj.theaterId,
          commander: obj.commander,
          objectiveCell: obj.objectiveCell,
          supplyPriority01: obj.supplyPriority01,
        },
      };
    case "issueOperationalBattlePlan":
      if (
        keys.length !== 5 ||
        typeof obj.formationId !== "number" ||
        !Number.isInteger(obj.formationId) ||
        obj.formationId < 0 ||
        typeof obj.targetCell !== "number" ||
        !Number.isInteger(obj.targetCell) ||
        obj.targetCell < 0 ||
        typeof obj.doctrineKey !== "string" ||
        obj.doctrineKey.length === 0 ||
        obj.doctrineKey.length > 64 ||
        typeof obj.noRetreat !== "boolean"
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "issueOperationalBattlePlan requires formationId >= 0, targetCell >= 0, doctrineKey, boolean noRetreat",
        };
      }
      return {
        ok: true,
        command: {
          kind: "issueOperationalBattlePlan",
          formationId: obj.formationId,
          targetCell: obj.targetCell,
          doctrineKey: obj.doctrineKey,
          noRetreat: obj.noRetreat,
        },
      };
    case "acknowledgeSuccession":
      if (keys.length !== 1) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "acknowledgeSuccession takes no extra fields",
        };
      }
      return {
        ok: true,
        command: { kind: "acknowledgeSuccession" },
      };
    case "constructDam": {
      if (
        (keys.length !== 2 && keys.length !== 3) ||
        typeof obj.targetCell !== "number" ||
        !Number.isInteger(obj.targetCell) ||
        obj.targetCell < 0
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "constructDam requires non-negative integer targetCell and optional valid ruleCurve",
        };
      }
      const curve = obj.ruleCurve === undefined ? "balanced_power" : obj.ruleCurve;
      if (typeof curve !== "string" || !ALLOWED_DAM_RULE_CURVES.has(curve)) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "constructDam ruleCurve must be balanced_power, flood_control, or irrigation_supply",
        };
      }
      return {
        ok: true,
        command: { kind: "constructDam", targetCell: obj.targetCell, ruleCurve: curve },
      };
    }
    case "setDamRuleCurve":
      if (
        keys.length !== 3 ||
        typeof obj.targetCell !== "number" ||
        !Number.isInteger(obj.targetCell) ||
        obj.targetCell < 0 ||
        typeof obj.ruleCurve !== "string" ||
        !ALLOWED_DAM_RULE_CURVES.has(obj.ruleCurve)
      ) {
        return {
          ok: false,
          errorCode: "INVALID_ARGUMENT",
          message: "setDamRuleCurve requires non-negative integer targetCell and valid ruleCurve",
        };
      }
      return {
        ok: true,
        command: { kind: "setDamRuleCurve", targetCell: obj.targetCell, ruleCurve: obj.ruleCurve },
      };
    case "issueTacticalMicroCommand":
      return {
        ok: false,
        errorCode: "ERR_TACTICAL_MICRO_FORBIDDEN",
        message: "Individual tactical unit micro is forbidden; issue operational directives or battle plans.",
      };
    case "buyOrSelectTechTarget":
      return {
        ok: false,
        errorCode: "ERR_BOUGHT_TECH_FORBIDDEN",
        message: "Technology targets cannot be bought or pre-selected.",
      };
    default:
      return { ok: false, errorCode: "UNAUTHORIZED_COMMAND", message: `Unknown command kind '${String(obj.kind)}'` };
  }
}
