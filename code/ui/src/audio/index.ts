import { UiIntentBus } from "../spine";

/** Milestone 0 headless-safe audio controller listening strictly to `UiIntentBus`. */
export function mountAudioController(bus: UiIntentBus): () => void {
  const unsub = bus.subscribe((intent) => {
    if (intent.kind === "PlayUiFoley") {
      // AudioContext routing activates on first user gesture in M2
    }
  });
  return unsub;
}
