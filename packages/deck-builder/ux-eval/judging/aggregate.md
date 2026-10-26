# Aggregator prompt: one backlog from many judgments

You are merging UX review findings into a single, ranked backlog. You will
receive several JSON outputs from absolute reviews of the same product: more
than one reviewer, and more than one sample from each. Each output has
`findings` (screen key, category, severity, issue, fix, scope) and
`watch_items`.

You are not reviewing the product yourself. Work only from the findings.

## What to do

1. **Group** findings that describe the same underlying problem, even when
   they name different screens or word it differently. ("Filter row blends
   into the list" on the 1280 deck and "no separation above the card list"
   on the 1440 deck are one problem.) Do not merge problems that merely share
   a category.
2. For each group, record:
   - `count`: how many findings are in it.
   - `reviewers`: how many distinct reviewers raised it (a reviewer's
     several samples count once here).
   - `severity`: the most common severity in the group; on a tie, the higher.
   - `score`: `count × weight`, where weight is high = 3, medium = 2, low = 1.
3. Write one **issue** sentence and one **fix** for the group: the most
   concrete fix proposed, combined with others where they agree. Keep the
   screen keys where it is visible.
4. Decide the group's **scope**: `small_fix` if a developer could make the
   change in under an hour without a product decision, otherwise
   `owner_decision` (new theme, removed or moved feature, navigation model,
   changed workflow). When reviewers disagree, choose `owner_decision`.
5. Rank groups by `score`, then by `reviewers`. Return the top N (N is given
   with the inputs; 15 if not).
6. Summarise the `watch_items`: for each concern, how many outputs said it
   holds, partly holds or does not hold.

Do not add problems no reviewer raised, and do not drop a problem because
you disagree with it.

## Output

A single JSON object and nothing else:

```json
{
  "backlog": [
    {
      "rank": 1,
      "issue": "",
      "fix": "",
      "screen_keys": [""],
      "category": "",
      "severity": "high | medium | low",
      "count": 0,
      "reviewers": 0,
      "score": 0,
      "scope": "small_fix | owner_decision"
    }
  ],
  "small_fixes": [1],
  "owner_decisions": [2],
  "watch_items": [
    { "concern": "", "holds": 0, "partly": 0, "does_not_hold": 0 }
  ]
}
```

`small_fixes` and `owner_decisions` list backlog ranks, so the two kinds can
be handed on separately.
