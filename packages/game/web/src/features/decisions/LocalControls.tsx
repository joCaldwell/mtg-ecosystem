import { useId, useLayoutEffect, useRef, useState } from 'react';
import { draftError, draftValue, reorderDraft, setDraftValue, toggleDraft, type LocalControl, type LocalDraft } from '../../decisions/controls.ts';
import styles from './decisions.module.css';

/** Controlled presentation only. Native controls supply keyboard conventions;
 * the parent owns local state and completion. No network/storage/SDK runtime. */
export function LocalControls({ control, draft, onChange, onComplete, confirmLabel = 'Confirm local draft', readyText = 'Draft shape complete. Core validation is still required.', validationKey, sharedHeading = false, quiet = false }: {
  readonly control: LocalControl; readonly draft: LocalDraft;
  readonly onChange: (draft: LocalDraft) => void; readonly onComplete: () => void;
  readonly confirmLabel?: string;
  readonly readyText?: string;
  readonly validationKey?: string; readonly sharedHeading?: boolean; readonly quiet?: boolean;
}) {
  const id = useId();
  const error = draftError(control, draft);
  const key = validationKey ?? JSON.stringify(control);
  const [validation, setValidation] = useState({ key, attempted: false });
  if (key !== validation.key) setValidation({ key, attempted: false });
  const showError = validation.key === key && validation.attempted && error !== null;
  const errorSummary = useRef<HTMLParagraphElement>(null);
  const [invalidSubmission, setInvalidSubmission] = useState(0);
  // Only an explicit failed submit routes focus; remote identity/option changes
  // reset validation without disturbing the player's active control.
  useLayoutEffect(() => {
    if (invalidSubmission > 0 && validation.key === key && validation.attempted) errorSummary.current?.focus();
  }, [invalidSubmission]);
  return <form className={styles.control} noValidate aria-labelledby={`${id}-title`} onSubmit={event => {
    event.preventDefault();
    if (error) {
      setValidation({ key, attempted: true });
      setInvalidSubmission(value => value + 1);
    } else onComplete();
  }} aria-describedby={`${id}-help`}>
    <fieldset><legend id={`${id}-title`} className={sharedHeading ? styles.srOnly : undefined}>{control.title}</legend><p id={`${id}-help`}>{control.description}</p>
      {control.kind === 'quantity' ? <div className={styles.quantity}><label htmlFor={`${id}-quantity`}>Exact quantity</label><input id={`${id}-quantity`} name="quantity" type="text" inputMode="numeric" enterKeyHint="done" autoComplete="off" autoCorrect="off" autoCapitalize="none" spellCheck={false} required pattern="0|[1-9][0-9]*" value={draft.quantity}
        aria-invalid={showError || undefined} aria-describedby={`${id}-help${showError ? ` ${id}-error` : ''}`} onBlur={() => {
          if (draft.quantity.length) setValidation({ key, attempted: true });
        }} onChange={event => onChange({ ...draft, quantity: event.currentTarget.value })} /></div>
        : control.kind === 'mapping' ? <div>{control.rows.map((row, index) => <div key={row.id} className={styles.quantity}>
          <label htmlFor={`${id}-row-${index}`}>{row.label}{row.detail && <small>{' '}Handle {row.detail}</small>}</label>
          <select id={`${id}-row-${index}`} name={`choice-${index}`} required value={draftValue(draft, row.id)}
            aria-invalid={showError || undefined} aria-describedby={`${id}-help${showError ? ` ${id}-error` : ''}`}
            onChange={event => onChange(setDraftValue(draft, row.id, event.currentTarget.value))}>
            <option value="" disabled>Choose an option</option>
            {row.options.map(option => <option key={option.id} value={option.id}>{option.label}{option.detail ? ` (${option.detail})` : ''}</option>)}
          </select>
        </div>)}</div>
        : control.kind === 'quantities' ? <div>{control.fields.map((field, index) => <div key={field.id} className={styles.quantity}>
          <label htmlFor={`${id}-amount-${index}`}>{field.label}{field.detail && <small>{' '}{field.detail}</small>}</label>
          <input id={`${id}-amount-${index}`} name={`amount-${index}`} type="text" inputMode="numeric" enterKeyHint="done" autoComplete="off" autoCorrect="off" autoCapitalize="none" spellCheck={false} required pattern="0|[1-9][0-9]*" maxLength={control.maxDigits}
            value={draftValue(draft, field.id)} aria-invalid={showError || undefined} aria-describedby={`${id}-help${showError ? ` ${id}-error` : ''}`}
            onBlur={() => { if (draftValue(draft, field.id).length) setValidation({ key, attempted: true }); }}
            onChange={event => onChange(setDraftValue(draft, field.id, event.currentTarget.value))} />
        </div>)}</div>
        : <div className={styles.options}>{control.options.map((option, index) => <label key={option.id} htmlFor={`${id}-option-${index}`} className={styles.option}>
          <input id={`${id}-option-${index}`} aria-describedby={`${id}-help${showError ? ` ${id}-error` : ''}`} type={control.kind === 'choice' ? 'radio' : 'checkbox'} name={id} value={option.id} checked={draft.selected.includes(option.id)} onChange={() => onChange(toggleDraft(control, draft, option.id))} />
          <span>{option.label}{option.detail && <small>Handle {option.detail}</small>}</span>
        </label>)}</div>}
      {control.kind === 'selection' && <p className={styles.count}>Selected {draft.selected.length} · Required {control.minimum === control.maximum ? control.minimum : `${control.minimum}–${control.maximum}`}</p>}
      {control.kind === 'selection' && control.ordered && <><h3>Chosen order · first to last</h3><ol className={styles.order}>{draft.selected.map((selected, index) => {
        const option = control.options.find(value => value.id === selected);
        return <li key={selected}><span>{option?.label ?? 'Unavailable object'}<small>Handle {selected}</small></span>
          <button type="button" aria-label={`Move ${selected} earlier`} aria-disabled={index === 0} onClick={() => { if (index > 0) onChange(reorderDraft(draft, selected, -1)); }}>Earlier</button>
          <button type="button" aria-label={`Move ${selected} later`} aria-disabled={index === draft.selected.length - 1} onClick={() => { if (index < draft.selected.length - 1) onChange(reorderDraft(draft, selected, 1)); }}>Later</button></li>;
      })}</ol></>}
    </fieldset>
    <p ref={errorSummary} id={`${id}-error`} tabIndex={-1} className={styles.hint}>{showError ? error : quiet ? null : error ? 'Complete your choice, then confirm to review it.' : readyText}</p>
    <button className={styles.complete} type="submit">{confirmLabel}</button>
  </form>;
}
