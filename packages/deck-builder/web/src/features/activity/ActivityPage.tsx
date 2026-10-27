import { Page, PageHeader } from "../../ui/index.ts";
import { ActivityPanel } from "./ActivityPanel.tsx";

/** Standalone audit trail, restored following review on 2026-10-27 (§11). */
export function ActivityPage() {
  return (
    <Page width="lg" header={<PageHeader title="Activity" subtitle="Agent writes, grouped by session. Your own edits are in each deck’s History." />}>
      <ActivityPanel />
    </Page>
  );
}
