import {
  selectDelegationTree,
  selectHqPresence,
  selectKnownCells,
  selectKnownSettlements,
  selectLedgerRows,
  selectTrackedConvoys,
  WindowContentPlugin,
} from "../spine";

function wireImageWithFallback(
  img: HTMLImageElement,
  primaryUrl: string,
  fallbackUrl: string,
  alt: string,
): void {
  img.alt = alt;
  let usedFallback = false;
  img.addEventListener("error", () => {
    if (!usedFallback && fallbackUrl && img.src !== fallbackUrl) {
      usedFallback = true;
      img.src = fallbackUrl;
    }
  });
  img.src = primaryUrl || fallbackUrl;
}

/**
 * Physical Commodity Ledger view with filter input and scrollable rows (`UI-WIN-003`, `UI-SKN-005`).
 */
export function createLedgerStubPlugin(): WindowContentPlugin {
  return {
    pluginId: "physical_ledger",
    defaultTitle: "Observed Physical Ledger",
    minWidth: 360,
    minHeight: 240,
    hotkey: "e",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const filterInput = document.createElement("input");
      filterInput.type = "search";
      filterInput.className = "ng-text-input";
      filterInput.setAttribute("data-ledger-filter", "true");
      filterInput.placeholder = "Filter commodities or settlements...";
      filterInput.setAttribute("aria-label", "Filter commodities or settlements");

      const list = document.createElement("ul");
      list.className = "ng-ledger-list ng-scroll-container";
      list.setAttribute("data-ledger-scroll", "true");

      const render = () => {
        const query = filterInput.value.trim().toLowerCase();
        const rows = selectLedgerRows(spine.store);
        const filtered = query
          ? rows.filter(
              (r) =>
                r.commodityName.toLowerCase().includes(query) ||
                r.settlementName.toLowerCase().includes(query) ||
                r.commodityKey.toLowerCase().includes(query),
            )
          : rows;

        const nodes: HTMLLIElement[] = [];
        for (const r of filtered) {
          const li = document.createElement("li");
          li.className = "ng-ledger-row";

          const left = document.createElement("span");
          const iconUrl = spine.assetResolver?.resolveIcon(r.commodityKey) ?? "";
          const img = document.createElement("img");
          img.className = "ng-commodity-icon";
          wireImageWithFallback(img, iconUrl, spine.assetResolver?.resolveIcon("__fallback__") ?? "", r.commodityName);
          left.append(img, document.createTextNode(` ${r.commodityName} (${r.settlementName})`));

          const right = document.createElement("span");
          right.textContent = `${Math.round(r.reportedMassKg)} kg [${Math.round(r.minEstKg)}..${Math.round(r.maxEstKg)}]`;

          li.append(left, right);
          nodes.push(li);
        }

        if (nodes.length === 0) {
          const empty = document.createElement("li");
          empty.className = "ng-ledger-row";
          empty.textContent = "No matching reported ledger entries.";
          nodes.push(empty);
        }
        list.replaceChildren(...nodes);
      };

      filterInput.addEventListener("input", render);
      render();
      root.append(filterInput, list);
      container.replaceChildren(root);

      const unsubStore = spine.store.subscribe(render);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") render();
      });
      return () => {
        unsubStore();
        unsubBus();
      };
    },
  };
}

/**
 * Multi-instance-capable Diegetic Entity Inspector (`UI-WIN-004`, `UI-SKN-005`).
 */
export function createEntityInspectorWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "entity_inspector",
    defaultTitle: "Diegetic Field Inspector",
    minWidth: 320,
    minHeight: 220,
    hotkey: "i",
    mountContent(container, instanceKey, spine) {
      const box = document.createElement("div");
      box.className = "ng-responsive-grid";

      const render = () => {
        const snap = spine.store.getSnapshot();
        const header = document.createElement("div");
        header.className = "ng-inspector-header";

        const [rawKind, rawId] = instanceKey.split(":");
        const targetKind = rawKind || "settlement";
        const targetId = Number(rawId);

        const img = document.createElement("img");
        img.className = "ng-portrait-img";

        const info = document.createElement("div");
        const title = document.createElement("strong");
        const detail = document.createElement("p");

        const fallbackPortrait =
          spine.assetResolver?.resolvePortrait({ characterId: 0, vocation: "__none__" }) ?? "";

        if (!snap) {
          title.textContent = "Awaiting Survey Snapshot";
          detail.textContent = "No survey records received yet.";
          wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, "Default silhouette");
        } else if (targetKind === "character" && Number.isFinite(targetId)) {
          const tree = selectDelegationTree(spine.store);
          const sub = tree?.subordinates.find((s) => s.characterId === targetId) ?? tree?.subordinates[0];
          const primaryUrl =
            spine.assetResolver?.resolvePortrait({
              characterId: sub?.characterId ?? targetId,
              vocation: sub?.roleTitle.toLowerCase().includes("artificer") ? "artificer" : "agronomist",
              ageBracket: "adult",
            }) ?? "";
          wireImageWithFallback(img, primaryUrl, fallbackPortrait, sub?.displayName ?? "Official");
          title.textContent = sub ? `${sub.displayName} — ${sub.roleTitle}` : `Official #${targetId}`;
          detail.textContent = sub
            ? `Competence: ${(sub.competence * 100).toFixed(0)}% | Reported to ${tree?.leaderTitle ?? "Commander"}`
            : "Unverified official record.";
        } else if (targetKind === "convoy" && Number.isFinite(targetId)) {
          const convoys = selectTrackedConvoys(spine.store);
          const conv = convoys.find((c) => c.convoyId === targetId) ?? convoys[0];
          const iconUrl = spine.assetResolver?.resolveIcon("colony_precision_tools") ?? "";
          wireImageWithFallback(img, iconUrl, fallbackPortrait, "Convoy icon");
          title.textContent = conv ? `${conv.label} (${conv.status})` : `Convoy #${targetId}`;
          detail.textContent = conv ? `Last projected cell: #${conv.projectedCellId}` : "No telemetry available.";
        } else if (targetKind === "cell" && Number.isFinite(targetId)) {
          const cell = selectKnownCells(spine.store).find((c) => c.cellId === targetId);
          wireImageWithFallback(img, fallbackPortrait, fallbackPortrait, `Cell #${targetId}`);
          if (!cell || cell.tier === "unexplored") {
            title.textContent = `Unexplored Territory (Cell #${targetId})`;
            detail.textContent = "No expedition survey report has reached HQ for this cell.";
          } else {
            title.textContent = `${cell.tier.toUpperCase()} Survey — Cell #${cell.cellId}`;
            detail.textContent = `Elevation: ${Math.round(cell.elevationM)}m | Confidence: ${Math.round(cell.confidence01 * 100)}% | Source: ${cell.provenanceLabel}`;
          }
        } else {
          const settlements = selectKnownSettlements(spine.store);
          const hq = selectHqPresence(spine.store);
          const found = Number.isFinite(targetId)
            ? settlements.find((s) => s.settlementId === targetId)
            : settlements[0];
          const tree = selectDelegationTree(spine.store);
          const rep = tree?.subordinates[0];
          const portraitUrl =
            spine.assetResolver?.resolvePortrait({
              characterId: rep?.characterId ?? 1,
              vocation: "agronomist",
              ageBracket: "adult",
            }) ?? "";
          wireImageWithFallback(img, portraitUrl, fallbackPortrait, found?.settlementName ?? "Settlement Overseer");
          title.textContent = found ? found.settlementName : hq?.settlementName ?? "Primary Settlement";
          detail.textContent = `Cell #${found?.cellId ?? hq?.cellId ?? 0} | HQ Sight Radius: ${hq?.sightRadiusCells ?? 1} cells`;
        }

        info.append(title, detail);
        header.append(img, info);

        const actions = document.createElement("div");
        actions.className = "ng-hud-cluster";
        const pinCloneBtn = document.createElement("button");
        pinCloneBtn.type = "button";
        pinCloneBtn.className = "ng-btn";
        pinCloneBtn.textContent = "Open Pinned Comparison Copy";
        pinCloneBtn.addEventListener("click", () => {
          spine.bus.emit({
            kind: "OpenSystemWindow",
            pluginId: "entity_inspector",
            instanceKey,
            pinned: true,
          });
        });

        actions.appendChild(pinCloneBtn);
        box.replaceChildren(header, actions);
      };

      render();
      container.replaceChildren(box);
      const unsubStore = spine.store.subscribe(render);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") render();
      });
      return () => {
        unsubStore();
        unsubBus();
      };
    },
  };
}

/**
 * Window Layout Presets & Workspace Profiles selector (`UI-WIN-007`).
 */
export function createLayoutPresetsWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "layout_presets",
    defaultTitle: "Workspace Layout Presets",
    minWidth: 320,
    minHeight: 220,
    hotkey: "l",
    mountContent(container, _instanceKey, spine) {
      const wrap = document.createElement("div");
      wrap.className = "ng-responsive-grid";

      const nameInput = document.createElement("input");
      nameInput.type = "text";
      nameInput.className = "ng-text-input";
      nameInput.placeholder = "New layout preset name...";
      nameInput.setAttribute("aria-label", "New layout preset name");

      const saveBtn = document.createElement("button");
      saveBtn.type = "button";
      saveBtn.className = "ng-btn ng-btn-primary";
      saveBtn.textContent = "Save Current Layout";

      const presetList = document.createElement("div");
      presetList.className = "ng-hud-cluster";

      const render = () => {
        const names = spine.layoutPresets?.listPresetNames() ?? [];
        const buttons: HTMLButtonElement[] = [];
        for (const name of names) {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "ng-btn";
          btn.setAttribute("data-layout-preset", name);
          btn.textContent = name;
          btn.addEventListener("click", () => {
            spine.bus.emit({ kind: "ApplyWindowLayoutPreset", presetName: name });
          });
          buttons.push(btn);
        }
        presetList.replaceChildren(...buttons);
      };

      saveBtn.addEventListener("click", () => {
        const val = nameInput.value.trim();
        if (!val) return;
        spine.bus.emit({ kind: "SaveWindowLayoutPreset", presetName: val });
        nameInput.value = "";
        render();
      });

      render();
      wrap.append(presetList, nameInput, saveBtn);
      container.replaceChildren(wrap);
      return () => {};
    },
  };
}
