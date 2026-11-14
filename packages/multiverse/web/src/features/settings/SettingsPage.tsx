import { useEffect } from "react";
import { useSession } from "../../app/Session.tsx";
import { LegalFooter } from "../auth/index.ts";
import { AccountCard } from "./AccountCard.tsx";
import { AdminCard } from "./AdminCard.tsx";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { ErrorNote, Loading, Page, PageHeader } from "../../ui/index.ts";
import { AgentConnectionCard } from "./AgentConnectionCard.tsx";
import { PriceSourceCard } from "./PriceSourceCard.tsx";
import { BudgetsCard } from "./BudgetsCard.tsx";
import { CardDataCard } from "./CardDataCard.tsx";
import styles from "./SettingsPage.module.css";

/** Connection details for agents, memory budgets, and card data (spec §10.1, §7.4). */
export function SettingsPage() {
  const {session} = useSession();
  const settings = useQuery(() => api.getSettings(), [], "settings");
  const data = settings.data;
  useEffect(() => { const timer=setInterval(settings.reload,30_000); return () => clearInterval(timer); },[settings.reload]);

  return (
    <Page
      width="lg"
      header={<PageHeader title="Settings" />}
    >
      {settings.error && !data && <ErrorNote>{settings.error}</ErrorNote>}
      {settings.loading && <Loading />}
      {data && (
        <div className={styles.cards}>
          <AgentConnectionCard agent={data.agent} onChanged={settings.reload} />
          <div className={styles.pair}>
          <PriceSourceCard source={data.price_source} onSaved={settings.reload} />
          <BudgetsCard budgets={data.budgets} usage={data.memory_usage} onSaved={settings.reload} />
          </div>
          <div className={styles.pair}>
          <CardDataCard cards={data.cards} jobs={data.jobs} onChanged={settings.reload} canRefresh={session.mode === "local" || !!session.user?.is_admin} />
          {session.mode === "hosted" && <AccountCard />}
          </div>
          {session.mode === "hosted" && !!session.user?.is_admin && <AdminCard />}
          {session.mode === "hosted" && <LegalFooter asOf={data.cards.prices.as_of} />}
        </div>
      )}
    </Page>
  );
}
