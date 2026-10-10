import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { ErrorNote, Loading, Page, PageHeader, Text } from "../../ui/index.ts";
import { AgentConnectionCard } from "./AgentConnectionCard.tsx";
import { BudgetsCard } from "./BudgetsCard.tsx";
import { CardDataCard } from "./CardDataCard.tsx";
import styles from "./SettingsPage.module.css";

/** Connection details for agents, memory budgets, and card data (spec §10.1, §7.4). */
export function SettingsPage() {
  const settings = useQuery(() => api.getSettings(), []);
  const data = settings.data;

  return (
    <Page
      width="md"
      header={<PageHeader title="Settings" subtitle="How agents reach this app, and what they are handed when they do." />}
    >
      {settings.error && !data && <ErrorNote>{settings.error}</ErrorNote>}
      {settings.loading && <Loading />}
      {data && (
        <div className={styles.cards}>
          <AgentConnectionCard agent={data.agent} onChanged={settings.reload} />
          <BudgetsCard budgets={data.budgets} onSaved={settings.reload} />
          <CardDataCard cards={data.cards} onChanged={settings.reload} />
          <Text tone="faint" size="sm">
            Colours, type and every component are on the <a href={href({ page: "design" })}>design reference</a>.
          </Text>
        </div>
      )}
    </Page>
  );
}
