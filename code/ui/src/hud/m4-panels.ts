import {
  ArbitrationProposalDto,
  DelegationTreeDto,
  MandateSummaryDto,
  NotificationRuleDto,
  WindowContentPlugin,
  createProvenanceBadgeElement,
  generateGoldbergTopology,
  greatCircleRadians,
  selectClockAndNotifications,
  selectDelegationTree,
  selectHqPresence,
  selectKnownCells,
  selectKnownSettlements,
  selectNotificationRules,
  selectOpenProposals,
} from "../spine";

const OBJECTIVE_OPTIONS: ReadonlyArray<{ readonly key: string; readonly label: string }> = [
  { key: "stabilize_food", label: "Stabilize Food & Rations" },
  { key: "expand_forestry", label: "Expand Forestry & Timber" },
  { key: "dewater_and_mine", label: "Dewater & Extract Ore" },
  { key: "build_road_link", label: "Construct Road & Bridge Link" },
  { key: "survey_frontier", label: "Dispatch Frontier Survey" },
  { key: "audit_stockpile", label: "Audit Warehouses & Ledgers" },
];

const AUTHORITY_TIERS: ReadonlyArray<{ readonly key: string; readonly label: string; readonly attnCost: number }> = [
  { key: "strategic_mandate", label: "Strategic Directive (General Authority)", attnCost: 9 },
  { key: "sovereign_direct", label: "Direct HQ Intervention (Detailed Order)", attnCost: 18 },
  { key: "expeditionary", label: "Expeditionary Charter", attnCost: 12 },
];

const NOTIFICATION_BEHAVIORS: ReadonlyArray<{ readonly key: string; readonly label: string }> = [
  { key: "LogOnly", label: "Log Only" },
  { key: "BannerAlert", label: "Banner Alert" },
  { key: "DispatchPopup", label: "Dispatch Popup" },
  { key: "DispatchPopupAndAutoPause", label: "Dispatch Popup + Auto-Pause" },
];

const FALLBACK_NOTIFICATION_RULES: readonly NotificationRuleDto[] = [
  { eventClassKey: "stockpile_shortage", displayName: "Critical Stockpile Shortage", behavior: "BannerAlert" },
  { eventClassKey: "courier_overdue", displayName: "Overdue Courier or Convoy", behavior: "DispatchPopup" },
  { eventClassKey: "arbitration_proposal", displayName: "Subordinate Arbitration Escalation", behavior: "DispatchPopupAndAutoPause" },
  { eventClassKey: "weather_storm_alert", displayName: "Severe Storm & Flood Hazard", behavior: "BannerAlert" },
  { eventClassKey: "survey_discovery", displayName: "Geological Assay Returned", behavior: "DispatchPopup" },
  { eventClassKey: "mandate_completed", displayName: "Mandate Fulfillment Report", behavior: "LogOnly" },
];

function collectAllMandates(tree: DelegationTreeDto | null): MandateSummaryDto[] {
  if (!tree) return [];
  const byId = new Map<number, MandateSummaryDto>();
  for (const m of tree.activeMandates ?? []) {
    byId.set(m.mandateId, m);
  }
  for (const sub of tree.subordinates) {
    for (const m of sub.assignedMandates ?? []) {
      byId.set(m.mandateId, m);
    }
  }
  return Array.from(byId.values());
}

export function estimateCourierEtaHours(
  subdivision: number,
  fromCellId: number,
  toCellId: number,
  contours: ReadonlyArray<{ readonly cellId: number; readonly estimatedCourierHours: number }>,
): number {
  const exact = contours.find((c) => c.cellId === toCellId);
  if (exact) return Math.round(exact.estimatedCourierHours);
  if (fromCellId === toCellId) return 0;
  const topo = generateGoldbergTopology(Math.max(1, subdivision));
  const a = topo.cells.find((c) => c.cellId === fromCellId);
  const b = topo.cells.find((c) => c.cellId === toCellId);
  if (!a || !b) return 6;
  const rad = greatCircleRadians(a.centerUnit, b.centerUnit);
  return Math.max(2, Math.round(rad * 72));
}

function findEnclosingWindowId(el: HTMLElement): string | null {
  const frame = el.closest<HTMLElement>(".ng-window-frame");
  return frame?.id || null;
}

/**
 * Strategic & Tactical Mandate Composer with real-time Attention Capacity impact and
 * outbound courier travel latency preview (`UI-HUD-002`, `UI-AUD-003`).
 */
export function createMandateComposerWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "mandate_composer",
    defaultTitle: "Issue Sovereign Mandate",
    minWidth: 420,
    minHeight: 320,
    hotkey: "g",
    mountContent(container, _instanceKey, spine) {
      const form = document.createElement("form");
      form.className = "ng-responsive-grid";

      const fieldset = document.createElement("fieldset");
      fieldset.className = "ng-form-fieldset";
      const legend = document.createElement("legend");
      legend.textContent = "Mandate Directive & Delegation Scope";

      const grid = document.createElement("div");
      grid.className = "ng-form-grid";

      const objWrap = document.createElement("div");
      const objLbl = document.createElement("label");
      objLbl.htmlFor = "mandate-objective";
      objLbl.textContent = "Outcome Goal";
      const objSelect = document.createElement("select");
      objSelect.id = "mandate-objective";
      objSelect.className = "ng-text-input";
      for (const opt of OBJECTIVE_OPTIONS) {
        const o = document.createElement("option");
        o.value = opt.key;
        o.textContent = opt.label;
        objSelect.appendChild(o);
      }
      objWrap.append(objLbl, objSelect);

      const cellWrap = document.createElement("div");
      const cellLbl = document.createElement("label");
      cellLbl.htmlFor = "mandate-target-cell";
      cellLbl.textContent = "Target Cell / Settlement";
      const cellSelect = document.createElement("select");
      cellSelect.id = "mandate-target-cell";
      cellSelect.className = "ng-text-input";
      cellWrap.append(cellLbl, cellSelect);

      const subWrap = document.createElement("div");
      const subLbl = document.createElement("label");
      subLbl.htmlFor = "mandate-assignee";
      subLbl.textContent = "Responsible Official";
      const subSelect = document.createElement("select");
      subSelect.id = "mandate-assignee";
      subSelect.className = "ng-text-input";
      subWrap.append(subLbl, subSelect);

      const tierWrap = document.createElement("div");
      const tierLbl = document.createElement("label");
      tierLbl.htmlFor = "mandate-authority";
      tierLbl.textContent = "Command Posture";
      const tierSelect = document.createElement("select");
      tierSelect.id = "mandate-authority";
      tierSelect.className = "ng-text-input";
      for (const tier of AUTHORITY_TIERS) {
        const o = document.createElement("option");
        o.value = tier.key;
        o.textContent = tier.label;
        tierSelect.appendChild(o);
      }
      tierWrap.append(tierLbl, tierSelect);

      const prioWrap = document.createElement("div");
      const prioLbl = document.createElement("label");
      prioLbl.htmlFor = "mandate-priority";
      prioLbl.textContent = "Priority Weight (1-10)";
      const prioInput = document.createElement("input");
      prioInput.id = "mandate-priority";
      prioInput.type = "range";
      prioInput.className = "ng-range-slider";
      prioInput.min = "1";
      prioInput.max = "10";
      prioInput.step = "1";
      prioInput.value = "5";
      prioInput.style.setProperty("--control-value", "5");
      const prioOut = document.createElement("output");
      prioOut.htmlFor = "mandate-priority";
      prioOut.textContent = "5";
      prioWrap.append(prioLbl, prioInput, prioOut);

      grid.append(objWrap, cellWrap, subWrap, tierWrap, prioWrap);
      fieldset.append(legend, grid);

      const previewOut = document.createElement("output");
      previewOut.className = "ng-impact-output";
      previewOut.setAttribute("data-mandate-preview", "true");

      const submitBtn = document.createElement("button");
      submitBtn.type = "submit";
      submitBtn.className = "ng-btn ng-btn-primary";
      submitBtn.textContent = "Seal & Dispatch Mandate";

      const populateSelectorsAndPreview = (preserveSelection: boolean) => {
        const snap = spine.store.getSnapshot();
        const prevCell = preserveSelection ? cellSelect.value : "";
        const prevSub = preserveSelection ? subSelect.value : "";

        const settlements = selectKnownSettlements(spine.store);
        const cells = selectKnownCells(spine.store);
        const targetOpts: Array<{ readonly value: string; readonly label: string }> = [];
        const seenCellIds = new Set<number>();

        for (const s of settlements) {
          seenCellIds.add(s.cellId);
          targetOpts.push({
            value: String(s.cellId),
            label: `${s.settlementName} (Cell #${s.cellId})${s.isHq ? " [HQ]" : ""}`,
          });
        }
        for (const c of cells) {
          if (c.tier !== "unexplored" && !c.isOcean && !seenCellIds.has(c.cellId) && targetOpts.length < 24) {
            seenCellIds.add(c.cellId);
            targetOpts.push({
              value: String(c.cellId),
              label: `Cell #${c.cellId} (${c.tier})`,
            });
          }
        }
        if (targetOpts.length === 0) {
          targetOpts.push({ value: "0", label: "Haven Origin (Cell #0)" });
        }

        cellSelect.replaceChildren(
          ...targetOpts.map((opt) => {
            const o = document.createElement("option");
            o.value = opt.value;
            o.textContent = opt.label;
            return o;
          }),
        );
        if (prevCell && targetOpts.some((t) => t.value === prevCell)) {
          cellSelect.value = prevCell;
        }

        const tree = selectDelegationTree(spine.store);
        const subs = tree?.subordinates ?? [];
        subSelect.replaceChildren(
          ...subs.map((sub) => {
            const o = document.createElement("option");
            o.value = String(sub.characterId);
            const activeCount = sub.assignedMandates?.length ?? 0;
            o.textContent = `${sub.displayName} (${sub.roleTitle}, ${Math.round(sub.competence * 100)}%, ${activeCount} active)`;
            return o;
          }),
        );
        if (subs.length === 0) {
          const fallback = document.createElement("option");
          fallback.value = "1";
          fallback.textContent = "Quartermaster Corps (#1)";
          subSelect.appendChild(fallback);
        } else if (prevSub && subs.some((s) => String(s.characterId) === prevSub)) {
          subSelect.value = prevSub;
        }

        updateImpactPreview(snap?.globeTopology.subdivision ?? 3);
      };

      const updateImpactPreview = (subdivision = spine.store.getSnapshot()?.globeTopology.subdivision ?? 3) => {
        const tree = selectDelegationTree(spine.store);
        const hq = selectHqPresence(spine.store);
        const selectedTier = AUTHORITY_TIERS.find((t) => t.key === tierSelect.value) ?? AUTHORITY_TIERS[0];
        const targetCell = Number(cellSelect.value) || 0;
        const assigneeId = Number(subSelect.value) || 0;
        const assignee = tree?.subordinates.find((s) => s.characterId === assigneeId);

        const capacity = tree?.capacityAttention ?? 100;
        const curUsed = tree?.usedAttention ?? 0;
        const projectedUsed = curUsed + selectedTier.attnCost;
        const existingForSub = assignee?.assignedMandates?.length ?? 0;
        const isOverloaded = projectedUsed > capacity || existingForSub >= 2;
        const etaHours = estimateCourierEtaHours(
          subdivision,
          hq?.cellId ?? 0,
          targetCell,
          hq?.communicationContours ?? [],
        );

        prioOut.textContent = prioInput.value;
        prioInput.style.setProperty("--control-value", prioInput.value);
        previewOut.setAttribute("data-overload-warning", isOverloaded ? "true" : "false");
        previewOut.textContent = isOverloaded
          ? `Attention Overload Warning: Projected load ${projectedUsed.toFixed(1)} / ${capacity.toFixed(1)} (${existingForSub} existing tasks on official). Courier ETA: ${etaHours}h.`
          : `Projected Attention: ${projectedUsed.toFixed(1)} / ${capacity.toFixed(1)}. Outbound Courier ETA: ${etaHours}h.`;
      };

      prioInput.addEventListener("input", () => updateImpactPreview());
      objSelect.addEventListener("change", () => updateImpactPreview());
      cellSelect.addEventListener("change", () => updateImpactPreview());
      subSelect.addEventListener("change", () => updateImpactPreview());
      tierSelect.addEventListener("change", () => updateImpactPreview());

      form.addEventListener("submit", (ev) => {
        ev.preventDefault();
        const assignee = Math.max(0, Math.floor(Number(subSelect.value) || 0));
        const targetCell = Math.max(0, Math.floor(Number(cellSelect.value) || 0));
        const priorityWeight = Math.max(1, Math.min(10, Math.floor(Number(prioInput.value) || 5)));

        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: {
            kind: "issueMandate",
            assignee,
            objectiveKind: objSelect.value || "stabilize_food",
            targetCell,
            priorityWeight,
            authorityTier: tierSelect.value || "strategic_mandate",
          },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "mandate_seal" });
      });

      populateSelectorsAndPreview(false);
      form.append(fieldset, previewOut, submitBtn);
      container.replaceChildren(form);

      const unsub = spine.store.subscribe(() => populateSelectorsAndPreview(true));
      return () => {
        unsub();
      };
    },
  };
}

/**
 * Escalated Subordinate Conflict & Bottleneck Arbitration Window (`UI-HUD-003`, `UI-AUD-003`).
 */
export function createArbitrationWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "arbitration_panel",
    defaultTitle: "Arbitration & Escalations",
    minWidth: 440,
    minHeight: 300,
    hotkey: "b",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const selectedByProposal = new Map<number, number>();

      const render = () => {
        const proposals = selectOpenProposals(spine.store);
        const hq = selectHqPresence(spine.store);
        const clk = selectClockAndNotifications(spine.store);
        const currentTick = clk?.tick ?? 0;

        if (proposals.length === 0) {
          const empty = document.createElement("p");
          empty.textContent = "No escalated subordinate arbitration proposals await resolution.";
          root.replaceChildren(empty);
          return;
        }

        const cards: HTMLElement[] = [];
        for (const prop of proposals as readonly ArbitrationProposalDto[]) {
          const card = document.createElement("article");
          card.className = "ng-proposal-card";
          card.dataset.proposalId = String(prop.proposalId);

          const hdr = document.createElement("div");
          hdr.className = "ng-hud-cluster";
          const title = document.createElement("strong");
          title.textContent = prop.title;

          const badge = createProvenanceBadgeElement(document, {
            currentTick,
            observedTick: prop.createdTick,
            confidence01: 0.88,
            provenanceLabel: `Settlement #${prop.settlementId}`,
            reporterName: prop.raisedByName,
          });
          hdr.append(title, badge);

          const summary = document.createElement("p");
          const eta = estimateCourierEtaHours(
            spine.store.getSnapshot()?.globeTopology.subdivision ?? 3,
            hq?.cellId ?? 0,
            prop.originCellId,
            hq?.communicationContours ?? [],
          );
          summary.textContent = `${prop.summary} · Raised by ${prop.raisedByName} · Deadline Tick ${prop.deadlineTick} · Courier Turnaround ~${eta}h`;

          const optionsField = document.createElement("fieldset");
          optionsField.className = "ng-form-fieldset";
          const optLegend = document.createElement("legend");
          optLegend.textContent = "Select Binding Resolution";
          optionsField.appendChild(optLegend);

          const currentChoice = selectedByProposal.get(prop.proposalId) ?? (prop.options[0]?.index ?? 0);
          selectedByProposal.set(prop.proposalId, currentChoice);

          for (const opt of prop.options) {
            const label = document.createElement("label");
            label.className = "ng-ledger-row";
            const radio = document.createElement("input");
            radio.type = "radio";
            radio.name = `arb-choice-${prop.proposalId}`;
            radio.value = String(opt.index);
            radio.checked = opt.index === currentChoice;
            radio.addEventListener("change", () => {
              selectedByProposal.set(prop.proposalId, opt.index);
            });
            const descSpan = document.createElement("span");
            descSpan.textContent = `${opt.label}: ${opt.description} (Attention ${opt.attentionCost.toFixed(1)})`;
            label.append(radio, descSpan);
            optionsField.appendChild(label);
          }

          const resolveBtn = document.createElement("button");
          resolveBtn.type = "button";
          resolveBtn.className = "ng-btn ng-btn-primary";
          resolveBtn.setAttribute("data-resolve-proposal", String(prop.proposalId));
          resolveBtn.textContent = "Stamp & Dispatch Resolution";
          resolveBtn.addEventListener("click", () => {
            const chosenOptionIndex = selectedByProposal.get(prop.proposalId) ?? 0;
            const remainingBefore = selectOpenProposals(spine.store).length;
            spine.bus.emit({
              kind: "DispatchSimCommand",
              command: {
                kind: "resolveArbitrationProposal",
                proposalId: prop.proposalId,
                chosenOptionIndex,
              },
            });
            spine.bus.emit({ kind: "PlayUiFoley", cueId: "mandate_seal" });
            if (remainingBefore <= 1) {
              const winId = findEnclosingWindowId(root);
              if (winId) {
                spine.bus.emit({ kind: "CloseWindow", windowId: winId });
              }
            }
          });

          card.append(hdr, summary, optionsField, resolveBtn);
          cards.push(card);
        }

        root.replaceChildren(...cards);
      };

      render();
      container.replaceChildren(root);
      const unsub = spine.store.subscribe(render);
      return () => {
        unsub();
      };
    },
  };
}

/**
 * Sovereign Personal Attention Zoom-In Allocation Panel with live dilution warnings
 * (`UI-HUD-004`).
 */
export function createAttentionZoomWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "attention_zoom",
    defaultTitle: "Personal Attention Focus",
    minWidth: 400,
    minHeight: 280,
    hotkey: "z",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const headerOut = document.createElement("output");
      headerOut.className = "ng-impact-output";

      const shareLabel = document.createElement("label");
      shareLabel.htmlFor = "attention-zoom-share";
      shareLabel.textContent = "Focused Attention Reserve Share (%)";

      const shareRange = document.createElement("input");
      shareRange.id = "attention-zoom-share";
      shareRange.type = "range";
      shareRange.className = "ng-range-slider";
      shareRange.min = "10";
      shareRange.max = "85";
      shareRange.step = "5";
      shareRange.value = "40";
      shareRange.style.setProperty("--control-value", "40");

      const listEl = document.createElement("div");
      listEl.className = "ng-org-grid";

      const dilutionOut = document.createElement("output");
      dilutionOut.className = "ng-impact-output";
      dilutionOut.setAttribute("data-dilution-warning", "true");

      const render = () => {
        const tree = selectDelegationTree(spine.store);
        const mandates = collectAllMandates(tree);
        const focusedId = tree?.focusedMandateId ?? null;
        const sharePct = Math.round((tree?.focusedAttentionShare ?? Number(shareRange.value) / 100) * 100);
        if (tree?.focusedAttentionShare !== undefined && tree.focusedAttentionShare > 0) {
          shareRange.value = String(Math.max(10, Math.min(85, sharePct)));
        }
        shareRange.style.setProperty("--control-value", shareRange.value);

        const cap = tree?.capacityAttention ?? 100;
        const used = tree?.usedAttention ?? 0;
        headerOut.textContent = `Sovereign Attention: ${used.toFixed(1)} / ${cap.toFixed(1)} · Focused Mandate: ${focusedId !== null ? `#${focusedId} (${shareRange.value}% reserve)` : "Distributed Equally"}`;

        if (focusedId !== null && mandates.length > 1) {
          const lossPct = Math.round((Number(shareRange.value) / Math.max(1, mandates.length - 1)) * 0.65);
          dilutionOut.textContent = `Oversight Dilution Warning: Concentrating ${shareRange.value}% on Mandate #${focusedId} reduces supervision across ${mandates.length - 1} other branch(es) by ~${lossPct}%.`;
        } else {
          dilutionOut.textContent = "Oversight distributed proportionally across active mandates.";
        }

        if (mandates.length === 0) {
          const empty = document.createElement("p");
          empty.textContent = "No active mandates to allocate personal attention toward.";
          listEl.replaceChildren(empty);
          return;
        }

        const cards: HTMLElement[] = [];
        for (const m of mandates) {
          const card = document.createElement("article");
          card.className = "ng-subordinate-card";
          const isFocused = focusedId === m.mandateId;
          card.dataset.focusedMandate = isFocused ? "true" : "false";

          const title = document.createElement("strong");
          title.textContent = `#${m.mandateId}: ${m.summary}`;
          const stat = document.createElement("p");
          const dilutedElsewhere = focusedId !== null && !isFocused;
          stat.textContent = `Eff ${(m.realizedEfficiency * 100).toFixed(0)}% · Progress ${(m.progress01 * 100).toFixed(0)}%${dilutedElsewhere ? " · DILUTED BY ZOOM" : ""}`;

          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = isFocused ? "ng-btn ng-btn-primary" : "ng-btn";
          btn.setAttribute("data-focus-mandate", String(m.mandateId));
          btn.setAttribute("aria-pressed", isFocused ? "true" : "false");
          btn.textContent = isFocused ? "Release Focus" : "Zoom In Personal Oversight";
          btn.addEventListener("click", () => {
            const share01 = isFocused ? 0 : Number(shareRange.value) / 100;
            spine.bus.emit({
              kind: "DispatchSimCommand",
              command: {
                kind: "setPersonalAttentionZoom",
                targetMandateId: isFocused ? null : m.mandateId,
                share01,
              },
            });
            spine.bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
          });

          card.append(title, stat, btn);
          cards.push(card);
        }
        listEl.replaceChildren(...cards);
      };

      shareRange.addEventListener("input", () => {
        shareRange.style.setProperty("--control-value", shareRange.value);
        const focusedId = selectDelegationTree(spine.store)?.focusedMandateId ?? null;
        if (focusedId !== null) {
          spine.bus.emit({
            kind: "DispatchSimCommand",
            command: {
              kind: "setPersonalAttentionZoom",
              targetMandateId: focusedId,
              share01: Number(shareRange.value) / 100,
            },
          });
        }
        render();
      });

      render();
      root.append(headerOut, shareLabel, shareRange, listEl, dilutionOut);
      container.replaceChildren(root);
      const unsub = spine.store.subscribe(render);
      return () => {
        unsub();
      };
    },
  };
}

/**
 * Physical HQ Relocation Planner previewing route highlights on the globe and
 * comparative before-versus-after dispatch latency (`UI-HUD-005`, `UI-AUD-003`).
 */
export function createHqRelocationPlannerWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "hq_relocation",
    defaultTitle: "Physical HQ Relocation Planner",
    minWidth: 420,
    minHeight: 280,
    hotkey: "h",
    mountContent(container, _instanceKey, spine) {
      const form = document.createElement("form");
      form.className = "ng-responsive-grid";

      const destLbl = document.createElement("label");
      destLbl.htmlFor = "hq-reloc-dest";
      destLbl.textContent = "Target HQ Relocation Post";

      const destSelect = document.createElement("select");
      destSelect.id = "hq-reloc-dest";
      destSelect.className = "ng-text-input";

      const previewOut = document.createElement("output");
      previewOut.className = "ng-impact-output";
      previewOut.setAttribute("data-hq-latency-preview", "true");

      const orderBtn = document.createElement("button");
      orderBtn.type = "submit";
      orderBtn.className = "ng-btn ng-btn-primary";
      orderBtn.textContent = "Order HQ Relocation March";

      const updatePreview = (emitRouteIntent: boolean) => {
        const hq = selectHqPresence(spine.store);
        const previews = hq?.relocationPreviews ?? [];
        const settlements = selectKnownSettlements(spine.store);

        const options: Array<{
          readonly cellId: number;
          readonly label: string;
          readonly travelHours: number;
          readonly projectedHavenLatencyHours: number;
        }> = [];
        const seen = new Set<number>();

        for (const p of previews) {
          if (p.targetCellId === hq?.cellId) continue;
          seen.add(p.targetCellId);
          options.push({
            cellId: p.targetCellId,
            label: p.label,
            travelHours: p.travelHours,
            projectedHavenLatencyHours: p.projectedHavenLatencyHours,
          });
        }
        for (const s of settlements) {
          if (s.cellId === hq?.cellId || seen.has(s.cellId)) continue;
          seen.add(s.cellId);
          options.push({
            cellId: s.cellId,
            label: s.settlementName,
            travelHours: 18,
            projectedHavenLatencyHours: 10,
          });
        }

        const prevVal = destSelect.value;
        destSelect.replaceChildren(
          ...options.map((opt) => {
            const o = document.createElement("option");
            o.value = String(opt.cellId);
            o.textContent = `${opt.label} (Cell #${opt.cellId} · ${opt.travelHours.toFixed(0)}h march)`;
            return o;
          }),
        );
        if (options.length === 0) {
          const fallback = document.createElement("option");
          fallback.value = String(hq?.cellId ?? 0);
          fallback.textContent = `${hq?.settlementName ?? "Haven"} (No remote hubs surveyed)`;
          destSelect.appendChild(fallback);
        } else if (prevVal && options.some((o) => String(o.cellId) === prevVal)) {
          destSelect.value = prevVal;
        }

        const chosenCell = Number(destSelect.value) || (hq?.cellId ?? 0);
        const chosenOpt = options.find((o) => o.cellId === chosenCell);
        const baseHavenLatency = estimateCourierEtaHours(
          spine.store.getSnapshot()?.globeTopology.subdivision ?? 3,
          hq?.cellId ?? 0,
          settlements[0]?.cellId ?? 0,
          hq?.communicationContours ?? [],
        );

        if (hq?.inTransit) {
          previewOut.textContent = `HQ Currently in Transit toward Cell #${hq.destinationCellId ?? chosenCell} (ETA Tick ${hq.etaTick ?? 0}).`;
        } else if (chosenOpt) {
          const days = (chosenOpt.travelHours / 24).toFixed(1);
          const delta = chosenOpt.projectedHavenLatencyHours - baseHavenLatency;
          const sign = delta >= 0 ? `+${delta.toFixed(1)}h` : `${delta.toFixed(1)}h`;
          previewOut.textContent = `Route to ${chosenOpt.label} (#${chosenOpt.cellId}): Travel ${chosenOpt.travelHours.toFixed(0)}h (${days}d) · Haven Courier Latency ${baseHavenLatency.toFixed(1)}h -> ${chosenOpt.projectedHavenLatencyHours.toFixed(1)}h (${sign}).`;
        } else {
          previewOut.textContent = `Stationed at ${hq?.settlementName ?? "Primary HQ"} (#${hq?.cellId ?? 0}).`;
        }

        if (emitRouteIntent && chosenOpt) {
          spine.bus.emit({ kind: "PreviewHqLatencyRoute", destinationCellId: chosenOpt.cellId });
        }
      };

      destSelect.addEventListener("change", () => updatePreview(true));

      form.addEventListener("submit", (ev) => {
        ev.preventDefault();
        const destinationCell = Math.max(0, Math.floor(Number(destSelect.value) || 0));
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: { kind: "relocateHq", destinationCell },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "mandate_seal" });
      });

      updatePreview(true);
      form.append(destLbl, destSelect, previewOut, orderBtn);
      container.replaceChildren(form);

      const unsub = spine.store.subscribe(() => updatePreview(false));
      return () => {
        unsub();
        spine.bus.emit({ kind: "PreviewHqLatencyRoute", destinationCellId: null });
      };
    },
  };
}

/**
 * Per-Event Notification & Auto-Pause Configuration Matrix persisted across restarts
 * (`UI-HUD-006`, `UI-AUD-003`).
 */
export function createNotificationMatrixWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "notification_matrix",
    defaultTitle: "Notification & Auto-Pause Rules",
    minWidth: 460,
    minHeight: 280,
    hotkey: "n",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const table = document.createElement("table");
      table.className = "ng-ledger-table";

      const thead = document.createElement("thead");
      const headTr = document.createElement("tr");
      const h1 = document.createElement("th");
      h1.scope = "col";
      h1.textContent = "Event Class";
      const h2 = document.createElement("th");
      h2.scope = "col";
      h2.textContent = "Delivery & Pause Behavior";
      headTr.append(h1, h2);
      thead.appendChild(headTr);

      const tbody = document.createElement("tbody");
      table.append(thead, tbody);

      const statusOut = document.createElement("output");
      statusOut.className = "ng-impact-output";
      statusOut.textContent = "Adjust any event class delivery policy; changes persist across restarts.";

      const persistRule = (eventClassKey: string, behavior: string) => {
        if (!spine.writePreferences) return;
        void (async () => {
          try {
            const existing = spine.readPreferences
              ? await spine.readPreferences()
              : {
                  audioMixer: {
                    masterVolume: 0.85,
                    musicVolume: 0.7,
                    ambienceVolume: 0.8,
                    foleyVolume: 0.75,
                    masterMuted: false,
                    musicMuted: false,
                    ambienceMuted: false,
                    foleyMuted: false,
                  },
                  namedLayoutPresets: {},
                  notificationRules: {},
                };
            await spine.writePreferences!({
              ...existing,
              notificationRules: {
                ...(existing.notificationRules ?? {}),
                [eventClassKey]: behavior,
              },
            });
          } catch {
            // Non-fatal preference safeguard
          }
        })();
      };

      const render = () => {
        const snapRules = selectNotificationRules(spine.store);
        const rules = snapRules.length > 0 ? snapRules : FALLBACK_NOTIFICATION_RULES;

        const rows: HTMLTableRowElement[] = [];
        for (const r of rules) {
          const tr = document.createElement("tr");
          const tdName = document.createElement("td");
          const lbl = document.createElement("label");
          lbl.htmlFor = `notif-select-${r.eventClassKey}`;
          lbl.textContent = r.displayName;
          tdName.appendChild(lbl);

          const tdPolicy = document.createElement("td");
          const sel = document.createElement("select");
          sel.id = `notif-select-${r.eventClassKey}`;
          sel.className = "ng-text-input";
          sel.setAttribute("data-notif-rule", r.eventClassKey);
          for (const opt of NOTIFICATION_BEHAVIORS) {
            const o = document.createElement("option");
            o.value = opt.key;
            o.textContent = opt.label;
            sel.appendChild(o);
          }
          sel.value = r.behavior;
          sel.addEventListener("change", () => {
            const nextBehavior = sel.value;
            spine.bus.emit({
              kind: "DispatchSimCommand",
              command: {
                kind: "updateNotificationRule",
                eventClassKey: r.eventClassKey,
                behavior: nextBehavior,
              },
            });
            persistRule(r.eventClassKey, nextBehavior);
            statusOut.textContent = `Updated ${r.displayName} -> ${nextBehavior}`;
            spine.bus.emit({ kind: "PlayUiFoley", cueId: "clock_step" });
          });

          tdPolicy.appendChild(sel);
          tr.append(tdName, tdPolicy);
          rows.push(tr);
        }
        tbody.replaceChildren(...rows);
      };

      render();
      root.append(table, statusOut);
      container.replaceChildren(root);

      const unsub = spine.store.subscribe(render);
      return () => {
        unsub();
      };
    },
  };
}
