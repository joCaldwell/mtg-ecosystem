import { useRef, useState } from 'react';
import { Link } from 'react-router';
import type { CardView } from '../../presentation/table-model.ts';
import { CardFace } from '../../cards/CardFace.tsx';
import { Inspector } from '../inspector/Inspector.tsx';
import { emptyDraft, type DecisionBinding, type DecisionCompletion, type DecisionRequest, type LocalControl, type ObjectLabel } from '../../decisions/controls.ts';
import { DecisionPanel } from './DecisionPanel.tsx';
import { LocalControls } from './LocalControls.tsx';
import styles from './decisions.module.css';

const labLabels: readonly ObjectLabel[] = [{ handle: 'lab-a', name: 'Same label' }, { handle: 'lab-b', name: 'Same label' }, { handle: 'lab-c', name: null }];
const quantity: LocalControl = { kind: 'quantity', title: 'Exact quantity bookkeeping', description: 'Synthetic input only. No request context or gameplay meaning is claimed. Input-size bound: 80 digits.', maxDigits: 80 };

/** An isolated fixture surface. App supplies only the already qualified public
 * capture; synthetic exercises are deliberately separate and contain no cards. */
export function DecisionGallery({ request, labels, binding, source, cards }: {
  readonly request: DecisionRequest; readonly labels: readonly ObjectLabel[];
  readonly binding: DecisionBinding; readonly source: string;
  readonly cards: readonly CardView[];
}) {
  const [inspected, setInspected] = useState<string | null>(null);
  const inspectionTrigger = useRef<HTMLButtonElement | null>(null);
  const capturedHand = useRef<HTMLDetailsElement>(null);
  const [receipt, setReceipt] = useState('No local draft confirmed.');
  const [generation, setGeneration] = useState(0);
  const [removed, setRemoved] = useState(false);
  const [exactDraft, setExactDraft] = useState(emptyDraft);
  const labBinding = { ...binding, match: 'structural-lab', incarnation: generation, token: `lab-${generation}` };
  const candidates = removed ? ['lab-b', 'lab-c'] : ['lab-a', 'lab-b', 'lab-c'];
  const complete = (value: DecisionCompletion) => setReceipt(JSON.stringify(value, null, 2));
  return <div className={styles.gallery}>
    <span className="eyebrow">Multiverse / isolated fixture gallery</span>
    <h1>Decision drafts</h1>
    <p className={styles.intro}>Review and confirm local drafts without sending game actions. Live opening choices are available separately on the live table.</p>
    <p><Link to="/fixtures/decisions#table">Try opening choices on the table</Link></p>
    <nav aria-label="Gallery sections"><a href="#captured">Captured opening</a><a href="#structural">Structural exercises</a><a href="#blocked">Contract gaps</a></nav>
    <div className={styles.galleryLayout}><div>
    <section id="captured" className={styles.section} aria-labelledby="captured-title"><h2 id="captured-title">Captured foundation choice</h2>
      <details className={styles.source}><summary>Capture provenance</summary><p>{source}</p><p>Historical permitted capture, checked against the current qualified SDK. This fixture does not advance a match.</p></details>
      <details ref={capturedHand} className={styles.hand}><summary>Captured hand · {labels.length} objects</summary><ul>{cards.map(card => <li key={card.inspectionId}><CardFace card={card} selected={inspected === card.inspectionId} onInspect={(id, button) => { inspectionTrigger.current = button; setInspected(id); }} /><small>Handle {card.inspectionId}</small></li>)}</ul><a href="/fixtures/opening">Open the captured table</a></details>
      <DecisionPanel binding={binding} request={request} labels={labels} onComplete={complete} />
    </section>
    <section id="structural" className={styles.section} aria-labelledby="structural-title"><h2 id="structural-title">Synthetic structural exercises</h2>
      <p className={styles.intro}>These generated-type-shaped samples test local bookkeeping only. They contain no card facts and establish no admitted game reachability. Two candidates intentionally share a label.</p>
      <p><button onClick={() => setGeneration(value => value + 1)}>Replace decision identity</button>{' '}<button onClick={() => setRemoved(value => !value)}>Update candidate set</button></p>
      <div className={styles.labs}>
        <DecisionPanel binding={labBinding} labels={labLabels} request={{ request: 'discard', player: binding.seat, token: labBinding.token, count: 2, candidates }} onComplete={complete} />
        <DecisionPanel binding={labBinding} labels={labLabels} request={{ request: 'bottom_opening', player: binding.seat, token: labBinding.token, count: 2, candidates }} onComplete={complete} />
        <div className={styles.panel}><LocalControls control={quantity} draft={exactDraft} onChange={setExactDraft} onComplete={() => setReceipt(`Synthetic exact quantity: ${exactDraft.quantity}`)} /><p className={styles.boundary}>No generated gameplay response is constructed from this quantity exercise.</p></div>
      </div>
    </section>
    <section id="blocked" className={styles.section} aria-labelledby="blocked-title"><h2 id="blocked-title">Explicit contract gaps</h2>
      <p className={styles.source}>Synthetic modes request: index membership alone provides no source-backed human option labels.</p>
      <DecisionPanel binding={labBinding} request={{ request: 'modes', token: labBinding.token, player: binding.seat, candidates: [0, 1] }} labels={[]} onComplete={complete} />
      <p className={styles.source}>Ordering/allocation primitives require supplied meaning and constraints before a gameplay adapter. Costs, target eligibility and lethal damage remain core-owned. Action labels are currently unavailable.</p>
    </section>
    </div><div className={styles.gallerySide}><Inspector card={cards.find(card => card.inspectionId === inspected) ?? null} onClose={() => {
      setInspected(null);
      // A player may collapse the hand while inspection remains open. Reveal
      // the exact originating button before returning keyboard focus to it.
      if (capturedHand.current) capturedHand.current.open = true;
      inspectionTrigger.current?.focus();
    }} /><section className={`${styles.section} ${styles.receiptPanel}`} aria-labelledby="receipt-title"><h2 id="receipt-title">Last local completion</h2><p className={styles.source}>Local output only · no command sent</p><pre aria-live="polite" className={styles.receipt}>{receipt}</pre></section></div></div>
  </div>;
}
