# Phase 4b QA evidence

Code head: `5d12baf07688b690aa45d7149a26a4696a6c6a6d`. Gate: 1,220 tests. Public startup JS: **89,744 gzip bytes**, including React and the entry.

Only synthetic QA accounts and sample decks are pictured. The source database was an online backup in ignored .dev/; no database or credential is published.

The four requested design checklists were read and applied by hand because this runtime has no Claude Code Skill tool. The full report is in docs/deck-builder-v3-4b-qa.md on the code branch.

31 axe scans passed: 23 page/overlay states, four empty-account views, and four signed-out public-page views. The hosted-mode local harness retained its production CSP; axe injection used Playwright’s test-only CSP bypass. Public startup was also checked without that bypass.

Real wheel input verified the scrolling contract at every pictured width; touch and keyboard inspection, nested Escape, Advanced persistence, hidden-control focus trapping and import preview were exercised. No QA check set scrollY or called scrollTo.

## Screenshots

- [decks-1024](decks-1024.png)
- [decks-1280](decks-1280.png)
- [decks-375](decks-375.png)
- [decks-768](decks-768.png)
- [drawer-768](drawer-768.png)
- [first-explainer-1280](first-explainer-1280.png)
- [first-run-1024](first-run-1024.png)
- [first-run-1280](first-run-1280.png)
- [first-run-375](first-run-375.png)
- [first-run-768](first-run-768.png)
- [import-preview-768](import-preview-768.png)
- [inbox-1024](inbox-1024.png)
- [inbox-1280](inbox-1280.png)
- [inbox-375](inbox-375.png)
- [inbox-768](inbox-768.png)
- [keyboard-card-1280](keyboard-card-1280.png)
- [new-account-deck-1024](new-account-deck-1024.png)
- [new-account-deck-1280](new-account-deck-1280.png)
- [new-account-deck-375](new-account-deck-375.png)
- [new-account-deck-768](new-account-deck-768.png)
- [public-1024](public-1024.png)
- [public-1280](public-1280.png)
- [public-375](public-375.png)
- [public-768](public-768.png)
- [sheet-1024](sheet-1024.png)
- [sheet-768](sheet-768.png)
- [sheet-explainer-768](sheet-explainer-768.png)
- [touch-card-768](touch-card-768.png)

## Machine records

- [Page, role, contrast and keyboard checks](final-qa.json)
- [Empty-account scans](empty-qa.json)
- [Signed-out public-page scans and export](public-anonymous-qa.json)
- [Production startup and touch](production-qa.json)
- [First-run walkthrough](walkthrough.json)
- [Import walkthrough](import-qa.json)
- [Bundle accounting](bundle-final.json)
- [Official MCP client transcript](transcript.md)
