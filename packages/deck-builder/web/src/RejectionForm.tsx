// The typed rejection is the agent's only learning signal (spec §7.3), so
// every surface that asks for one — proposal items, audit dismissals, brief
// edits — asks through this one form. Shared rather than duplicated so the
// surfaces cannot drift into asking different questions, and the type routes
// the answer: a hard filter is forever, a playtest finding becomes a note.

import { useState } from "react";

export const REJECTION_TYPES = [
  { value: "hard_filter", label: "Hard filter — never suggest again" },
  { value: "thesis_change", label: "Thesis change — not what this deck is" },
  { value: "playtest_finding", label: "Playtest finding — tried it, know better" },
  { value: "soft", label: "Soft / not now" },
];

export function RejectionForm({
  placeholder,
  onConfirm,
}: {
  /** Each surface asks its own question; the type vocabulary stays shared. */
  placeholder: string;
  onConfirm: (type: string, reason: string) => void;
}) {
  const [type, setType] = useState("soft");
  const [reason, setReason] = useState("");
  return (
    <div className="row gap wrap reject-form">
      <select value={type} onChange={(e) => setType(e.target.value)}>
        {REJECTION_TYPES.map((t) => (
          <option key={t.value} value={t.value}>
            {t.label}
          </option>
        ))}
      </select>
      <input
        className="grow"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder={placeholder}
        autoFocus
      />
      <button className="small" disabled={!reason.trim()} onClick={() => onConfirm(type, reason)}>
        confirm
      </button>
    </div>
  );
}
