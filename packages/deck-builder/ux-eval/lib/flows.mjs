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
    name: "search-query-switch",
    caption: "Switch card searches while a response is pending, then inspect the current match",
    width: 1440,
    user: u => u.owner,
    async run(t) {
      const { page } = t, id = t.decks.najeela;
      const before = (await t.api(`/api/decks/${id}`, t.users.owner)).state;
      const [first, second] = before.cards.filter(c => !c.maybe && c.role === "card").slice(0, 2);
      if (!first || !second) throw Error("Search fixture needs two saved cards");
      await page.goto(t.origin + `/deck/${id}`);
      const input = page.getByRole("combobox", { name: "Add a card or search", exact: true });
      await t.press(input);
      await t.type(input, first.name);
      await page.getByRole("option").filter({ has: page.getByText(first.name, { exact: true }) }).waitFor();
      await t.step("Search for a saved card");
      let release, requestedResolve;
      const held = new Promise(resolve => { release = resolve; });
      const requested = new Promise(resolve => { requestedResolve = resolve; });
      await page.route("**/api/search?**", async route => { requestedResolve(); await held; await route.continue(); }, { times: 1 });
      try {
        await t.type(input, second.name);
        await requested;
        await t.key(input, "Enter");
        await t.step("Press Enter while the next search response is pending");
        const previousOpened = await page.getByRole("heading", { name: first.name, exact: true }).count() > 0;
        release();
        await t.press(input);
        await page.getByRole("option").filter({ has: page.getByText(second.name, { exact: true }) }).waitFor();
        await t.key(input, "Enter");
        await page.getByRole("heading", { name: second.name, exact: true }).waitFor();
        await t.step("Open the current search match after its response arrives");
        const after = (await t.api(`/api/decks/${id}`, t.users.owner)).state;
        const unchanged = JSON.stringify(before) === JSON.stringify(after);
        return { completed: !previousOpened && unchanged, facts: { previous_match_not_opened_during_loading: !previousOpened, current_match_opened: true, deck_unchanged: unchanged } };
      } finally { release(); }
    },
  },
  {
    name: "export-filtered-maybe",
    caption: "Filter a maybe card and copy its export without editing the deck",
    width: 1440,
    user: u => u.owner,
    async run(t) {
      const {page} = t, id = t.decks.najeela, before = (await t.api(`/api/decks/${id}`, t.users.owner)).state;
      const card = before.cards.find(card => card.maybe);
      if (!card) throw Error("Export fixture needs a maybe card");
      await page.context().grantPermissions(["clipboard-read", "clipboard-write"], { origin: t.origin });
      await page.goto(t.origin + `/deck/${id}`);
      await t.type(page.getByRole("searchbox", { name: "Search this deck", exact: true }), card.name);
      await t.settle();
      await t.press(page.getByRole("button", { name: "More", exact: true }));
      await t.press(page.getByRole("menuitem", { name: "Export", exact: true }));
      const dialog = page.getByRole("dialog", { name: "Export", exact: true });
      await t.step("Open an export for the current maybe-card filter");
      const initiallyEmpty = await dialog.getByRole("button", { name: "Copy", exact: true }).isDisabled();
      await t.press(dialog.getByRole("checkbox", { name: "Include maybe list", exact: true }));
      const field = dialog.getByRole("textbox", { name: "Exported list", exact: true });
      await field.waitFor();
      await t.step("Include the matching maybe card");
      await t.press(dialog.getByRole("button", { name: "Copy", exact: true }));
      await t.step("Copy the filtered export");
      const oracleIds = before.cards.filter(c => c.name.toLowerCase().includes(card.name.toLowerCase())).map(c => c.oracle_id).sort().join(",");
      const result = await t.api(`/api/decks/${id}/export?categories=off&include_maybe=on&oracle_ids=${oracleIds}`, t.users.owner);
      const textMatches = await field.inputValue() === result.text;
      const copied = await page.evaluate(() => navigator.clipboard.readText()) === result.text;
      const after = (await t.api(`/api/decks/${id}`, t.users.owner)).state;
      const unchanged = JSON.stringify(after) === JSON.stringify(before);
      return { completed: initiallyEmpty && textMatches && copied && unchanged, facts: { fixture_card: card.name, cards: result.card_count, lines: result.line_count, initially_excludes_maybe: initiallyEmpty, exact_capability_result: textMatches, exact_clipboard: copied, deck_unchanged: unchanged } };
    },
  },
  {
    name: "playtest-draw-undo-resume",
    caption: "Keep a hand, draw, undo and resume the saved manual table",
    width: 1440,
    user: (u) => u.owner,
    async run(t) {
      const { page } = t, id = t.decks.najeela;
      const before = (await t.api(`/api/decks/${id}`, t.users.owner)).state;
      await page.goto(t.origin + `/deck/${id}/playtest`);
      await page.getByRole("dialog", { name: "New game", exact: true }).waitFor();
      await t.step("Open playtest from a saved deck");
      await t.press(page.getByRole("button", { name: "Shuffle and draw seven", exact: true }));
      await page.getByRole("region", { name: "Opening hand", exact: true }).waitFor();
      await t.step("Shuffle and draw seven");
      await t.press(page.getByRole("button", { name: "Keep hand", exact: true }));
      const draw = page.getByRole("button", { name: /^Draw(?:\s|$)/ }); await draw.waitFor();
      await t.step("Keep the opening hand");
      const read = async () => (await t.api(`/api/decks/${id}/playtest`, t.users.owner)).playtest;
      const kept = await read();
      const handCard = page.getByRole("region", { name: "Hand", exact: true }).locator("[data-object-id]").first();
      await handCard.focus(); await t.key(handCard, "Enter"); await t.settle();
      const selected = await handCard.getAttribute("data-selected") === "true";
      await t.key(handCard, "Shift+F10"); await page.getByRole("menu").waitFor();
      await t.step("Select a hand card and open its keyboard action menu");
      await page.keyboard.press("Escape"); await t.settle();
      await t.press(draw); await t.settle();
      await t.step("Draw one card"); const drawn = await read();
      await t.press(page.getByRole("button", { name: "Undo (Z)", exact: true })); await t.settle();
      await t.step("Undo the draw"); const undone = await read();
      await page.reload(); await draw.waitFor(); await t.settle();
      await t.step("Reload and resume the saved table"); const resumed = await read();
      const after = (await t.api(`/api/decks/${id}`, t.users.owner)).state;
      const drawnOne = drawn.computed.counts.hand === kept.computed.counts.hand + 1;
      const restored = JSON.stringify(undone.session.frame.zones) === JSON.stringify(kept.session.frame.zones);
      const persisted = resumed.session.id === kept.session.id && JSON.stringify(resumed.session.frame.zones) === JSON.stringify(undone.session.frame.zones);
      const unchanged = after.deck.revision === before.deck.revision && JSON.stringify(after.cards) === JSON.stringify(before.cards);
      return { completed: drawnOne && restored && persisted && unchanged && selected, facts: { keyboard_card_selected: selected, opening_hand: kept.computed.counts.hand, drew_one: drawnOne, exact_zone_order_restored: restored, session_resumed: persisted, deck_unchanged: unchanged } };
    },
  },
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
      await t.press(control(page, /^Import deck$/));
      await t.step("Click “Import deck”");
      const name = page.getByRole("textbox", { name: /deck name/i });
      await t.press(name);
      await t.keys(name, "Kinnan from Moxfield");
      await t.step("Type a deck name");
      const box = page.getByRole("textbox", { name: /list to import/i });
      await t.press(box);
      await t.type(box, list);
      await t.step("Paste the Moxfield “Copy Plain Text” export");
      await t.press(control(page, /^Preview/));
      await t.step("Click “Preview”");
      await t.press(page.getByRole("button", { name: /^Apply the import/ }));
      await page.waitForURL(/\/deck\/\d+/, { timeout: 10000 });
      await t.step("Click “Apply the import”");
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
