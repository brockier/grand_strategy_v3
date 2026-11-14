import { WindowContentPlugin, selectLedgerRows } from "../spine";

/** Baseline ledger table plugin cartridge using Spine selectors only. */
export function createLedgerStubPlugin(): WindowContentPlugin {
  return {
    pluginId: "physical_ledger",
    defaultTitle: "Observed Physical Ledger",
    minWidth: 360,
    minHeight: 240,
    mountContent(container, _instanceKey, spine) {
      const pre = document.createElement("p");
      const update = () => {
        const rows = selectLedgerRows(spine.store);
        pre.textContent = `Ledger entries visible at HQ: ${rows.length}`;
      };
      update();
      container.replaceChildren(pre);
      return spine.store.subscribe(update);
    },
  };
}
