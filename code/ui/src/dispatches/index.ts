import { WindowContentPlugin, selectDispatchInbox } from "../spine";

/** Courier dispatch inbox view rendering dispatch header plate and arrived dispatches (`UI-SKN-005`). */
export function createDispatchInboxStubPlugin(): WindowContentPlugin {
  return {
    pluginId: "dispatch_inbox",
    defaultTitle: "Courier Dispatches",
    minWidth: 320,
    minHeight: 220,
    hotkey: "d",
    mountContent(container, _instanceKey, spine) {
      const root = document.createElement("div");
      root.className = "ng-responsive-grid";

      const plate = document.createElement("img");
      plate.className = "ng-dispatch-plate-img";
      plate.alt = "Expedition Dispatch Seal";

      const list = document.createElement("ul");
      list.className = "ng-ledger-list ng-scroll-container";

      const update = () => {
        const primaryUrl = spine.assetResolver?.resolveDispatchPlate("expedition_dispatch") ?? "";
        const fallbackUrl = spine.assetResolver?.resolveDispatchPlate("__fallback__") ?? "";
        if (plate.src !== (primaryUrl || fallbackUrl)) {
          plate.src = primaryUrl || fallbackUrl;
        }

        const items = selectDispatchInbox(spine.store);
        const rows: HTMLLIElement[] = items.map((item) => {
          const li = document.createElement("li");
          li.className = "ng-ledger-row";
          li.textContent = `${item.title} (${item.originLabel}, arrived tick ${item.arrivedTick})`;
          return li;
        });

        if (rows.length === 0) {
          const empty = document.createElement("li");
          empty.className = "ng-ledger-row";
          empty.textContent = "Arrived dispatches: 0";
          rows.push(empty);
        }
        list.replaceChildren(...rows);
      };

      update();
      root.append(plate, list);
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
