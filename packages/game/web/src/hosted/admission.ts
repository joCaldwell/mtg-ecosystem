import type { GameAdmission } from "../../../shared/contracts.ts";

export function admissionMessage(admission: GameAdmission): string {
  switch (admission.state) {
    case "not_checked": return "Deck compatibility has not been checked.";
    case "checking": return "Checking this deck revision with the game server…";
    case "stale_revision": return `Your deck changed to revision ${admission.current_revision}. Review the current deck and check again.`;
    case "unavailable": return admission.reason === "native_unconfigured" ? "Game admission is not configured on this server." : admission.reason === "identity_mismatch" ? "The game server's catalog or build does not match this configuration." : admission.reason === "session_incomplete" ? "Live game controls are not qualified yet." : "The game server is unavailable. Try checking again.";
    case "unsupported": return admission.code === "multiple_commanders" ? "This game profile does not support multiple commanders." : admission.code === "companion" ? "This game profile does not support companions." : "This deck is not supported by the selected game profile.";
    case "supported": return admission.launch_ready ? "This exact deck revision is admitted for a game." : "This deck revision is admitted. Live sessions and automated opponents are not connected yet.";
  }
}
