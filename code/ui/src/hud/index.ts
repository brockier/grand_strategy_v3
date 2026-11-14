import {
  DiegeticStoreAccess,
  UiIntentBus,
  selectClockAndNotifications,
  selectHqPresence,
} from "../spine";

/** Top command HUD bar displaying simulated calendar hour and live HQ location. */
export function mountCommandHud(
  host: HTMLElement,
  store: DiegeticStoreAccess,
  bus: UiIntentBus,
): () => void {
  const bar = document.createElement("header");
  bar.className = "ng-hud-bar";

  const scenarioLabel = document.createElement("strong");
  scenarioLabel.id = "hud-scenario-title";
  scenarioLabel.textContent = "Awaiting scenario";

  const clockLabel = document.createElement("span");
  clockLabel.id = "hud-clock-text";
  clockLabel.textContent = "Year 1";

  const stepBtn = document.createElement("button");
  stepBtn.type = "button";
  stepBtn.id = "hud-step-btn";
  stepBtn.className = "ng-btn";
  stepBtn.textContent = "+1 Hour";
  stepBtn.addEventListener("click", () => {
    bus.emit({ kind: "DispatchSimCommand", command: { kind: "stepSingleTick" } });
  });

  bar.appendChild(scenarioLabel);
  bar.appendChild(clockLabel);
  bar.appendChild(stepBtn);
  host.appendChild(bar);

  return store.subscribe((snap) => {
    const hq = selectHqPresence(store);
    const clk = selectClockAndNotifications(store);
    scenarioLabel.textContent = `${snap.scenarioName} — HQ: ${hq?.settlementName ?? "Unknown"}`;
    clockLabel.textContent = clk ? `${clk.calendarText} [Tick ${clk.tick}]` : "";
  });
}
