import { useRef, useState } from 'react';
import { Link } from 'react-router';
import type { CardView } from '@multiverse/game-ui';
import { CardFace, GameSurface } from '@multiverse/game-ui';
import { CardInspector as Inspector } from '@multiverse/game-ui';
import { emptyDraft, type DecisionBinding, type DecisionCompletion, type DecisionRequest, type LocalControl, type ObjectLabel } from '../../decisions/controls.ts';
import { DecisionPanel } from './DecisionPanel.tsx';
import { LocalControls } from './LocalControls.tsx';
import styles from './decisions.module.css';

const labLabels: readonly ObjectLabel[] = [{ handle: 'lab-a', name: 'Same label' }, { handle: 'lab-b', name: 'Same label' }, { handle: 'lab-c', name: null }];
const quantity: LocalControl = { shape:{shape:'number',min:'0',max:null}, kind: 'quantity', title: 'Exact quantity bookkeeping', description: 'Synthetic input only. No request context or gameplay meaning is claimed. Input-size bound: 80 digits.', maxDigits: 80 };

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
  const structuredBinding = { ...binding, match: 'structured-lab', incarnation: 0, token: 'structured-lab' };
  const candidates = removed ? ['lab-b', 'lab-c'] : ['lab-a', 'lab-b', 'lab-c'];
  const objects = candidates.map(object => ({ candidate: 'object' as const, object }));
  const indices = objects.map((_, i) => i);
  const lab = { player: binding.seat, token: labBinding.token, candidates: objects };
  const combat = { player: binding.seat, token: structuredBinding.token, candidates: [...objects, {candidate:'player' as const, player:1}] };
  const complete = (value: DecisionCompletion) => setReceipt(JSON.stringify(value, null, 2));
  return <GameSurface className={styles.gallery}>
    <h1>Decision drafts</h1>
    <p className={styles.intro}>Review and confirm local drafts without sending game actions. Live opening choices are available separately on the live table.</p>
    <p><Link to="/fixtures/decisions#table">Try opening choices on the table</Link></p>
    <nav aria-label="Gallery sections"><a href="#captured">Captured opening</a><a href="#structural">Structural exercises</a></nav>
    <div className={styles.galleryLayout}><div>
    <section id="captured" className={styles.section} aria-labelledby="captured-title"><h2 id="captured-title">Captured foundation choice</h2>
      <details className={styles.source}><summary>Capture provenance</summary><p>{source}</p><p>Permitted Worker capture, checked against the qualified SDK. This fixture does not advance a match.</p></details>
      <details ref={capturedHand} className={styles.hand}><summary>Captured hand · {labels.length} objects</summary><ul role="list">{cards.map(card => <li key={card.inspectionId}><CardFace card={card} selected={inspected === card.inspectionId} onInspect={(id, button) => { inspectionTrigger.current = button; setInspected(id); }} /><small>Handle {card.inspectionId}</small></li>)}</ul><a href="/fixtures/opening">Open the captured table</a></details>
      <DecisionPanel binding={binding} request={request} labels={labels} onComplete={complete} />
    </section>
    <section id="structural" className={styles.section} aria-labelledby="structural-title"><h2 id="structural-title">Synthetic structural exercises</h2>
      <p className={styles.intro}>These generated-type-shaped samples test local bookkeeping only. They contain no card facts and establish no admitted game reachability. Two candidates intentionally share a label.</p>
      <p><button type="button" onClick={() => setGeneration(value => value + 1)}>Replace decision identity</button>{' '}<button type="button" onClick={() => setRemoved(value => !value)}>Update candidate set</button></p>
      <div className={styles.labs}>
        <DecisionPanel binding={labBinding} labels={labLabels} request={{ ...lab, reason:{reason:'discard'}, shape:{shape:'pick',slots:[{role:{role:'choice'},subject:null,min:2,max:2,repeat:false,allowed:indices}],rules:[]} }} onComplete={complete} />
        <DecisionPanel binding={labBinding} labels={labLabels} request={{ ...lab, reason:{reason:'bottom_opening'},shape:{shape:'order',items:indices,min:2,max:2} }} onComplete={complete} />
        <div className={styles.panel}><LocalControls control={quantity} draft={exactDraft} onChange={setExactDraft} onComplete={() => setReceipt(`Synthetic exact quantity: ${exactDraft.quantity}`)} /><p className={styles.boundary}>No generated gameplay response is constructed from this quantity exercise.</p></div>
      </div>
    </section>
    <section className={styles.section} aria-labelledby="structured-title"><h2 id="structured-title">Supplied combat and payment drafts</h2>
      <p className={styles.intro}>Synthetic candidates, quantities and source explanation only. These exercises establish no real-game reachability or legality.</p>
      <div className={styles.labs}>
        <DecisionPanel binding={structuredBinding} labels={labLabels} request={{ ...combat,reason:{reason:'attackers'},shape:{shape:'pick',slots:indices.map(subject=>({role:{role:'attack_with'},min:0,max:1,repeat:false,subject,allowed:[objects.length]})),rules:[]} }} onComplete={complete} />
        <DecisionPanel binding={structuredBinding} labels={labLabels} request={{ ...combat,reason:{reason:'combat_damage', source:'lab-a',power:'5'},shape:{shape:'distribute',total:'5',buckets:[{candidate:1,min:'0',max:'5'},{candidate:objects.length,min:'0',max:'5'}],gates:[{bucket:1,requires:[[0,'2']]}]} }} onComplete={complete} />
        <DecisionPanel binding={structuredBinding} labels={labLabels} context={{ source: { kind: 'spell', oracle_id: 'synthetic-source', text: 'Synthetic complete source block for payment.' } }} request={{ ...combat,reason:{reason:'spend_mana',source:null},shape:{shape:'pay',cost:{generic:'9007199254740993',colored:['U']},pool:{unrestricted:{U:'1',C:'9007199254740993'}},step:{step:'spend'}} }} onComplete={complete} />
      </div>
    </section>
    </div><div className={styles.gallerySide}><Inspector diagnostics card={cards.find(card => card.inspectionId === inspected) ?? null} onClose={() => {
      setInspected(null);
      // A player may collapse the hand while inspection remains open. Reveal
      // the exact originating button before returning keyboard focus to it.
      if (capturedHand.current) capturedHand.current.open = true;
      inspectionTrigger.current?.focus();
    }} /><section className={`${styles.section} ${styles.receiptPanel}`} aria-labelledby="receipt-title"><h2 id="receipt-title">Last local completion</h2><p className={styles.source}>Local output only · no command sent</p><pre aria-live="polite" className={styles.receipt}>{receipt}</pre></section></div></div>
  </GameSurface>;
}
