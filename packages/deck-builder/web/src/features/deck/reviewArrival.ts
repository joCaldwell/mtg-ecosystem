// Panel scrolling must stay in its own pane. scrollIntoView on a decision
// also scrolls the window, undoing the list's reveal on an inbox handoff.
// A decision out of view is brought in by its top edge, never its bottom:
// aligning bottoms left the proposal's summary, or the decision above,
// clipped under the panel's navigation.
export function revealReviewUnit(unit: HTMLElement): void {
  const pane = unit.closest<HTMLElement>('[data-panel-body]');
  if (!pane || !unit.getClientRects().length) return;
  const bounds = pane.getBoundingClientRect();
  const row = unit.getBoundingClientRect();
  const proposal = unit.closest<HTMLElement>('[data-proposal]');
  const box = proposal?.getBoundingClientRect();
  // Keep the proposal's summary with its first decision when both fit.
  const top = box && row.bottom - box.top <= pane.clientHeight ? box.top : row.top;
  // The proposal's own padding is its margin under the navigation, so
  // nothing of the content above shows at the pane's top edge.
  if (top < bounds.top || row.bottom > bounds.bottom) pane.scrollTop += top - bounds.top;
  // Near the end the pane cannot scroll that far. Then the proposal above
  // would be cut by the navigation: show it whole if the decision still fits.
  const above = proposal?.previousElementSibling?.getBoundingClientRect();
  if (!above || above.top >= bounds.top || above.bottom <= bounds.top) return;
  const shift = bounds.top - above.top;
  if (unit.getBoundingClientRect().bottom + shift <= bounds.bottom) pane.scrollTop -= shift;
}

export function focusReviewArrival(root: ParentNode, key: string): void {
  const unit = Array.from(root.querySelectorAll<HTMLElement>('[data-review-unit]'))
    .find(el => el.dataset.reviewUnit === key);
  if (!unit) return;
  revealReviewUnit(unit);
  unit.querySelector<HTMLInputElement>('input')?.focus({ preventScroll: true });
}
