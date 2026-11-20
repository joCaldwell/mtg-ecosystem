import type { GameAdmission } from "../../../shared/contracts.ts";

export function admissionMessage(admission: GameAdmission): string {
  switch (admission.state) {
    case "not_checked": return "Check compatibility to start.";
    case "checking": return "Checking deck compatibility…";
    case "stale_revision": return "Your deck changed. Review it and check compatibility again.";
    case "unavailable": return "Games are unavailable right now. Try again later.";
    case "unsupported": return admission.code === "multiple_commanders" ? "This game profile does not support multiple commanders." : admission.code === "companion" ? "This game profile does not support companions." : "This deck is not supported by the selected game profile.";
    case "supported": return admission.launch_ready ? "This deck is ready." : "This deck is compatible. Live games are unavailable.";
  }
}
