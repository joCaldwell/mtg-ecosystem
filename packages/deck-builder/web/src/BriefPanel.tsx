import { useEffect, useState, type FormEvent } from "react";
import { api, type Brief } from "./api.ts";

export function BriefPanel({ deckId }: { deckId: number }) {
  const [brief, setBrief] = useState<Brief | null>(null);
  const [thesis, setThesis] = useState("");
  const [constraints, setConstraints] = useState("");
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [engineName, setEngineName] = useState("");
  const [engineDesc, setEngineDesc] = useState("");
  const [enginePieces, setEnginePieces] = useState("");

  useEffect(() => {
    api.getBrief(deckId).then((b) => {
      setBrief(b);
      setThesis(b.thesis);
      setConstraints(b.constraints_md);
    });
  }, [deckId]);

  if (!brief) return null;

  async function save() {
    setError(null);
    try {
      const b = await api.updateBrief(deckId, { thesis, constraints_md: constraints });
      setBrief(b);
      setDirty(false);
    } catch (e: any) {
      setError(e.message);
    }
  }

  async function addEngine(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      const names = enginePieces.split(",").map((n) => n.trim()).filter(Boolean);
      const pieces: Array<{ oracle_id: string }> = [];
      for (const name of names) {
        const matches = await api.resolveCard(name);
        if (!matches.length) throw new Error(`'${name}' is not an exact card name`);
        pieces.push({ oracle_id: matches[0].oracle_id });
      }
      const b = await api.setEngine(deckId, engineName, engineDesc, pieces);
      setBrief(b);
      setEngineName("");
      setEngineDesc("");
      setEnginePieces("");
    } catch (e: any) {
      setError(e.message);
    }
  }

  return (
    <details className="group">
      <summary>
        Brief{brief.thesis ? "" : " (empty — write the thesis)"}
        {brief.engines.length > 0 && ` · ${brief.engines.length} engine(s)`}
      </summary>
      {error && <div className="error-banner">{error}</div>}

      <h2>Thesis</h2>
      <textarea
        value={thesis}
        onChange={(e) => {
          setThesis(e.target.value);
          setDirty(true);
        }}
        rows={3}
        placeholder="What is this deck trying to do?"
      />
      <h2>Constraints</h2>
      <textarea
        value={constraints}
        onChange={(e) => {
          setConstraints(e.target.value);
          setDirty(true);
        }}
        rows={3}
        placeholder="Budget, pod meta, no infinite combos…"
      />
      {dirty && (
        <div className="row gap" style={{ marginTop: 6 }}>
          <button onClick={save}>Save brief</button>
        </div>
      )}

      <h2>Named engines</h2>
      {brief.engines.map((e) => (
        <div className="card-row" key={e.id}>
          <div className="card-main">
            <span className="name">{e.name}</span>
            <span className="muted">{e.description}</span>
            <span className="spacer" />
            <button
              className="small danger"
              onClick={() => api.removeEngine(deckId, e.id).then(setBrief).catch((err) => setError(err.message))}
            >
              ✕
            </button>
          </div>
          <div className="card-tags">
            {e.pieces.map((p) => (
              <span key={p.oracle_id} className={`tag-chip ${p.in_deck ? "active" : ""}`} title={p.in_deck ? "in deck" : "NOT in deck"}>
                {p.name}
                {!p.in_deck && " ⚠"}
              </span>
            ))}
          </div>
        </div>
      ))}
      <form onSubmit={addEngine} className="stack gap" style={{ marginTop: 6 }}>
        <div className="row gap">
          <input value={engineName} onChange={(e) => setEngineName(e.target.value)} placeholder="Engine name" />
          <input className="grow" value={engineDesc} onChange={(e) => setEngineDesc(e.target.value)} placeholder="Description" />
        </div>
        <div className="row gap">
          <input
            className="grow"
            value={enginePieces}
            onChange={(e) => setEnginePieces(e.target.value)}
            placeholder="Pieces: exact card names, comma-separated"
          />
          <button type="submit" disabled={!engineName.trim()}>
            Set engine
          </button>
        </div>
      </form>
    </details>
  );
}
