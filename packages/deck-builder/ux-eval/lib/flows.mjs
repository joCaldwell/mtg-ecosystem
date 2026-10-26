// The task flows. Each is a sequence of real user actions, with a screenshot
// after each one and a caption that says what was done (never why). Elements
// are found by role and accessible name, and a step looks for a better path
// when one exists (an import entry on All decks, a reason field for
// accepting), so the flows keep working while the UI improves.
//
// run(t) returns { completed, facts }: whether the task's outcome is really in
// the database, read back through the API afterwards.

import { readFileSync } from "node:fs";
import { join } from "node:path";

/** A control by its accessible name, whatever its role. */
function control(page, name) {
  return page.getByRole("button", { name })
    .or(page.getByRole("link", { name }))
    .or(page.getByRole("menuitem", { name }))
    .or(page.getByRole("tab", { name }))
    .or(page.getByRole("radio", { name }))
    .first();
}
async function exists(locator, timeout = 1500) {
  try { await locator.waitFor({ state: "visible", timeout }); return true; } catch { return false; }
}
/** The first element after the text `anchor` (in document order) that matches the XPath test. */
function after(page, anchor, test) {
  return page.locator(`xpath=(//*[normalize-space(text())=${JSON.stringify(anchor)}])[last()]/following::*[${test}][1]`);
}
const ACCEPT = `(self::button or @role='button') and starts-with(normalize-space(.),'Accept') and not(contains(.,'all'))`;
const REASON = `(self::input or self::textarea) and contains(translate(concat(@aria-label,' ',@placeholder),'R','r'),'reason') and not(contains(@aria-label,'reject')) and not(contains(@aria-label,'everything'))`;

async function latestDecision(t, deckId) {
  const [entry] = await t.api(`/api/log?deck=${deckId}&limit=1`, t.users.owner);
  return entry;
}

export const FLOWS = [
  {
    name: "first-run-sample",
    caption: "From a new, empty account to the sample deck",
    width: 1280,
    user: (u) => u.empty,
    async run(t) {
      await t.page.goto(t.origin + "/");
      await t.step("Open the app");
      await t.press(control(t.page, /^Try the sample/));
      await t.page.waitForURL(/\/deck\/\d+/, { timeout: 10000 }).catch(() => {});
      await t.step("Click “Try the sample deck”");
      const decks = await t.api("/api/decks", t.users.empty);
      return { completed: /\/deck\/\d+/.test(t.page.url()) && decks.length === 1, facts: { decks_after: decks.length, landed_on_deck: /\/deck\/\d+/.test(t.page.url()) } };
    },
  },
  {
    name: "import-moxfield",
    caption: "Import a Moxfield list into a new deck",
    width: 1280,
    user: (u) => u.owner,
    async run(t) {
      const { page } = t;
      const list = readFileSync(join(t.PKG, "test/fixtures/moxfield/kinnan-midrange.copy-plain-text.txt"), "utf8");
      await page.goto(t.origin + "/");
      await t.step("Open All decks");
      const direct = control(page, /^Import/);
      if (await exists(direct, 800)) {
        await t.press(direct);
        await t.step("Click “Import”");
      } else {
        await t.press(control(page, /^New deck/));
        await t.step("Click “New deck”");
      }
      const name = page.getByRole("textbox", { name: /deck name/i });
      await t.press(name);
      await t.keys(name, "Kinnan from Moxfield");
      await t.step("Type a deck name");
      await t.press(control(page, /^(Create and import|Create deck|Create)/));
      await t.page.waitForURL(/\/deck\/\d+/, { timeout: 10000 });
      await t.step("Click “Create deck”");
      let box = page.getByRole("textbox", { name: /list to import/i });
      if (!(await exists(box, 1200))) {
        await t.press(control(page, /^More/));
        await t.step("Open the “More” menu");
        await t.press(page.getByRole("menuitem", { name: /import/i }));
        await t.step("Choose “Import / export”");
        if (!(await exists(box, 800))) {
          await t.press(page.getByRole("tab", { name: /^Import/ }));
          await t.step("Choose the Import tab");
        }
      }
      await t.press(box);
      await t.type(box, list);
      await t.step("Paste the Moxfield “Copy Plain Text” export");
      await t.press(control(page, /^Preview/));
      await t.step("Click “Preview”");
      await t.press(page.getByRole("button", { name: /^Apply the import/ }).last());
      await page.waitForTimeout(300);
      if (await exists(page.getByText(/^Cut \d+ cards? from this deck/), 500)) {
        await t.step("Click “Apply the import”");
        await t.press(page.getByRole("button", { name: /^Apply the import/ }).last());
        await t.step("Confirm “Apply the import”");
      } else await t.step("Click “Apply the import”");
      const close = page.getByRole("dialog").getByRole("button", { name: /^Close/ }).first();
      if (await exists(close, 800)) {
        await t.press(close);
        await t.step("Close the dialog");
      }
      const deck = (await t.api("/api/decks", t.users.owner)).find((d) => d.name === "Kinnan from Moxfield");
      return {
        completed: !!deck && deck.card_count >= 90 && deck.commanders.length > 0,
        facts: { deck_created: !!deck, card_count: deck?.card_count ?? 0, commander_set: (deck?.commanders.length ?? 0) > 0 },
      };
    },
  },
  {
    name: "review-accept-with-reason",
    caption: "Review one agent proposal and accept it with a reason",
    width: 1440,
    user: (u) => u.owner,
    async run(t) {
      const { page } = t;
      const deckId = t.decks.najeela;
      const before = await latestDecision(t, deckId);
      await page.goto(t.origin + "/inbox");
      await t.step("Open the inbox");
      await t.press(control(page, /Protect the combo turn/));
      await t.step("Click the proposal “Protect the combo turn”");
      const review = control(page, /^Review in the list/);
      if (await exists(review, 1500)) {
        await t.press(review);
        await page.waitForURL(/\/deck\/\d+/, { timeout: 10000 }).catch(() => {});
        await t.step("Click “Review in the list”");
      }
      const accept = after(page, "Protect the combo turn", ACCEPT);
      const reason = after(page, "Protect the combo turn", REASON);
      await t.scroll(accept);
      await t.step("Scroll to the proposal “Protect the combo turn”");
      let reasonTyped = false;
      if (await exists(reason, 800)) {
        await t.press(reason);
        await t.keys(reason, "Pyroblast also covers the blue matchups.");
        reasonTyped = true;
        await t.step("Type a reason");
      }
      await t.press(accept);
      await t.step("Click “Accept”");
      const entry = await latestDecision(t, deckId);
      const accepted = entry.id !== before.id && entry.kind === "accept";
      const reasoned = accepted && (entry.decision_reason ?? "").trim().length > 0;
      return { completed: accepted && reasoned, facts: { accepted, reason_field_found: reasonTyped, reason_recorded: reasoned } };
    },
  },
  {
    name: "phone-accept",
    caption: "Accept an agent proposal on a phone",
    width: 375,
    user: (u) => u.owner,
    async run(t) {
      const { page } = t;
      const deckId = t.decks.kinnan;
      const before = await latestDecision(t, deckId);
      await page.goto(t.origin + "/inbox");
      await t.step("Open the inbox");
      await t.press(control(page, /Card selection over a tax piece/));
      await t.step("Tap the proposal “Card selection over a tax piece”");
      const accept = after(page, "Card selection over a tax piece", ACCEPT);
      const reason = after(page, "Card selection over a tax piece", REASON);
      let reasonTyped = false;
      if (await exists(reason, 1000)) {
        await t.press(reason);
        await t.keys(reason, "Worth testing for a week.");
        reasonTyped = true;
        await t.step("Type a reason");
      }
      await t.press(accept);
      await t.step("Tap “Accept”");
      const entry = await latestDecision(t, deckId);
      const accepted = entry.id !== before.id && entry.kind === "accept";
      return { completed: accepted, facts: { accepted, reason_field_found: reasonTyped, reason_recorded: accepted && (entry.decision_reason ?? "").trim().length > 0 } };
    },
  },
  {
    name: "share-public",
    caption: "Share a deck publicly and open its public page",
    width: 1280,
    user: (u) => u.owner,
    async run(t) {
      const { page } = t;
      await page.goto(t.origin + `/deck/${t.decks.bombadil}`);
      await t.step("Open the deck “Bombadil Enchantments”");
      await t.press(control(page, /^Share/));
      await t.step("Click “Share”");
      await t.press(control(page, /^Public/));
      await t.step("Choose “Public”");
      const view = control(page, /what viewers see|public page|open.*page/i);
      const [opened] = await Promise.all([
        page.context().waitForEvent("page", { timeout: 5000 }).catch(() => null),
        t.press(view),
      ]);
      const target = opened ?? page;
      if (opened) await opened.waitForLoadState("load");
      await t.step("Click “See what viewers see”", target);
      const deck = (await t.api("/api/decks", t.users.owner)).find((d) => d.id === t.decks.bombadil);
      const ok = deck.visibility === "public" && /\/d\//.test(target.url());
      return { completed: ok, facts: { visibility: deck.visibility, public_page_opened: /\/d\//.test(target.url()), new_tab: !!opened } };
    },
  },
  {
    name: "maybe-card-details",
    caption: "Find a card, add it to the maybe list, then open its details",
    width: 1440,
    user: (u) => u.owner,
    async run(t) {
      const { page } = t;
      await page.goto(t.origin + `/deck/${t.decks.kinnan}`);
      await t.step("Open the deck “Kinnan Midrange”");
      const search = page.getByRole("combobox", { name: /add a card/i }).or(page.getByPlaceholder(/add a card/i)).first();
      await t.press(search);
      await t.keys(search, "Mystic Snake");
      await t.step("Type “Mystic Snake” in the add bar");
      const maybeButton = page.getByRole("option", { name: /Mystic Snake/ }).getByRole("button", { name: /maybe/i });
      if (await exists(maybeButton, 800)) {
        await t.press(maybeButton);
        await t.step("Click “Add to maybe list”");
      } else {
        await t.key(search, "Shift+Enter");
        await t.step("Press Shift+Enter");
      }
      const row = page.getByRole("region", { name: /maybe list/i }).getByRole("button", { name: /^Mystic Snake/ }).first();
      await t.scroll(row);
      await t.step("Scroll to the maybe list");
      await t.press(row);
      await t.step("Click “Mystic Snake” in the maybe list");
      const deck = await t.api(`/api/decks/${t.decks.kinnan}`, t.users.owner);
      const onMaybe = deck.state.cards.some((c) => c.name === "Mystic Snake" && c.maybe);
      const panel = await page.getByRole("heading", { name: /^Mystic Snake/ }).count();
      return { completed: onMaybe && panel > 0, facts: { on_maybe_list: onMaybe, details_open: panel > 0 } };
    },
  },
];
