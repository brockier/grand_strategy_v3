import {
  DiegeticStoreAccess,
  UiIntentBus,
  WindowContentPlugin,
  createAccessibleTabGroup,
  createProvenanceBadgeElement,
  selectDelegationTree,
  selectGovernance,
  selectHqPresence,
  selectLedgerRows,
  selectNonStateEconomy,
  selectSuccession,
  selectWarfare,
  syncSelectOptions,
} from "../spine";
import { estimateCourierEtaHours } from "./m4-panels";

/**
 * Churchill Theater Delegation vs Hitler Direct Operational Command Window (`UI-HUD-007.1`).
 * Previews Attention cost and HQ courier travel delay while omitting any per-unit tactical micro controls.
 */
export function createWarfareCommandWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "warfare_command",
    defaultTitle: "Operational Warfare Command",
    minWidth: 500,
    minHeight: 380,
    hotkey: "f",
    mountContent(container, _instanceKey, spine) {
      const tabGroup = createAccessibleTabGroup(document, {
        prefix: "warfare-command",
        ariaLabel: "Operational Command Modes",
        tabs: [
          { key: "churchill_theater", label: "Theater Directives (Delegated)" },
          { key: "hitler_operational", label: "Operational Battle Plans (Direct)" },
        ],
      });

      const churchillHost = tabGroup.panelInnerByKey.get("churchill_theater")!;
      const hitlerHost = tabGroup.panelInnerByKey.get("hitler_operational")!;

      // Churchill mode panel
      const churchillPreview = document.createElement("div");
      churchillPreview.className = "ng-autopause-banner";
      churchillPreview.setAttribute("data-warfare-attention-preview", "churchill");
      churchillPreview.textContent =
        "Attention Load: Low (1.5 slices) — Theater commander retains operational initiative between courier reports.";

      const theaterListBox = document.createElement("div");
      const theaterForm = document.createElement("div");
      theaterForm.className = "ng-responsive-grid";

      const theaterIdInput = document.createElement("input");
      theaterIdInput.type = "number";
      theaterIdInput.className = "ng-text-input";
      theaterIdInput.min = "1";
      theaterIdInput.step = "1";
      theaterIdInput.value = "1";
      theaterIdInput.setAttribute("data-theater-id-input", "true");
      theaterIdInput.setAttribute("aria-label", "Theater identifier");

      const commanderSelect = document.createElement("select");
      commanderSelect.className = "ng-select";
      commanderSelect.setAttribute("data-theater-commander-select", "true");
      commanderSelect.setAttribute("aria-label", "Assigned theater commander");

      const objectiveInput = document.createElement("input");
      objectiveInput.type = "number";
      objectiveInput.className = "ng-text-input";
      objectiveInput.min = "0";
      objectiveInput.step = "1";
      objectiveInput.value = "1";
      objectiveInput.setAttribute("data-theater-objective-input", "true");
      objectiveInput.setAttribute("aria-label", "Theater objective cell");

      const supplyInput = document.createElement("input");
      supplyInput.type = "number";
      supplyInput.className = "ng-text-input";
      supplyInput.min = "0";
      supplyInput.max = "1";
      supplyInput.step = "0.05";
      supplyInput.value = "0.75";
      supplyInput.setAttribute("data-theater-supply-input", "true");
      supplyInput.setAttribute("aria-label", "Theater supply priority from 0 to 1");

      const theaterEtaBox = document.createElement("div");
      theaterEtaBox.className = "ng-hud-stat";

      const theaterSubmitBtn = document.createElement("button");
      theaterSubmitBtn.type = "button";
      theaterSubmitBtn.className = "ng-btn ng-btn-primary";
      theaterSubmitBtn.setAttribute("data-theater-submit", "true");
      theaterSubmitBtn.textContent = "Issue Theater Directive";

      const theaterStatus = document.createElement("div");
      theaterStatus.className = "ng-hud-stat";
      theaterStatus.setAttribute("role", "status");

      theaterForm.append(
        theaterIdInput,
        commanderSelect,
        objectiveInput,
        supplyInput,
        theaterEtaBox,
        theaterSubmitBtn,
        theaterStatus,
      );
      churchillHost.append(churchillPreview, theaterListBox, theaterForm);

      // Hitler operational mode panel
      const hitlerPreview = document.createElement("div");
      hitlerPreview.className = "ng-autopause-banner";
      hitlerPreview.setAttribute("data-warfare-attention-preview", "hitler");
      hitlerPreview.textContent =
        "Attention Load: High (6.0 slices) — Direct HQ phase-line intervention dilutes ministry oversight across all other branches.";

      const noMicroNotice = document.createElement("div");
      noMicroNotice.className = "ng-hud-stat";
      noMicroNotice.textContent =
        "Strategic Command Rule: Sovereign HQ issues operational phase-line and standing posture directives; individual platoon or squad tactical control cannot be exercised remotely.";

      const formationListBox = document.createElement("div");
      const planForm = document.createElement("div");
      planForm.className = "ng-responsive-grid";

      const planFormationSelect = document.createElement("select");
      planFormationSelect.className = "ng-select";
      planFormationSelect.setAttribute("data-plan-formation-select", "true");
      planFormationSelect.setAttribute("aria-label", "Target military formation");

      const planTargetInput = document.createElement("input");
      planTargetInput.type = "number";
      planTargetInput.className = "ng-text-input";
      planTargetInput.min = "0";
      planTargetInput.step = "1";
      planTargetInput.value = "1";
      planTargetInput.setAttribute("data-plan-target-input", "true");
      planTargetInput.setAttribute("aria-label", "Phase-line target cell");

      const planDoctrineSelect = document.createElement("select");
      planDoctrineSelect.className = "ng-select";
      planDoctrineSelect.setAttribute("data-plan-doctrine-select", "true");
      planDoctrineSelect.setAttribute("aria-label", "Operational doctrine posture");

      const noRetreatLabel = document.createElement("label");
      noRetreatLabel.className = "ng-hud-stat";
      const noRetreatCheckbox = document.createElement("input");
      noRetreatCheckbox.type = "checkbox";
      noRetreatCheckbox.setAttribute("data-plan-noretreat-checkbox", "true");
      noRetreatLabel.append(noRetreatCheckbox, " Enforce Standing Order: No Retreat");

      const planEtaBox = document.createElement("div");
      planEtaBox.className = "ng-hud-stat";

      const planSubmitBtn = document.createElement("button");
      planSubmitBtn.type = "button";
      planSubmitBtn.className = "ng-btn ng-btn-primary";
      planSubmitBtn.setAttribute("data-plan-submit", "true");
      planSubmitBtn.textContent = "Issue Operational Battle Plan";

      const planStatus = document.createElement("div");
      planStatus.className = "ng-hud-stat";
      planStatus.setAttribute("role", "status");

      planForm.append(
        planFormationSelect,
        planTargetInput,
        planDoctrineSelect,
        noRetreatLabel,
        planEtaBox,
        planSubmitBtn,
        planStatus,
      );
      hitlerHost.append(hitlerPreview, noMicroNotice, formationListBox, planForm);

      const updateEtaPreviews = () => {
        const snap = spine.store.getSnapshot();
        const hq = selectHqPresence(spine.store);
        const sub = snap?.globeTopology.subdivision ?? 3;
        const fromCell = hq?.cellId ?? 0;
        const contours = hq?.communicationContours ?? [];

        const objCell = Number.parseInt(objectiveInput.value || "0", 10) || 0;
        const theaterHours = estimateCourierEtaHours(sub, fromCell, objCell, contours);
        theaterEtaBox.textContent = `Estimated HQ Courier Dispatch Latency to Cell ${objCell}: ${theaterHours} hours`;

        const planCell = Number.parseInt(planTargetInput.value || "0", 10) || 0;
        const planHours = estimateCourierEtaHours(sub, fromCell, planCell, contours);
        planEtaBox.textContent = `Estimated HQ Courier Dispatch Latency to Phase Cell ${planCell}: ${planHours} hours`;
      };

      objectiveInput.addEventListener("input", updateEtaPreviews);
      planTargetInput.addEventListener("input", updateEtaPreviews);

      theaterSubmitBtn.addEventListener("click", () => {
        const theaterId = Math.max(1, Number.parseInt(theaterIdInput.value || "1", 10) || 1);
        const commander = Math.max(0, Number.parseInt(commanderSelect.value || "0", 10) || 0);
        const objectiveCell = Math.max(0, Number.parseInt(objectiveInput.value || "0", 10) || 0);
        const supplyRaw = Number.parseFloat(supplyInput.value || "0.75");
        const supplyPriority01 = Number.isFinite(supplyRaw) ? Math.max(0, Math.min(1, supplyRaw)) : 0.75;
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: {
            kind: "issueTheaterDirective",
            theaterId,
            commander,
            objectiveCell,
            supplyPriority01,
          },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
        theaterStatus.textContent = `Dispatched Theater ${theaterId} directive toward Cell ${objectiveCell}`;
      });

      planSubmitBtn.addEventListener("click", () => {
        const formationId = Math.max(1, Number.parseInt(planFormationSelect.value || "1", 10) || 1);
        const targetCell = Math.max(0, Number.parseInt(planTargetInput.value || "0", 10) || 0);
        const doctrineKey = planDoctrineSelect.value;
        if (!doctrineKey) return;
        const noRetreat = noRetreatCheckbox.checked;
        spine.bus.emit({
          kind: "DispatchSimCommand",
          command: {
            kind: "issueOperationalBattlePlan",
            formationId,
            targetCell,
            doctrineKey,
            noRetreat,
          },
        });
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
        planStatus.textContent = `Dispatched operational battle plan for Formation ${formationId} (No Retreat: ${noRetreat ? "Yes" : "No"})`;
      });

      const render = () => {
        const snap = spine.store.getSnapshot();
        const currentTick = snap?.clockAndNotifications.tick ?? 0;
        const warfare = selectWarfare(spine.store);
        const tree = selectDelegationTree(spine.store);

        const thCards: HTMLElement[] = [];
        for (const th of warfare?.theaters ?? []) {
          const card = document.createElement("div");
          card.className = "ng-subordinate-card";
          card.textContent = `Theater ${th.theaterId} — Commander ${th.commanderName} · Objective Cell ${th.objectiveCellId} · Supply Priority ${Math.round(th.supplyPriority01 * 100)}% · Issued Tick ${th.issuedTick} · Autonomous Withdrawals ${th.autonomousWithdrawals}`;
          thCards.push(card);
        }
        if (thCards.length === 0) {
          const empty = document.createElement("p");
          empty.textContent = "No active military theaters reported.";
          thCards.push(empty);
        }
        theaterListBox.replaceChildren(...thCards);

        const fCards: HTMLElement[] = [];
        for (const form of warfare?.formations ?? []) {
          const card = document.createElement("div");
          card.className = "ng-subordinate-card";
          card.setAttribute("data-formation-id", String(form.formationId));

          const row = document.createElement("div");
          row.className = "ng-ledger-row";
          const title = document.createElement("strong");
          title.textContent = `${form.label} (${form.commanderName}) — Cell ${form.currentCellId} -> Target Cell ${form.targetCellId}`;
          const prov = createProvenanceBadgeElement(document, {
            currentTick,
            observedTick: form.observedTick,
            confidence01: form.confidence01,
            provenanceLabel: form.provenanceLabel,
          });
          row.append(title, prov);

          const meta = document.createElement("div");
          meta.className = "ng-hud-stat";
          meta.textContent = `Personnel ${form.personnelCount} · Cohesion ${Math.round(form.cohesion01 * 100)}% · Munitions ${form.munitionsDays.toFixed(1)}d · Rations ${form.rationsDays.toFixed(1)}d · Doctrine ${form.doctrineKey}`;

          const badges = document.createElement("div");
          badges.className = "ng-hud-cluster";
          if (form.noRetreatOrder) {
            const b = document.createElement("span");
            b.className = "ng-posture-badge";
            b.textContent = "No Retreat";
            badges.appendChild(b);
          }
          if (form.encircled) {
            const b = document.createElement("span");
            b.className = "ng-posture-badge";
            b.textContent = "Encircled";
            badges.appendChild(b);
          }
          if (form.insubordinateWithdrawal) {
            const b = document.createElement("span");
            b.className = "ng-posture-badge";
            b.textContent = "Autonomous Withdrawal";
            badges.appendChild(b);
          }

          const flyBtn = document.createElement("button");
          flyBtn.type = "button";
          flyBtn.className = "ng-btn";
          flyBtn.textContent = `Locate Cell ${form.currentCellId}`;
          flyBtn.addEventListener("click", () => {
            spine.bus.emit({ kind: "FlyGlobeToCell", cellId: form.currentCellId });
          });

          card.append(row, meta, badges, flyBtn);
          fCards.push(card);
        }
        if (fCards.length === 0) {
          const empty = document.createElement("p");
          empty.textContent = "No operational formations reported.";
          fCards.push(empty);
        }
        formationListBox.replaceChildren(...fCards);

        const commanders = new Map<number, string>();
        for (const sub of tree?.subordinates ?? []) {
          commanders.set(sub.characterId, `${sub.displayName} (${sub.roleTitle})`);
        }
        for (const th of warfare?.theaters ?? []) {
          if (!commanders.has(th.commanderId)) {
            commanders.set(th.commanderId, th.commanderName);
          }
        }
        syncSelectOptions(
          commanderSelect,
          Array.from(commanders.entries()).map(([id, label]) => ({
            value: String(id),
            label,
          })),
        );
        syncSelectOptions(
          planFormationSelect,
          (warfare?.formations ?? []).map((f) => ({
            value: String(f.formationId),
            label: `${f.label} (Cell ${f.currentCellId})`,
          })),
        );
        syncSelectOptions(
          planDoctrineSelect,
          (warfare?.availableDoctrines ?? []).map((d) => ({
            value: d.doctrineKey,
            label: d.displayName,
          })),
        );

        updateEtaPreviews();
      };

      render();
      container.replaceChildren(tabGroup.root);
      const unsub = spine.store.subscribe(render);
      return unsub;
    },
  };
}

/**
 * Native modal `<dialog>` for Generational Succession Transitions (`UI-HUD-007.2`).
 * Uses native `showModal()`, `aria-labelledby`, zero redundant ARIA dialog/modal attributes,
 * and cleans `dialog.returnValue` in the `close` listener.
 */
export function mountSuccessionTransitionModal(
  host: HTMLElement,
  store: DiegeticStoreAccess,
  bus: UiIntentBus,
): () => void {
  const dialog = document.createElement("dialog");
  dialog.id = "succession-transition-dialog";
  dialog.className = "ng-modal-dialog ng-succession-dialog";
  dialog.setAttribute("data-chrome-occluder", "true");
  dialog.setAttribute("aria-labelledby", "succession-dialog-title");

  const titleEl = document.createElement("h2");
  titleEl.id = "succession-dialog-title";
  titleEl.textContent = "Generational Succession Transition";

  const summaryPara = document.createElement("p");
  summaryPara.className = "ng-hud-stat";

  const warningBanner = document.createElement("div");
  warningBanner.className = "ng-autopause-banner";
  warningBanner.setAttribute("data-succession-unrecorded-loss", "true");

  const archiveDisclosure = document.createElement("details");
  archiveDisclosure.className = "ng-disclosure";
  archiveDisclosure.open = true;
  const archiveSummary = document.createElement("summary");
  const archiveTitleStrong = document.createElement("strong");
  archiveTitleStrong.textContent = "Inherited Physical Archives & Institutions";
  const archiveCountBadge = document.createElement("span");
  archiveCountBadge.className = "ng-prov-badge";
  archiveSummary.append(archiveTitleStrong, archiveCountBadge);
  const archiveList = document.createElement("ul");
  archiveList.className = "ng-ledger-list";
  archiveDisclosure.append(archiveSummary, archiveList);

  const rosterDisclosure = document.createElement("details");
  rosterDisclosure.className = "ng-disclosure";
  const rosterSummary = document.createElement("summary");
  const rosterTitleStrong = document.createElement("strong");
  rosterTitleStrong.textContent = "Surviving Sovereign Officials";
  const rosterCountBadge = document.createElement("span");
  rosterCountBadge.className = "ng-prov-badge";
  rosterSummary.append(rosterTitleStrong, rosterCountBadge);
  const rosterList = document.createElement("ul");
  rosterList.className = "ng-ledger-list";
  rosterDisclosure.append(rosterSummary, rosterList);

  const actionsBar = document.createElement("div");
  actionsBar.className = "ng-dialog-actions";
  const sealBtn = document.createElement("button");
  sealBtn.type = "button";
  sealBtn.className = "ng-btn ng-btn-primary";
  sealBtn.setAttribute("data-acknowledge-succession", "true");
  sealBtn.textContent = "Acknowledge Succession & Unseal Ledgers";
  sealBtn.addEventListener("click", () => {
    if (dialog.open) {
      dialog.close("acknowledge");
    } else {
      dialog.returnValue = "";
      bus.emit({ kind: "DispatchSimCommand", command: { kind: "acknowledgeSuccession" } });
      bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
    }
  });
  actionsBar.appendChild(sealBtn);

  dialog.append(titleEl, summaryPara, warningBanner, archiveDisclosure, rosterDisclosure, actionsBar);
  host.appendChild(dialog);

  let lastPromptedKey: string | null = null;

  const onClose = () => {
    const action = dialog.returnValue;
    dialog.returnValue = "";
    if (action === "acknowledge") {
      bus.emit({ kind: "DispatchSimCommand", command: { kind: "acknowledgeSuccession" } });
      bus.emit({ kind: "PlayUiFoley", cueId: "seal_stamp" });
    }
  };
  dialog.addEventListener("close", onClose);

  const syncFromStore = () => {
    const succ = selectSuccession(store);
    const pending = succ?.pendingTransition ?? null;
    if (!pending || !pending.awaitingPlayerAcknowledgement) {
      lastPromptedKey = null;
      if (dialog.open) {
        dialog.close("");
      }
      return;
    }

    const ledgerCount = selectLedgerRows(store).length;
    const actorCount = selectNonStateEconomy(store)?.actors.length ?? 0;
    const gov = selectGovernance(store);
    const livingOfficials = (succ?.roster ?? []).filter((c) => c.isAlive);

    summaryPara.textContent = `Predecessor ${pending.deceasedRulerName} succeeded under ${pending.ruleDisplayName} by ${pending.successorName} (Tick ${pending.transitionTick}). Transition disruption lasts until Tick ${pending.disruptionUntilTick} (Effective Command Multiplier ${succ?.effectiveCommandMultiplier.toFixed(2) ?? "1.00"}x). Preserved Continuity: ${ledgerCount} sealed stock ledgers, ${actorCount} known non-state institutions under ${gov?.activeModelName ?? "current governance"}.`;

    warningBanner.textContent = `Unrecorded Head-Knowledge Loss: ${pending.lostUnrecordedRapportCount} personal unwritten understandings and verbal impressions passed with ${pending.deceasedRulerName}.`;

    archiveCountBadge.textContent = `${pending.inheritedArchiveEntries.length} Physical Entries`;
    const archiveNodes: HTMLLIElement[] = pending.inheritedArchiveEntries.map((entry) => {
      const li = document.createElement("li");
      li.className = "ng-ledger-row";
      li.textContent = entry;
      return li;
    });
    if (archiveNodes.length === 0) {
      const empty = document.createElement("li");
      empty.textContent = "Only baseline physical settlement charters survived.";
      archiveNodes.push(empty);
    }
    archiveList.replaceChildren(...archiveNodes);

    rosterCountBadge.textContent = `${livingOfficials.length} Active`;
    const rosterNodes: HTMLLIElement[] = livingOfficials.map((ch) => {
      const li = document.createElement("li");
      li.className = "ng-ledger-row";
      li.textContent = `${ch.name} — ${ch.roleTitle} (${ch.settlementName}, Age ${Math.round(ch.ageYears)}, Health ${Math.round(ch.health01 * 100)}%)${ch.isRuler ? " [Incoming Sovereign]" : ""}`;
      return li;
    });
    rosterList.replaceChildren(...rosterNodes);

    const transitionKey = `${pending.transitionTick}:${pending.deceasedRulerName}:${pending.successorName}`;
    if (transitionKey !== lastPromptedKey) {
      lastPromptedKey = transitionKey;
      if (!dialog.open && typeof dialog.showModal === "function") {
        dialog.showModal();
      }
    }
  };

  syncFromStore();
  const unsub = store.subscribe(syncFromStore);

  return () => {
    unsub();
    dialog.removeEventListener("close", onClose);
    if (dialog.open) {
      dialog.close("");
    }
    dialog.remove();
  };
}
