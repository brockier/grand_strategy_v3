# Module Design: Command HUD, Mandates, HQ Planner & Notification Matrix

**Module ID:** `UI-HUD-07`  
**Code Location:** `apps/ui-shell/src/hud/`  
**Milestone & Design Depth:** `M2 & M4: Full Detailed Design` + `M7: Locked Command/Succession Slots`  
**Satisfies Requirements:** `UI-HUD-001`, `UI-HUD-002`, `UI-HUD-003`, `UI-HUD-004`, `UI-HUD-005`, `UI-HUD-006`, `UI-HUD-007` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`selectClockAndNotifications`, `selectHqPresence`, `selectDelegationTree`, `selectKnownCells`)
  - `UiIntentBus` (`OpenSystemWindow`, `FocusEntityInspector`)
- **Provided Spine Ports (Outputs):**
  - Fixed top/edge HUD controls (`ClockAndStatusHudBar`)
  - Registered `WindowContentPlugin` panels:
    1. `MandateAuthoringWindow` (`UI-HUD-002`)
    2. `ProposalArbitrationWindow` (`UI-HUD-003`)
    3. `PersonalAttentionWindow` (`UI-HUD-004`)
    4. `HqRelocationWindow` (`UI-HUD-005`)
    5. `NotificationMatrixWindow` (`UI-HUD-006`)
    6. `WarfarePostureAndSuccessionSlots` (`UI-HUD-007`, `M7` slot)
  - Emits `DispatchSimCommand`, `PreviewHqLatencyRoute`, and `PlayUiFoley` intents

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Clock Bar & Mandate Draft Form State

```typescript
interface MandateDraftState {
  objectiveKind: "IncreaseCommodity" | "ConnectCorridor" | "SurveyRegion" | "MaintainSubsistence";
  targetCommodityId?: number;
  assigneeCharacterId: number;
  priority: number;
  previewAttentionDelta: number;      // Calculated locally from assignee currentLoad + 1
  previewOutboundCourierEtaHours: number;
}

interface HqRelocationDraftState {
  selectedDestinationCellId: number | null;
  previewRouteCells: readonly number[];
  previewTravelDurationHours: number;
  latencyDeltaRows: readonly { settlementName: string; beforeHours: number; afterHours: number }[];
}
```

---

## 3. Algorithms & Player Command Workflows

### 3.1 Clock & Scenario Calendar HUD Bar (`UI-HUD-001`)
1. Anchored cleanly along the top edge (`pointer-events: auto` only on bar bounds so empty canvas stays clickable).
2. Displays formatted scenario calendar (`Year 1, Spring Day 14 — 08:00`), current HQ location chip (`Click to center globe`), Personal Attention badge (`3.5 / 5.0`), Pause/Play toggle, `+1h` single-tick step button (enabled strictly when paused), speed selector (`1x`, `4x`, `24x`, `168x`), and Auto-Pause alert banner showing exact `EventClassId` reason whenever the engine paused automatically.

### 3.2 Mandate Authoring & Bottleneck Arbitration (`UI-HUD-002`, `UI-HUD-003`)
1. **Mandate Authoring:** Selecting an outcome objective and assignee immediately previews whether assigning the mandate pushes that subordinate above Attention Capacity (`Oversight Dilution Warning`) and displays estimated outbound courier hours across currently surveyed routes before sending `IssueMandate`.
2. **Proposal Arbitration:** Escalated `ArbitrationProposal` items open side-by-side option cards comparing required commodities, diverted cohort labor, institutional concessions (monopoly/contract impact), and turnaround courier delay; selecting an option dispatches `ResolveProposal` and triggers a skin-matched wax-seal stamp sound cue (`UI-AUD-003`).

### 3.3 Personal Attention Allocation & HQ Relocation (`UI-HUD-004`, `UI-HUD-005`)
1. **Personal Attention Panel:** Lists player active direct reports and active mandates with toggle switches for `Zoom-In Direct Supervision`. Toggling one on recalculates live player Attention budget utilization across all remaining ministries instantly.
2. **HQ Relocation Planner:** Clicking a target cell emits `PreviewHqLatencyRoute { destinationCellId }` to overlay travel corridor and before-versus-after courier latency deltas to every known settlement before submitting `RelocateHq`.

### 3.4 Four-Tier Notification & Auto-Pause Matrix (`UI-HUD-006`)
Displays every scenario `EventClassId` row against four mutually exclusive radio columns (`Log Only`, `Banner Alert`, `Dispatch Popup`, `Dispatch Popup + Auto-Pause`), immediately dispatching `UpdateNotificationRule` commands persisted to user/scenario settings.

### 3.5 Locked `M7` Command & Succession Slots (`UI-HUD-007`)
Pre-wires plugin IDs and IPC command stubs (`IssueTheaterDirective`, `IssueOperationalBattlePlan`, `AcknowledgeSuccession`) so `M7` warfare posture controls (Churchill theater vs. Hitler operational plan + `"No Retreat"`) and the Succession transition screen drop into the HUD without structural rewrites.

---

## 4. Isolated Cartridge Verification Strategy

1. **Clock Controls & Auto-Pause Banner (`@req UI-HUD-001`):** Playwright clicks Pause, `+1h`, and `24x`, asserts matching `SetClockMode`/`StepTick` IPC calls, and verifies auto-pause reason banner appears when snapshot enters `AutoPaused`.
2. **Mandate, Proposal & Attention Flows (`@req UI-HUD-002`, `@req UI-HUD-003`, `@req UI-HUD-004`):** Verifies overload preview warning when assigning to an overloaded subordinate, validates proposal choice submission, and confirms zoom-in toggle updates live Attention meter.
3. **HQ Relocation Preview (`@req UI-HUD-005`):** Selects distant settlement and verifies `PreviewHqLatencyRoute` intent emission plus before/after latency comparison table prior to confirmation.
4. **Notification Matrix Persistence (`@req UI-HUD-006`):** Changes `overdue_expedition` to `Dispatch Popup + Auto-Pause`, restarts mock shell with persisted settings, and confirms preserved selection.

---

## 5. Lift-and-Shift Replacement Guide

1. Every command dialog (`MandateAuthoringWindow`, `ProposalArbitrationWindow`, `PersonalAttentionWindow`, `HqRelocationWindow`, `NotificationMatrixWindow`) sits inside `apps/ui-shell/src/hud/` and communicates strictly via `UiIntentBus` and `DiegeticStore`.
2. Reordering HUD layouts (for instance, moving the clock bar to a bottom tray or redesigning proposal cards) touches zero globe, window-manager, or Rust engine files.
