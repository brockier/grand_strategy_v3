import { WindowContentPlugin, selectDispatchInbox } from "../spine";

/** Baseline courier dispatch inbox plugin cartridge isolated behind UI Spine. */
export function createDispatchInboxStubPlugin(): WindowContentPlugin {
  return {
    pluginId: "dispatch_inbox",
    defaultTitle: "Courier Dispatches",
    minWidth: 320,
    minHeight: 200,
    mountContent(container, _instanceKey, spine) {
      const p = document.createElement("p");
      const update = () => {
        const items = selectDispatchInbox(spine.store);
        p.textContent = `Arrived dispatches: ${items.length}`;
      };
      update();
      container.replaceChildren(p);
      return spine.store.subscribe(update);
    },
  };
}
