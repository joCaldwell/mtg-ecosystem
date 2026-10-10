import { useEffect, useState } from "react";
import { api, type DocumentEdit, type Profile } from "./api.ts";
import { useLive } from "./live.tsx";
import { DocumentEditCard, type Rule } from "./Rulings.tsx";
import { ago } from "./lib.ts";

/** The player profile (spec §6): the owner's document, always shown to agents. */
export function ProfilePage() {
  const live = useLive();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const [edits, setEdits] = useState<DocumentEdit[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    void api.getProfile().then((r) => setProfile(r.profile));
    void api.getInbox().then((inbox) => setEdits(inbox.document_edits.filter((e) => e.kind === "profile")));
  };
  useEffect(load, [live]);

  async function save() {
    if (draft == null) return;
    try {
      setProfile((await api.setProfile(draft)).profile);
      setDraft(null);
      setError(null);
    } catch (e: any) {
      setError(e.message);
    }
  }

  const rule: Rule = async (fn) => {
    try {
      const result = await fn();
      if (result.profile) setProfile(result.profile);
      load();
      return result;
    } catch (e: any) {
      setError(e.message);
      return null;
    }
  };

  if (!profile) return <div className="page muted">Loading…</div>;
  const words = (draft ?? profile.body_md).trim() ? (draft ?? profile.body_md).trim().split(/\s+/).length : 0;

  return (
    <div className="page">
      <header className="deck-list-heading">
        <h1>Profile</h1>
        <span className="muted">Who you are as a builder</span>
      </header>
      <p className="muted">
        Your tastes, playgroup and power level, budget stance, standing dislikes. Every agent session
        starts by reading this. It is yours: an agent can only propose an edit, which you rule on here.
      </p>
      {error && <div className="error-banner">{error}</div>}
      {edits.map((e) => (
        <DocumentEditCard key={e.id} edit={e} rule={rule} />
      ))}
      <textarea
        className="profile-body"
        value={draft ?? profile.body_md}
        onChange={(e) => setDraft(e.target.value)}
        rows={18}
        placeholder={"What do you like to build? What does your playgroup allow?\nWhat will you never play?"}
        aria-label="Profile"
      />
      <div className="row gap">
        <span className={words > 600 ? "error-text" : "muted"}>
          {words} words{words > 600 && " — over the 600-word guide; an agent reads all of it every session"}
        </span>
        <span className="muted">· saved {ago(profile.updated_at)}</span>
        <span className="spacer" />
        {draft != null && (
          <button onClick={() => setDraft(null)}>Discard changes</button>
        )}
        <button className="primary" disabled={draft == null || draft === profile.body_md} onClick={save}>
          Save
        </button>
      </div>
    </div>
  );
}
