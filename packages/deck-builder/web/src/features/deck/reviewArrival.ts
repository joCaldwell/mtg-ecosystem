// Panel scrolling must stay in its own pane. scrollIntoView on a decision
// also scrolls the window, undoing the list's reveal on an inbox handoff.
export function revealReviewUnit(unit: HTMLElement): void {
  const pane = unit.closest<HTMLElement>('[data-panel-body]');
  if (!pane || !unit.getClientRects().length) return;
  const bounds = pane.getBoundingClientRect();
  const row = unit.getBoundingClientRect();
  const proposal = unit.closest<HTMLElement>('[data-proposal]')?.getBoundingClientRect();
  // Keep the proposal's summary with its first decision when both fit.
  const top = proposal && row.bottom - proposal.top <= pane.clientHeight ? proposal.top : row.top;
  if (top < bounds.top || row.bottom > bounds.bottom) {
    pane.scrollTop += row.bottom - top <= pane.clientHeight ? top - bounds.top : row.bottom - bounds.bottom;
  }
}

export function focusReviewArrival(root: ParentNode, key: string): void {
  const unit = Array.from(root.querySelectorAll<HTMLElement>('[data-review-unit]'))
    .find(el => el.dataset.reviewUnit === key);
  if (!unit) return;
  revealReviewUnit(unit);
  unit.querySelector<HTMLInputElement>('input')?.focus({ preventScroll: true });
}
