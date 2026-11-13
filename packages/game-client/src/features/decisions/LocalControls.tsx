import { useId } from 'react';
import { draftError, reorderDraft, toggleDraft, type LocalControl, type LocalDraft } from '../../decisions/controls.ts';
import styles from './decisions.module.css';

/** Controlled presentation only. Native controls supply keyboard conventions;
 * the parent owns local state and completion. No network/storage/SDK runtime. */
export function LocalControls({ control, draft, onChange, onComplete, confirmLabel = 'Confirm local draft', readyText = 'Draft shape complete. Core validation is still required.' }: {
  readonly control: LocalControl; readonly draft: LocalDraft;
  readonly onChange: (draft: LocalDraft) => void; readonly onComplete: () => void;
  readonly confirmLabel?: string;
  readonly readyText?: string;
}) {
  const id = useId();
  const error = draftError(control, draft);
  return <form className={styles.control} onSubmit={event => { event.preventDefault(); if (!error) onComplete(); }} aria-describedby={`${id}-help ${id}-error`}>
    <fieldset><legend>{control.title}</legend><p id={`${id}-help`}>{control.description}</p>
      {control.kind === 'quantity' ? <label className={styles.quantity}>Exact quantity<input type="text" inputMode="numeric" autoComplete="off" spellCheck={false} value={draft.quantity}
        aria-invalid={draft.quantity.length > 0 && error !== null} aria-describedby={`${id}-error`} onChange={event => onChange({ ...draft, quantity: event.currentTarget.value })} /></label>
        : <div className={styles.options}>{control.options.map(option => <label key={option.id} className={styles.option}>
          <input type={control.kind === 'choice' ? 'radio' : 'checkbox'} name={id} value={option.id} checked={draft.selected.includes(option.id)} onChange={() => onChange(toggleDraft(control, draft, option.id))} />
          <span>{option.label}{option.detail && <small>Handle {option.detail}</small>}</span>
        </label>)}</div>}
      {control.kind === 'selection' && <p className={styles.count}>Selected {draft.selected.length} · Required {control.minimum === control.maximum ? control.minimum : `${control.minimum}–${control.maximum}`}</p>}
      {control.kind === 'selection' && control.ordered && <><h3>Chosen order · first to last</h3><ol className={styles.order}>{draft.selected.map((selected, index) => {
        const option = control.options.find(value => value.id === selected);
        return <li key={selected}><span>{option?.label ?? 'Unavailable object'}<small>Handle {selected}</small></span>
          <button type="button" aria-label={`Move ${selected} earlier`} aria-disabled={index === 0} onClick={() => onChange(reorderDraft(draft, selected, -1))}>Earlier</button>
          <button type="button" aria-label={`Move ${selected} later`} aria-disabled={index === draft.selected.length - 1} onClick={() => onChange(reorderDraft(draft, selected, 1))}>Later</button></li>;
      })}</ol></>}
    </fieldset>
    <p id={`${id}-error`} className={styles.hint} aria-live="polite">{error ?? readyText}</p>
    <button className={styles.complete} type="submit" disabled={error !== null}>{confirmLabel}</button>
  </form>;
}
