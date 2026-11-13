import type { CommandSnapshot } from '../../session/command.ts';

/** Status is separate from rules outcomes. Accepted and missing status never
 * imply consumption. No universal undo or local ambiguity-clearing control. */
export function CommandStatus({ command, connected, onRetry }: {
  readonly command: CommandSnapshot; readonly connected: boolean; readonly onRetry: () => void;
}) {
  let message: string;
  switch (command.phase) {
    case 'disabled': return null;
    case 'scanning': message = 'Checking for a saved choice before enabling actions.'; break;
    case 'ready': message = command.disposition === 'rejected' ? 'The table rejected the previous choice. The view has refreshed. Review and explicitly confirm a new choice.'
      : command.disposition === 'consumed' ? 'The table used your choice. Review the next decision.' : 'Opening choices are ready. Nothing is sent until you confirm.'; break;
    case 'persisting': message = 'Saving your choice safely. Please wait.'; break;
    case 'sending': message = 'Sending your saved choice. Further choices are disabled.'; break;
    case 'unresolved': message = command.failure === 'transport' ? 'The result is uncertain. Your saved choice is retained; reconnect or retry to check it.'
      : 'Your choice is pending. Waiting for the table to confirm whether it was used.'; break;
    case 'awaiting-refresh': message = command.disposition === 'rejected' ? 'The table rejected this choice. Refreshing the view before a new confirmation.' : 'Choice confirmed. Refreshing the table before enabling another choice.'; break;
    case 'blocked': message = command.failure === 'binding' ? 'A saved choice belongs to a different table build. Sending is blocked; keep the original seat file and ask the operator to restore that table.'
      : 'Safe choice storage is unavailable or damaged. Sending is blocked. Restore browser storage, then release and reimport your seat file. Saved uncertain choices must be preserved.'; break;
  }
  return <section aria-label="Choice status"><p role="status" aria-live="polite">{message}</p>
    {command.phase === 'unresolved' && <button disabled={!connected} onClick={onRetry}>Check or retry saved choice</button>}
    {!connected && command.commandId && <p>Reconnect by reimporting the same seat file. Releasing or refreshing this page does not undo the saved choice.</p>}
  </section>;
}
