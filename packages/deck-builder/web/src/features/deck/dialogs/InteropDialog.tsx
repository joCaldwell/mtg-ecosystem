// Archidekt interop (spec §12). Deck construction happens here, playtesting
// happens in Archidekt — one-way out, one-way back. The way back applies in
// one shot: the preview diff IS the approval step, and the import lands as a
// single undoable entry in History.
//
// Only the open panel is mounted (the dialog's Tab trap counts every focusable
// element in it, hidden or not), so the pasted list and its preview live here
// and survive a look at the export.

import { useState } from "react";
import { Button, Dialog, Tabs } from "../../../ui/index.ts";
import { ExportPanel } from "./ExportPanel.tsx";
import { ImportPanel, type ImportDraft } from "./ImportPanel.tsx";

type Tab = "export" | "import";

export function InteropDialog({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("export");
  const [draft, setDraft] = useState<ImportDraft>({ paste: "", note: "", diff: null });
  return (
    <Dialog
      title="Archidekt"
      width="lg"
      onClose={onClose}
      footer={<Button onClick={onClose}>Close</Button>}
    >
      <Tabs
        aria-label="Archidekt"
        tabs={[
          { id: "export", label: "Export" },
          { id: "import", label: "Import" },
        ]}
        value={tab}
        onChange={(id) => setTab(id === "import" ? "import" : "export")}
      />
      <div role="tabpanel">
        {tab === "export" ? (
          <ExportPanel />
        ) : (
          <ImportPanel draft={draft} onDraft={(patch) => setDraft((d) => ({ ...d, ...patch }))} onClose={onClose} />
        )}
      </div>
    </Dialog>
  );
}
