import { useCallback, useEffect, useRef, useState } from "react";
import { useLive } from "../../api/live.tsx";
import { api } from "../../api/client.ts";
import type {
  Mode,
  PlaytestAction,
  PlaytestUiPatch,
  PlaytestView,
} from "../../api/client.ts";
type Ui = PlaytestView["session"]["ui"];
const EMPTY_UI: Ui = {
  selected: [],
  pinned: null,
  viewer: null,
  draft: { text: "", oracle_id: null, request_id: "" },
};
/** Serialize writes against the latest server version. Drafts are independent
 * of returned game frames so a delayed response never overwrites newer typing. */
export function usePlaytest(deckId: number) {
  const live = useLive({ resources: ["playtest"], deckId });
  const [view, setView] = useState<PlaytestView | null>(null),
    current = useRef<PlaytestView | null>(null);
  const [ui, setUi] = useState<Ui>(EMPTY_UI),
    uiRef = useRef(ui);
  const [loading, setLoading] = useState(true),
    [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<"saved" | "saving" | "failed">("saved");
  const [pending, setPending] = useState(0),
    queue = useRef<Promise<unknown>>(Promise.resolve());
  const mounted = useRef(true),
    draftTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reloadGeneration = useRef(0);
  const setLocal = useCallback((next: Ui) => {
    uiRef.current = next;
    if (mounted.current) setUi(next);
  }, []);
  const apply = useCallback(
    (v: PlaytestView) => {
      const sameSession = current.current?.session.id === v.session.id;
      current.current = v;
      if (mounted.current) setView(v);
      try {
        localStorage.setItem(
          `playtest.draft.${deckId}.${v.session.id}`,
          JSON.stringify({
            draft: sameSession ? uiRef.current.draft : v.session.ui.draft,
            version: v.session.version,
          }),
        );
      } catch {
        /* Optional draft recovery only. */
      }
    },
    [deckId],
  );
  const scratchKey = (sessionId: string) =>
    `playtest.draft.${deckId}.${sessionId}`;
  const stashDraft = (v: PlaytestView, draft: Ui["draft"]) => {
    try {
      localStorage.setItem(
        scratchKey(v.session.id),
        JSON.stringify({ draft, version: v.session.version }),
      );
    } catch {
      /* Server save status remains authoritative. */
    }
  };
  const enqueue = useCallback((job: () => Promise<void>) => {
    if (mounted.current) {
      setPending((n) => n + 1);
      setStatus("saving");
    }
    const result = queue.current.then(async () => {
      try {
        await job();
        if (mounted.current) {
          setStatus("saved");
          setError(null);
        }
        return true;
      } catch (e) {
        if (mounted.current) {
          setError(e instanceof Error ? e.message : String(e));
          setStatus("failed");
        }
        return false;
      } finally {
        if (mounted.current) setPending((n) => n - 1);
      }
    });
    queue.current = result;
    return result;
  }, []);
  const persistUi = useCallback(
    (patch: PlaytestUiPatch) =>
      enqueue(async () => {
        const s = current.current?.session;
        if (!s) return;
        const r = await api.playtestUi(deckId, s.id, s.version, patch);
        apply(r.playtest);
      }),
    [deckId, enqueue, apply],
  );
  const editUi = useCallback(
    (patch: PlaytestUiPatch) => {
      const next = { ...uiRef.current, ...patch };
      if (patch.draft && !uiRef.current.draft.text && current.current)
        next.draft = {
          ...next.draft,
          deck_revision: current.current.session.deck_revision,
        };
      if (patch.draft && uiRef.current.draft.text !== patch.draft.text)
        next.draft = { ...next.draft, request_id: crypto.randomUUID() };
      setLocal(next);
      if (current.current) stashDraft(current.current, next.draft);
      if (draftTimer.current) clearTimeout(draftTimer.current);
      draftTimer.current = setTimeout(() => {
        draftTimer.current = null;
        void persistUi(uiRef.current);
      }, 200);
    },
    [setLocal, persistUi],
  );
  const reload = useCallback(
    async (preserveDraft = true) => {
      const generation = ++reloadGeneration.current;
      await queue.current;
      if (!mounted.current || generation !== reloadGeneration.current) return;
      setLoading(true);
      try {
        const r = await api.getPlaytest(deckId);
        if (!mounted.current || generation !== reloadGeneration.current) return;
        current.current = r.playtest;
        setView(r.playtest);
        if (r.playtest) {
          const next = structuredClone(r.playtest.session.ui);
          if (preserveDraft && uiRef.current.draft.text)
            next.draft = uiRef.current.draft;
          else {
            try {
              const raw = localStorage.getItem(
                scratchKey(r.playtest.session.id),
              );
              if (raw) {
                const saved = JSON.parse(raw),
                  d = saved.draft;
                if (
                  saved.version >= r.playtest.session.version &&
                  d &&
                  typeof d.text === "string" &&
                  typeof d.request_id === "string" &&
                  /^[0-9a-f]{8}-[0-9a-f-]{27}$/i.test(d.request_id) &&
                  (d.oracle_id === null ||
                    r.playtest.session.cards[d.oracle_id])
                )
                  next.draft = d;
              }
            } catch {
              /* A malformed scratch copy must not replace the saved table. */
            }
          }
          setLocal(next);
        }
        setError(null);
        const unsaved =
          r.playtest &&
          JSON.stringify(uiRef.current.draft) !==
            JSON.stringify(r.playtest.session.ui.draft);
        if (unsaved) void persistUi({ draft: uiRef.current.draft });
        else setStatus("saved");
      } catch (e) {
        if (!mounted.current || generation !== reloadGeneration.current) return;
        setError(e instanceof Error ? e.message : String(e));
        setStatus("failed");
      } finally {
        if (mounted.current && generation === reloadGeneration.current)
          setLoading(false);
      }
    },
    [deckId, setLocal, persistUi],
  );
  useEffect(() => {
    mounted.current = true;
    void reload(false);
    return () => {
      mounted.current = false;
      if (draftTimer.current) {
        clearTimeout(draftTimer.current);
        draftTimer.current = null;
        void persistUi(uiRef.current);
      }
    };
  }, [reload, persistUi]);
  useEffect(() => { if (live) void reload(); }, [live, reload]);
  const start = (mode: Mode, replace = false) =>
    enqueue(async () => {
      if (draftTimer.current) {
        clearTimeout(draftTimer.current);
        draftTimer.current = null;
      }
      const old = current.current?.session;
      if (old && uiRef.current.draft.text)
        apply(
          (
            await api.playtestUi(deckId, old.id, old.version, {
              draft: uiRef.current.draft,
            })
          ).playtest,
        );
      const r = await api.startPlaytest(
        deckId,
        mode,
        current.current ?? undefined,
        replace,
      );
      apply(r.playtest);
      setLocal(r.playtest.session.ui);
      if (old && old.id !== r.playtest.session.id) {
        try {
          localStorage.removeItem(scratchKey(old.id));
        } catch {
          /* Optional scratch cleanup. */
        }
      }
    });
  /** Resolves to the table the action produced, or null if it failed. */
  const act = async (action: PlaytestAction) => {
    let applied: PlaytestView | null = null;
    const ok = await enqueue(async () => {
      const s = current.current?.session;
      if (!s) return;
      applied = (await api.playtestAction(deckId, s.id, s.version, action)).playtest;
      apply(applied);
      const next = {
        ...uiRef.current,
        selected: uiRef.current.selected.filter(
          (id) => !!current.current?.session.frame.objects[id],
        ),
        pinned:
          uiRef.current.pinned &&
          current.current?.session.frame.objects[uiRef.current.pinned]
            ? uiRef.current.pinned
            : null,
      };
      setLocal(next);
    });
    return ok ? (applied as PlaytestView | null) : null;
  };
  const saveObservation = async () => {
    const s = current.current?.session;
    if (!s) return false;
    const draft = { ...uiRef.current.draft };
    const success = await enqueue(async () => {
      const latest = current.current!.session;
      apply(
        (await api.playtestUi(deckId, latest.id, latest.version, { draft }))
          .playtest,
      );
      await api.playtestObservation(deckId, { ...draft, session_id: s.id });
      if (
        uiRef.current.draft.request_id === draft.request_id &&
        uiRef.current.draft.text === draft.text
      ) {
        editUi({
          draft: { ...draft, text: "", request_id: crypto.randomUUID() },
        });
      }
    });
    return success;
  };
  return {
    view,
    ui,
    loading,
    error,
    status,
    pending,
    act,
    editUi,
    start,
    reload,
    saveObservation,
    retrySave: () => persistUi(uiRef.current),
  };
}
export type PlaytestController = ReturnType<typeof usePlaytest>;
