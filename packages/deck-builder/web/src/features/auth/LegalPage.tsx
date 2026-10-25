import { RichText } from "../../mtg/index.ts";
import { Page, PageHeader, Stack } from "../../ui/index.ts";
import { LegalFooter } from "./LegalFooter.tsx";
const privacy = `## Privacy policy
Operator: <OPERATOR NAME>. Contact: <CONTACT EMAIL>.

We store your provider account identifier, display name, email and optional avatar through Better Auth; your chosen handle; decks, cards, notes, memories, proposals, decisions, activity, settings and saved playtest state; personal-token hashes; sessions and OAuth grants. Provider credentials are used to complete sign-in. We use this data to operate your account and authorize the agents you connect. We make no AI calls and sell no personal data.

Essential HttpOnly session cookies keep you signed in. A short-lived registration cookie carries your invite or claim intent through sign-in. No advertising or analytics cookies are used. Your browser sends IP address and request headers to the service; authentication may store session IP/user-agent metadata. Rate limits use temporary in-memory identifiers.

Hosted infrastructure is provided by Railway. Discord or Google handles identity under its own policy. Your browser fetches card images directly from Scryfall, exposing your IP address and request metadata to Scryfall. Agents you connect receive data you authorize them to read; their providers have their own policies.

Data is kept until you delete it or your account. Settings offers a JSON account export, including Archidekt deck text, and permanent account deletion. Deletion removes service records and credentials immediately. Local backups expire after 14 days; off-volume backups follow the operator’s configured bucket retention. Restoring an older backup may require repeating deletion. Contact the operator for assistance. The final administrator must arrange another administrator before deleting their account.

This policy may change as the service evolves. Contact the operator with privacy questions. Updated 2026-10-25.`;
const terms = `## Terms of use
Operator: <OPERATOR NAME>. Contact: <CONTACT EMAIL>.

This free, invite-only deck-building tool is provided as is, without warranties of availability, accuracy, fitness or uninterrupted service. Keep exports of work you cannot afford to lose. Card prices are daily estimates, not offers, valuations or guarantees. You remain responsible for checking card rules and deck legality.

You retain rights to your content. You permit the operator to store and process it solely to provide the service, including delivering it to agents you authorize. Keep credentials private, connect only agents you trust, and do not abuse the service or attempt to access another account. The operator may suspend abusive access and contact you about service operation.

You may export your data and delete your account from Settings. Deletion and backup retention are described in the privacy policy. This service is not affiliated with or endorsed by Wizards of the Coast, Scryfall, Discord, Google, Anthropic or OpenAI. Magic: The Gathering and related marks belong to their owners. Use is subject to the Wizards of the Coast Fan Content Policy. Updated 2026-10-25.`;
export function LegalPage({page}:{page:"privacy"|"terms"}) {
  return <Page width="md" header={<PageHeader title={page === "privacy" ? "Privacy" : "Terms"} />}><Stack gap={6}><RichText text={page === "privacy" ? privacy : terms} /><a href="/">Back to deck builder</a><LegalFooter /></Stack></Page>;
}
