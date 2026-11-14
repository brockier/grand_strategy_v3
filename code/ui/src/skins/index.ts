import { SkinManifestBundle } from "../spine";

/** Applies an external CSS skin bundle atomically via `<link id="active-skin-css">`. */
export function applySkinBundle(bundle: SkinManifestBundle): void {
  const existing = document.getElementById("active-skin-css") as HTMLLinkElement | null;
  if (existing) {
    existing.href = bundle.stylesheetHref;
    return;
  }
  const link = document.createElement("link");
  link.id = "active-skin-css";
  link.rel = "stylesheet";
  link.href = bundle.stylesheetHref;
  document.head.appendChild(link);
}
