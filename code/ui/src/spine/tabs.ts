export interface AccessibleTabSpec {
  readonly key: string;
  readonly label: string;
}

export interface AccessibleTabGroupHandle {
  readonly root: HTMLDivElement;
  readonly tablist: HTMLDivElement;
  readonly panelByKey: ReadonlyMap<string, HTMLDivElement>;
  readonly panelInnerByKey: ReadonlyMap<string, HTMLDivElement>;
  getActiveKey(): string;
  activateTab(key: string): void;
}

/**
 * Builds an accessible `role="tablist"` / `role="tabpanel"` widget following
 * Modern Web Guidance (`hidden="until-found"`, `beforematch` reveal, zero box
 * chrome on `[role="tabpanel"]` with `.ng-tabpanel-inner`, and keyboard roving).
 */
export function createAccessibleTabGroup(
  doc: Document,
  options: {
    readonly prefix: string;
    readonly ariaLabel: string;
    readonly tabs: readonly AccessibleTabSpec[];
    readonly initialKey?: string;
    readonly onSelect?: (key: string) => void;
  },
): AccessibleTabGroupHandle {
  const root = doc.createElement("div");
  root.className = "ng-accessible-tabs";

  const tablist = doc.createElement("div");
  tablist.className = "ng-tablist";
  tablist.setAttribute("role", "tablist");
  tablist.setAttribute("aria-label", options.ariaLabel);

  const tabBtnByKey = new Map<string, HTMLButtonElement>();
  const panelByKey = new Map<string, HTMLDivElement>();
  const panelInnerByKey = new Map<string, HTMLDivElement>();
  const orderedKeys = options.tabs.map((t) => t.key);
  let activeKey =
    options.initialKey && orderedKeys.includes(options.initialKey)
      ? options.initialKey
      : orderedKeys[0] ?? "";

  const activateTab = (nextKey: string, notify = true) => {
    if (!tabBtnByKey.has(nextKey)) return;
    activeKey = nextKey;
    for (const key of orderedKeys) {
      const btn = tabBtnByKey.get(key)!;
      const panel = panelByKey.get(key)!;
      const isSelected = key === activeKey;
      btn.setAttribute("aria-selected", isSelected ? "true" : "false");
      btn.tabIndex = isSelected ? 0 : -1;
      if (isSelected) {
        panel.removeAttribute("hidden");
      } else {
        panel.setAttribute("hidden", "until-found");
      }
    }
    if (notify) {
      options.onSelect?.(activeKey);
    }
  };

  for (const spec of options.tabs) {
    const tabId = `${options.prefix}-tab-${spec.key}`;
    const panelId = `${options.prefix}-panel-${spec.key}`;

    const btn = doc.createElement("button");
    btn.type = "button";
    btn.className = "ng-tab";
    btn.id = tabId;
    btn.setAttribute("role", "tab");
    btn.setAttribute("aria-controls", panelId);
    btn.setAttribute("data-tab-key", spec.key);
    btn.textContent = spec.label;
    btn.addEventListener("click", () => {
      activateTab(spec.key);
    });
    tabBtnByKey.set(spec.key, btn);
    tablist.appendChild(btn);

    const panel = doc.createElement("div");
    panel.className = "ng-tabpanel";
    panel.id = panelId;
    panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-labelledby", tabId);
    panel.setAttribute("data-panel-key", spec.key);

    const inner = doc.createElement("div");
    inner.className = "ng-tabpanel-inner";
    panel.appendChild(inner);

    panel.addEventListener("beforematch", () => {
      activateTab(spec.key);
    });

    panelByKey.set(spec.key, panel);
    panelInnerByKey.set(spec.key, inner);
  }

  tablist.addEventListener("keydown", (ev) => {
    const curIdx = orderedKeys.indexOf(activeKey);
    if (curIdx < 0 || orderedKeys.length === 0) return;
    let nextIdx = -1;
    if (ev.key === "ArrowRight") {
      nextIdx = (curIdx + 1) % orderedKeys.length;
    } else if (ev.key === "ArrowLeft") {
      nextIdx = (curIdx - 1 + orderedKeys.length) % orderedKeys.length;
    } else if (ev.key === "Home") {
      nextIdx = 0;
    } else if (ev.key === "End") {
      nextIdx = orderedKeys.length - 1;
    }
    if (nextIdx >= 0) {
      ev.preventDefault();
      const targetKey = orderedKeys[nextIdx];
      activateTab(targetKey);
      tabBtnByKey.get(targetKey)?.focus();
    }
  });

  root.appendChild(tablist);
  for (const key of orderedKeys) {
    const panel = panelByKey.get(key);
    if (panel) root.appendChild(panel);
  }

  activateTab(activeKey, false);

  return {
    root,
    tablist,
    panelByKey,
    panelInnerByKey,
    getActiveKey: () => activeKey,
    activateTab: (key) => activateTab(key, true),
  };
}

/**
 * Synchronizes `<select>` options only when the option list values or labels change,
 * preserving active user selection across high-frequency live snapshot updates.
 */
export function syncSelectOptions(
  select: HTMLSelectElement,
  items: ReadonlyArray<{ readonly value: string; readonly label: string }>,
): void {
  const sig = items.map((i) => `${i.value}\u0000${i.label}`).join("\u0001");
  if (select.dataset.optionsSig === sig) return;
  const prevValue = select.value;
  select.dataset.optionsSig = sig;
  const doc = select.ownerDocument;
  const opts = items.map((item) => {
    const opt = doc.createElement("option");
    opt.value = item.value;
    opt.textContent = item.label;
    return opt;
  });
  select.replaceChildren(...opts);
  if (prevValue && items.some((i) => i.value === prevValue)) {
    select.value = prevValue;
  } else if (items.length > 0) {
    select.value = items[0].value;
  }
}
