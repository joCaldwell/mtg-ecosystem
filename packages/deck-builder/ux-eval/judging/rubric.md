# Rubric

Score each category from 0 to 10. The anchors describe a 3, a 6 and a 9 *in
this app*; interpolate between them. Score what the screenshots show, across
every screen and viewport in the packet, not the best screen alone.

"Watch for" items are reported concerns about this app. Check each one
against what you see. They are hypotheses, not verdicts: if the screenshots
show the concern does not hold, say so and do not mark it down.

Category ids (used in the JSON outputs) are in brackets.

## 1. Visual consistency [`visual_consistency`]

Buttons, links, headings, spacing and controls look and behave alike
wherever they mean the same thing.

- **3**: The same action looks different on different screens (a filled
  button here, an underlined link there, a bordered chip elsewhere).
  Heading sizes and weights vary without reason. Spacing differs between
  similar panels.
- **6**: One button family and one link style dominate, with a few
  exceptions (a lone underlined text action among buttons, a header that
  uses a different size than its siblings).
- **9**: Primary, secondary and destructive actions are each one recognisable
  style everywhere; links are only for navigation; heading levels, eyebrows
  and spacing follow a visible system across pages.

Watch for: inconsistent buttons, links and headers (for example a header
mixing link-styled actions with buttons, such as Playtest and Share beside
More); a placeholder-looking logo.

## 2. Visual hierarchy and layout [`hierarchy_layout`]

What matters most is seen first; regions are clearly separated; pages use
their space well.

- **3**: The eye has nowhere to land; toolbars, banners and lists blur into
  one band; secondary pages are a column of loosely related boxes.
- **6**: The deck list reads clearly, but some regions run together (for
  example the search and filter row and the list beneath it), and some pages
  feel unarranged.
- **9**: Every page has an obvious primary region and a clear order;
  toolbars, banners and content are distinct at a glance; non-editor pages
  (inbox, memories, activity, settings, profile, all decks) are composed as
  deliberately as the deck editor.

Watch for: weak separation between the filter/search row and the deck list;
the layout of every page other than the deck builder; the navigation bar's
layout.

## 3. Colour and contrast [`colour_contrast`]

Colour carries the meanings in the brief and nothing else; text is legible.

- **3**: Colours are decorative or contradictory (green used for neutral
  things, violet on owner actions); low-contrast text; surfaces that do not
  separate.
- **6**: Meanings are mostly respected and text is readable, but the palette
  feels muddy or flat, surface steps are too subtle, or a few uses of colour
  carry no meaning.
- **9**: Agent violet, add green, cut red, warn amber and selection blue are
  instantly legible and never misused; greys form clear surface layers;
  contrast is comfortable for long sessions.

Watch for: whether the colour system needs rethinking, possibly toward a
true-black theme. Judge whether the current palette serves long sessions,
not whether it is black.

## 4. Information density and text economy [`density_text_economy`]

Dense where density helps (the card list), economical with words everywhere.

- **3**: Paragraphs of explanation on every screen; repeated helper text;
  lists so padded that a deck needs several screens, or so cramped that
  rows collide.
- **6**: The list is dense and scannable, but pages carry subtitles,
  explainers and hints that restate the obvious.
- **9**: Each screen says what it needs once, in few words; help is
  available on demand rather than always shown; the card list shows a
  whole deck's structure in one or two screens on desktop.

Watch for: too much descriptive text (page subtitles, explainers, hints
under controls).

## 5. Navigation and wayfinding [`navigation_wayfinding`]

The owner always knows where they are and how to get elsewhere.

- **3**: Unclear current location; important places (inbox, a deck) are hard
  to reach; tablet and phone navigation hides things without a clear way back.
- **6**: The sidebar works, current page is marked, but grouping or labels
  are weak, or narrow-screen navigation is awkward.
- **9**: Current location, deck and lifecycle are obvious; the inbox's
  waiting count is visible; navigation adapts cleanly from phone to desktop;
  back paths are obvious.

Watch for: the nav bar layout at each width.

## 6. Workflow efficiency [`workflow_efficiency`]

Common tasks take few, obvious steps. Use the step-by-step flows.

- **3**: Tasks need hidden menus, many dialogs or guesswork; obvious
  targets do nothing (a row that looks clickable but is not).
- **6**: Tasks complete in a reasonable number of steps, but at least one
  entry point is buried or one step is surprising.
- **9**: Each task's path is short and discoverable; the thing you want to
  open opens when you click it; review, import and sharing feel direct.

Watch for: the import workflow and its dialog (getting to it from a new
deck, and reading the preview); maybe-list rows that do not open the card's
details when the row itself is clicked; and the Advanced toggle (what it hides,
where it sits, whether it earns its place).

## 7. Feedback and states [`feedback_states`]

Empty, loading, success and error states are informative and calm.

- **3**: Empty pages are blank or confusing; actions give no confirmation;
  errors are raw.
- **6**: Empty states and confirmations exist but are wordy, inconsistent or
  easy to miss.
- **9**: Empty states point to the next action; every action confirms what
  happened with an undo where it matters; pending agent work is clearly
  pending.

## 8. Responsive and touch [`responsive_touch`]

The app holds up at 375, 768, 1280 and 1440 px, and touch targets are
usable on touch screens.

- **3**: Content overflows sideways, controls overlap, text is tiny on the
  phone, tap targets are cramped.
- **6**: Layouts adapt, but some screens waste space or squeeze controls at
  tablet width; some touch targets are small.
- **9**: Each width gets a layout designed for it; nothing scrolls sideways;
  touch controls are comfortably sized while desktop keeps its density.

## 9. Accessibility [`accessibility`]

Legible, labelled, navigable, perceivable without relying on colour alone.

- **3**: Tiny or faint text, meaning carried by colour alone, controls
  without visible labels, missing focus indication.
- **6**: Mostly legible and labelled, with some reliance on colour or
  icon-only controls.
- **9**: Text is comfortable, every control has a visible or obvious name,
  states are marked by more than colour, focus and selection are clear.

## Overall [`overall`]

Your holistic judgment of the product's UX quality for its users, not an
average of the categories. A 6 is a competent tool with clear friction; a 9
is one you would hold up as an example of a dense, professional tool.
