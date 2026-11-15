import {
  DiegeticDispatchDto,
  DispatchTokenDto,
  EntityKind,
  WindowContentPlugin,
  createProvenanceBadgeElement,
  selectDispatchInbox,
} from "../spine";

const ALLOWED_ENTITY_KINDS = new Set<string>([
  "settlement",
  "character",
  "convoy",
  "facility",
  "cell",
]);

function renderTokenButton(
  doc: Document,
  token: DispatchTokenDto,
  onNavigate: (kind: EntityKind | null, id: number | null, cellId: number | null) => void,
): Node {
  const validKind =
    token.entityKind && ALLOWED_ENTITY_KINDS.has(token.entityKind)
      ? (token.entityKind as EntityKind)
      : null;

  if (validKind === null && token.targetCellId === null) {
    return doc.createTextNode(token.text);
  }

  const btn = doc.createElement("button");
  btn.type = "button";
  btn.className = "ng-dispatch-token";
  if (validKind) btn.dataset.entityKind = validKind;
  if (token.entityId !== null) btn.dataset.entityId = String(token.entityId);
  if (token.targetCellId !== null) btn.dataset.targetCell = String(token.targetCellId);
  btn.textContent = token.text;
  btn.addEventListener("click", () => {
    onNavigate(validKind, token.entityId, token.targetCellId);
  });
  return btn;
}

/**
 * Courier Dispatch & Field Survey Reader displaying epistemic provenance badges and
 * hyperlinked entity tokens for instant globe fly-to and inspector focus
 * (`UI-DAT-004`, `UI-DAT-005`, `UI-SKN-005`, `UI-AUD-003`).
 */
export function createDispatchInboxStubPlugin(): WindowContentPlugin {
  return {
    pluginId: "dispatch_inbox",
    defaultTitle: "Courier Dispatches",
    minWidth: 400,
    minHeight: 280,
    hotkey: "d",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const plate = document.createElement("img");
      plate.className = "ng-dispatch-plate-img";
      plate.alt = "Expedition Dispatch Seal";

      const list = document.createElement("ul");
      list.className = "ng-ledger-list ng-scroll-container";

      const reader = document.createElement("article");
      reader.className = "ng-subordinate-card";
      reader.setAttribute("data-dispatch-reader", "true");

      let selectedDispatchId: number | null = null;

      const navigateToken = (
        kind: EntityKind | null,
        id: number | null,
        cellId: number | null,
      ) => {
        if (cellId !== null && Number.isFinite(cellId)) {
          spine.bus.emit({ kind: "FlyGlobeToCell", cellId });
        }
        if (kind !== null && id !== null && Number.isFinite(id)) {
          spine.bus.emit({
            kind: "FocusEntityInspector",
            entityKind: kind,
            entityId: id,
          });
        }
        spine.bus.emit({ kind: "PlayUiFoley", cueId: "ledger_page_turn" });
      };

      const renderReader = (item: DiegeticDispatchDto | undefined, currentTick: number) => {
        if (!item) {
          reader.replaceChildren(document.createTextNode("Select an arrived courier dispatch to inspect its seal and provenance."));
          return;
        }

        const header = document.createElement("div");
        header.className = "ng-hud-cluster";

        const titleEl = document.createElement("strong");
        titleEl.textContent = item.title;

        const provBadge = createProvenanceBadgeElement(document, {
          currentTick,
          observedTick: item.sentTick,
          confidence01: item.confidence01 ?? 0.78,
          provenanceLabel: item.originLabel,
          reporterName: item.senderName,
        });

        header.append(titleEl, provBadge);

        const metaRow = document.createElement("div");
        metaRow.className = "ng-hud-cluster";
        metaRow.textContent = `Sent t${item.sentTick}, Arrived t${item.arrivedTick} (${Math.max(0, item.arrivedTick - item.sentTick)}h transit) · `;

        if (item.senderName && item.senderId !== undefined) {
          const senderBtn = document.createElement("button");
          senderBtn.type = "button";
          senderBtn.className = "ng-dispatch-token";
          senderBtn.dataset.entityKind = "character";
          senderBtn.dataset.entityId = String(item.senderId);
          senderBtn.textContent = item.senderName;
          senderBtn.addEventListener("click", () => {
            navigateToken("character", item.senderId ?? null, item.originCellId ?? null);
          });
          metaRow.appendChild(senderBtn);
        }

        if (item.originCellId !== undefined) {
          metaRow.appendChild(document.createTextNode(" from "));
          const originBtn = document.createElement("button");
          originBtn.type = "button";
          originBtn.className = "ng-dispatch-token";
          originBtn.dataset.targetCell = String(item.originCellId);
          originBtn.textContent = `${item.originLabel} (#${item.originCellId})`;
          originBtn.addEventListener("click", () => {
            navigateToken("cell", item.originCellId ?? null, item.originCellId ?? null);
          });
          metaRow.appendChild(originBtn);
        }

        const bodyPara = document.createElement("p");
        const tokens = item.bodyTokens ?? [];
        if (tokens.length > 0) {
          for (const tok of tokens) {
            bodyPara.appendChild(renderTokenButton(document, tok, navigateToken));
          }
        } else {
          bodyPara.textContent = `Dispatched from ${item.originLabel} on Tick ${item.sentTick}.`;
        }

        reader.replaceChildren(header, metaRow, bodyPara);
      };

      const update = () => {
        const primaryUrl = spine.assetResolver?.resolveDispatchPlate("expedition_dispatch") ?? "";
        const fallbackUrl = spine.assetResolver?.resolveDispatchPlate("__fallback__") ?? "";
        if (plate.src !== (primaryUrl || fallbackUrl)) {
          plate.src = primaryUrl || fallbackUrl;
        }

        const snap = spine.store.getSnapshot();
        const currentTick = snap?.clockAndNotifications.tick ?? 0;
        const items = selectDispatchInbox(spine.store);
        if (selectedDispatchId === null && items.length > 0) {
          selectedDispatchId = items[items.length - 1].dispatchId;
        }

        const rows: HTMLLIElement[] = items.map((item) => {
          const li = document.createElement("li");
          li.className = "ng-ledger-row";

          const pickBtn = document.createElement("button");
          pickBtn.type = "button";
          pickBtn.className = "ng-btn";
          pickBtn.dataset.dispatchId = String(item.dispatchId);
          pickBtn.setAttribute("aria-pressed", item.dispatchId === selectedDispatchId ? "true" : "false");
          pickBtn.textContent = `${item.title} (${item.originLabel}, t${item.arrivedTick})`;
          pickBtn.addEventListener("click", () => {
            selectedDispatchId = item.dispatchId;
            spine.bus.emit({ kind: "PlayUiFoley", cueId: "dispatch_seal_break" });
            update();
          });

          const badge = createProvenanceBadgeElement(document, {
            currentTick,
            observedTick: item.sentTick,
            confidence01: item.confidence01 ?? 0.78,
            provenanceLabel: item.originLabel,
            reporterName: item.senderName,
          });

          li.append(pickBtn, badge);
          return li;
        });

        if (rows.length === 0) {
          const empty = document.createElement("li");
          empty.className = "ng-ledger-row";
          empty.textContent = "Arrived dispatches: 0";
          rows.push(empty);
        }
        list.replaceChildren(...rows);

        const active = items.find((d) => d.dispatchId === selectedDispatchId) ?? items[0];
        renderReader(active, currentTick);
      };

      update();
      root.append(plate, list, reader);
      container.replaceChildren(root);

      const unsubStore = spine.store.subscribe(update);
      const unsubBus = spine.bus.subscribe((intent) => {
        if (intent.kind === "SkinBundleUpdated") update();
      });
      return () => {
        unsubStore();
        unsubBus();
      };
    },
  };
}

export const createDispatchReaderWindowPlugin = createDispatchInboxStubPlugin;
