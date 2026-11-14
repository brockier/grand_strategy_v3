import {
  ModValidationFailureState,
  WindowContentPlugin,
} from "../spine";

export interface DiagnosticRecoveryActions {
  onReloadMods(): Promise<void>;
  onRevertDefault(): Promise<void>;
}

/**
 * Mounts and manages the accessible `<dialog>` Mod & Schema Diagnostic Error Screen (`UI-SHL-004`).
 * Uses native `dialog.showModal()`, semantic responsive `<table>` structure, and strictly safe DOM APIs.
 */
export class ModDiagnosticDialog {
  private readonly dialog: HTMLDialogElement;
  private readonly tbody: HTMLTableSectionElement;
  private readonly summaryLine: HTMLParagraphElement;
  private readonly reloadBtn: HTMLButtonElement;
  private readonly revertBtn: HTMLButtonElement;

  constructor(hostRoot: HTMLElement, actions: DiagnosticRecoveryActions) {
    this.dialog = document.createElement("dialog");
    this.dialog.id = "mod-diagnostic-dialog";
    this.dialog.className = "ng-mod-dialog";
    this.dialog.setAttribute("aria-labelledby", "mod-diag-title");

    const title = document.createElement("h2");
    title.id = "mod-diag-title";
    title.textContent = "Mod Validation & Schema Diagnostics";
    this.dialog.appendChild(title);

    this.summaryLine = document.createElement("p");
    this.summaryLine.textContent = "Validation errors prevented loading the active mod stack.";
    this.dialog.appendChild(this.summaryLine);

    const wrapper = document.createElement("div");
    wrapper.tabIndex = 0;
    wrapper.setAttribute("role", "region");
    wrapper.setAttribute("aria-labelledby", "mod-diag-table-caption");
    wrapper.className = "ng-table-wrapper";

    const table = document.createElement("table");
    table.className = "ng-diagnostic-table";
    table.style.setProperty("--label-2", '"Layer"');
    table.style.setProperty("--label-3", '"Location"');
    table.style.setProperty("--label-4", '"Field"');
    table.style.setProperty("--label-5", '"Remediation"');

    const caption = document.createElement("caption");
    caption.id = "mod-diag-table-caption";
    caption.textContent = "Detected TOML and JSON Mod Schema Diagnostics";
    table.appendChild(caption);

    const thead = document.createElement("thead");
    const headRow = document.createElement("tr");
    for (const label of ["File Path", "Layer", "Location", "Field", "Remediation"]) {
      const th = document.createElement("th");
      th.scope = "col";
      th.textContent = label;
      headRow.appendChild(th);
    }
    thead.appendChild(headRow);
    table.appendChild(thead);

    this.tbody = document.createElement("tbody");
    table.appendChild(this.tbody);
    wrapper.appendChild(table);
    this.dialog.appendChild(wrapper);

    const actionsRow = document.createElement("div");
    actionsRow.className = "ng-dialog-actions";

    this.reloadBtn = document.createElement("button");
    this.reloadBtn.type = "button";
    this.reloadBtn.id = "mod-diag-reload-btn";
    this.reloadBtn.className = "ng-btn ng-btn-primary";
    this.reloadBtn.autofocus = true;
    this.reloadBtn.textContent = "Reload Mods";

    this.revertBtn = document.createElement("button");
    this.revertBtn.type = "button";
    this.revertBtn.id = "mod-diag-revert-btn";
    this.revertBtn.className = "ng-btn";
    this.revertBtn.textContent = "Revert to Default Pack";

    actionsRow.appendChild(this.revertBtn);
    actionsRow.appendChild(this.reloadBtn);
    this.dialog.appendChild(actionsRow);

    this.dialog.addEventListener("close", () => {
      const _chosen = this.dialog.returnValue;
      this.dialog.returnValue = "";
    });

    this.reloadBtn.addEventListener("click", () => {
      void actions.onReloadMods();
    });
    this.revertBtn.addEventListener("click", () => {
      void actions.onRevertDefault();
    });

    hostRoot.appendChild(this.dialog);
  }

  public showDiagnostics(failure: ModValidationFailureState): void {
    this.tbody.replaceChildren();
    this.summaryLine.textContent = `Detected ${failure.diagnostics.length} mod validation issue(s). Fix the listed files or revert to the verified baseline pack.`;

    for (const diag of failure.diagnostics) {
      const tr = document.createElement("tr");

      const thFile = document.createElement("th");
      thFile.scope = "row";
      thFile.textContent = diag.filePath;
      tr.appendChild(thFile);

      const tdLayer = document.createElement("td");
      tdLayer.textContent = diag.layer;
      tr.appendChild(tdLayer);

      const tdLoc = document.createElement("td");
      tdLoc.textContent = `${diag.line}:${diag.column}`;
      tr.appendChild(tdLoc);

      const tdField = document.createElement("td");
      tdField.textContent = diag.fieldPath;
      tr.appendChild(tdField);

      const tdMsg = document.createElement("td");
      tdMsg.textContent = diag.message;
      tr.appendChild(tdMsg);

      this.tbody.appendChild(tr);
    }

    if (!this.dialog.open) {
      if (typeof this.dialog.showModal === "function") {
        this.dialog.showModal();
      } else {
        this.dialog.setAttribute("open", "");
      }
    }
  }

  public hide(): void {
    if (this.dialog.open) {
      if (typeof this.dialog.close === "function") {
        this.dialog.close("recovered");
      } else {
        this.dialog.removeAttribute("open");
      }
    }
  }

  public isOpen(): boolean {
    return this.dialog.open;
  }
}

export function createPlaceholderWindowPlugin(): WindowContentPlugin {
  return {
    pluginId: "shell_overview",
    defaultTitle: "Colony Readiness Overview",
    minWidth: 320,
    minHeight: 200,
    mountContent(container, _instanceKey, spine) {
      const p = document.createElement("p");
      const update = () => {
        const snap = spine.store.getSnapshot();
        p.textContent = snap
          ? `${snap.scenarioName} — ${snap.clockAndNotifications.calendarText}`
          : "Awaiting DiegeticSnapshot...";
      };
      update();
      container.replaceChildren(p);
      return spine.store.subscribe(update);
    },
  };
}
