export type EpistemicBadgeTier = "direct" | "courier" | "stale";

export interface ProvenanceBadgeInput {
  readonly currentTick: number;
  readonly observedTick: number;
  readonly confidence01: number;
  readonly provenanceLabel: string;
  readonly reporterName?: string;
  readonly isLiveSight?: boolean;
}

export interface ProvenanceBadgeDescription {
  readonly tier: EpistemicBadgeTier;
  readonly ageHours: number;
  readonly confidencePct: number;
  readonly badgeText: string;
  readonly accessibleTitle: string;
}

/**
 * Classifies epistemic currency and uncertainty for any datum reported by `player-view` (`UI-DAT-005`).
 * Direct observation (`<= 2h` at high confidence) yields `"direct"`, active courier relays yield
 * `"courier"`, and aged or low-confidence estimates yield `"stale"`.
 */
export function describeProvenanceBadge(input: ProvenanceBadgeInput): ProvenanceBadgeDescription {
  const safeCurrent = Math.max(0, Math.floor(input.currentTick));
  const safeObserved = Math.max(0, Math.min(safeCurrent, Math.floor(input.observedTick)));
  const ageHours = Math.max(0, safeCurrent - safeObserved);
  const clampedConf = Math.max(0, Math.min(1, Number.isFinite(input.confidence01) ? input.confidence01 : 0));
  const confidencePct = Math.round(clampedConf * 100);
  const label = input.provenanceLabel.trim().length > 0 ? input.provenanceLabel.trim() : "Unattributed report";

  let tier: EpistemicBadgeTier;
  let stageWord: string;

  if (input.isLiveSight || (ageHours <= 2 && clampedConf >= 0.85)) {
    tier = "direct";
    stageWord = "Direct";
  } else if (ageHours <= 24 && clampedConf >= 0.5) {
    tier = "courier";
    stageWord = "Courier";
  } else {
    tier = "stale";
    stageWord = clampedConf < 0.45 ? "Rumored" : "Stale";
  }

  const reporterSuffix = input.reporterName && input.reporterName.trim().length > 0
    ? ` via ${input.reporterName.trim()}`
    : "";
  const badgeText = `${stageWord} ${ageHours}h · ${confidencePct}%`;
  const accessibleTitle = `${label}${reporterSuffix} — ${stageWord}, ${ageHours}h old (Tick ${safeObserved}), ${confidencePct}% confidence`;

  return {
    tier,
    ageHours,
    confidencePct,
    badgeText,
    accessibleTitle,
  };
}

/**
 * Renders a skin-styled epistemic provenance badge element bound exclusively via CSS data attributes
 * (`UI-DAT-005`) without injecting inline color literals.
 */
export function createProvenanceBadgeElement(
  doc: Document,
  input: ProvenanceBadgeInput,
): HTMLSpanElement {
  const desc = describeProvenanceBadge(input);
  const badge = doc.createElement("span");
  badge.className = "ng-prov-badge";
  badge.setAttribute("data-tier", desc.tier);
  badge.setAttribute("data-age-hours", String(desc.ageHours));
  badge.setAttribute("data-confidence-pct", String(desc.confidencePct));
  badge.setAttribute("title", desc.accessibleTitle);
  badge.setAttribute("aria-label", desc.accessibleTitle);
  badge.textContent = desc.badgeText;
  return badge;
}
