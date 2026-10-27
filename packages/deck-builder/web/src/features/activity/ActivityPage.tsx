import { useVoice } from "../../app/Preferences.tsx";
import { Page, PageHeader } from "../../ui/index.ts";
import { ActivityPanel } from "./ActivityPanel.tsx";

/** Standalone audit trail, restored following review on 2026-10-27 (§11). */
export function ActivityPage() {
  const voice = useVoice();
  return (
    <Page width="lg" header={<PageHeader title="Activity" subtitle={voice("An optional activity trail for help you connect to your decks. Your own edits appear in each deck’s History.", "Every write an agent has made, grouped by session.")} />}>
      <ActivityPanel />
    </Page>
  );
}
