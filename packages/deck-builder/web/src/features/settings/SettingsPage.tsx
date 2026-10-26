import { Advanced, useVoice } from "../../app/Preferences.tsx";
import { useEffect } from "react";
import { useSession } from "../../app/Session.tsx";
import { LegalFooter } from "../auth/index.ts";
import { AccountCard } from "./AccountCard.tsx";
import { AdminCard } from "./AdminCard.tsx";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { ErrorNote, Loading, Page, PageHeader, Text } from "../../ui/index.ts";
import { AgentConnectionCard } from "./AgentConnectionCard.tsx";
import { PriceSourceCard } from "./PriceSourceCard.tsx";
import { BudgetsCard } from "./BudgetsCard.tsx";
import { CardDataCard } from "./CardDataCard.tsx";
import styles from "./SettingsPage.module.css";

/** Connection details for agents, memory budgets, and card data (spec §10.1, §7.4). */
export function SettingsPage() {
  const voice = useVoice();
  const {session} = useSession();
  const settings = useQuery(() => api.getSettings(), []);
  const data = settings.data;
  useEffect(() => { const timer=setInterval(settings.reload,30_000); return () => clearInterval(timer); },[settings.reload]);

  return (
    <Page
      width="md"
      header={<PageHeader title="Settings" subtitle={voice("Your account, card prices and optional agent connections.", "How agents reach this app, and what they are handed when they do.")} />}
    >
      {settings.error && !data && <ErrorNote>{settings.error}</ErrorNote>}
      {settings.loading && <Loading />}
      {data && (
        <div className={styles.cards}>
          <AgentConnectionCard agent={data.agent} onChanged={settings.reload} />
          <PriceSourceCard source={data.price_source} onSaved={settings.reload} />
          <Advanced><BudgetsCard budgets={data.budgets} onSaved={settings.reload} /></Advanced>
          <CardDataCard cards={data.cards} jobs={data.jobs} onChanged={settings.reload} canRefresh={session.mode === "local" || !!session.user?.is_admin} />
          {session.mode === "hosted" && <AccountCard />}
          {session.mode === "hosted" && !!session.user?.is_admin && <AdminCard />}
          {session.mode === "hosted" && <LegalFooter asOf={data.cards.prices.as_of} />}
          <Text tone="faint" size="sm">
            Colours, type and every component are on the <a href={href({ page: "design" })}>design reference</a>.
          </Text>
        </div>
      )}
    </Page>
  );
}
