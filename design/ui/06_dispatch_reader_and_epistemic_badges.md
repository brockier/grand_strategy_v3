# Module Design: Hyperlinked Dispatch Reader & Epistemic Provenance Badges

**Module ID:** `UI-DIS-06`  
**Code Location:** `code/ui/src/dispatches/`  
**Milestone & Design Depth:** `M4: Full Detailed Design`  
**Satisfies Requirements:** `UI-DAT-004`, `UI-DAT-005` in [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `DiegeticStore` (`selectDispatchInbox`, `selectKnownCells`, `selectLedgerRows`)
  - `AssetUrlResolverPort` (dispatch header illustrations, wax seal icons, character portraits)
- **Provided Spine Ports (Outputs):**
  - Registered `DispatchReaderWindow` `WindowContentPlugin`
  - Shared pure `EpistemicProvenanceFormatter` utility exported via `src/spine/` contracts for `UI-DAT-005`
  - Emits `FocusEntityInspector`, `FlyGlobeToCell`, and `PlayUiFoley` intents onto `UiIntentBus`

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Structured Dispatch Token AST (`UI-DAT-004`)

To support safe, moddable rich hyperlinks without HTML injection risks (`innerHTML` forbidden):

```typescript
export type DispatchToken =
  | { kind: "Text"; text: string }
  | { kind: "EntityLink"; entityKind: EntityKind; entityId: number; label: string; cellId?: number }
  | { kind: "NumericMetric"; value: EpistemicNumericValue };

export interface EpistemicNumericValue {
  pointEstimate: number;
  minEstimate?: number;
  maxEstimate?: number;
  unit: string;
  observedTick: number;
  isDirectHqObservation: boolean;
  authorCharacterId?: number;
  authorDisplayName?: string;
}

export interface DispatchDocument {
  dispatchId: number;
  title: string;
  sentTick: number;
  arrivedTick: number;
  originCellId: number;
  headerPlateKey: string;
  senderCharacterId: number;
  tokens: readonly DispatchToken[];
  read: boolean;
}
```

---

## 3. Algorithms, Safe Hyperlinking & Epistemic Provenance Formatting

### 3.1 Safe Structured Token Renderer (`UI-DAT-004`)
1. `player-view` compiles parameterized localization templates into `DispatchToken[]` arrays—never raw HTML strings from user mods.
2. `DispatchReaderWindow` renders:
   - `Text` tokens as plain text spans.
   - `EntityLink` tokens as interactive underlined/chip buttons decorated with skin-defined seal/glyph markers:
     - **Primary Click:** Emits `FocusEntityInspector { entityKind, entityId, pinned: false }` (and emits `FlyGlobeToCell` when clicking coordinate or settlement badges).
     - **Shift-Click or Map Pin Sub-Button:** Opens a pinned side-by-side comparison inspector or centers the globe camera directly onto `cellId` without losing the open dispatch reader.

### 3.2 Standardized Epistemic Provenance Badges (`UI-DAT-005`)
Every numeric figure originating outside immediate HQ sight renders through one consistent provenance component:
1. If `isDirectHqObservation === true`: displays crisp numeric string plus badge `"Direct Observation (HQ)"`.
2. Otherwise:
   - When `minEstimate !== undefined && maxEstimate !== undefined`: formats `"~42,000 kg [28,000 .. 61,000]"` (widened cleanly for sloppy or novice reports).
   - Attaches subtle age indicator (`"14d 6h old"`) styled by staleness tier CSS classes (`.provenance-fresh`, `.provenance-aging`, `.provenance-stale`).
   - Hover tooltip or click link reveals `"Reported Tick <T> by <AuthorLink>"` where clicking `<AuthorLink>` opens the reporting official's character sheet (exposing their known competence and honesty/precision reputation!).

---

## 4. Isolated Cartridge Verification Strategy

1. **Hyperlinked Entity Routing (`@req UI-DAT-004`):** Playwright loads a dispatch fixture containing a `SettlementId` link and a `CharacterId` link, clicks both, and verifies `UiIntentBus` emits exact `FlyGlobeToCell` and `FocusEntityInspector` intents while opening the target character window.
2. **HTML Injection Safety:** Feeds raw `<script>` inside mock dispatch text tokens and verifies literal escaped text rendering with zero DOM script tags.
3. **Provenance & Confidence Display (`@req UI-DAT-005`):** Vitest asserts remote mineral and stock values always display `[min .. max]` intervals, age duration, and authoring subordinate citation while live HQ entries display `"Direct Observation"`.

---

## 5. Lift-and-Shift Replacement Guide

1. Swapping the visual layout (for instance, turning dispatches into parchment scrolls, printed telegram strips, or side-by-side archive binders) changes strictly `code/ui/src/dispatches/` and external skin CSS.
2. `DispatchToken` schema inside `src/spine` keeps `player-view` IPC completely decoupled from frontend rendering choices.
