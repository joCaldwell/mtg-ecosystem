import { useMemo, useState, type CSSProperties } from "react";
import type {
  DeckState,
  Mode,
  PlaytestAction,
  Zone,
  TableRow,
} from "../../api/client.ts";
import { useReportFocus } from "../../app/useReportFocus.ts";
import { href } from "../../app/routes.ts";
import {
  ROW_LABELS,
  ZONE_LABELS,
  objectZone,
  selectedAfterClick,
  observeObject,
} from "../../domain/table.ts";
import { PlayerSummary, TableZone, TextCard } from "../../mtg/index.ts";
import {
  Banner,
  Button,
  ConfirmDialog,
  ResizeHandle,
  Segmented,
  TextField,
  tokenPx,
  useHotkeys,
  useLocalStorage,
} from "../../ui/index.ts";
import { InspectionPanel } from "./InspectionPanel.tsx";
import { ZoneViewer } from "./ZoneViewer.tsx";
import { TokenDialog } from "./TokenDialog.tsx";
import type { PlaytestController } from "./usePlaytest.ts";
import styles from "./PlaytestTable.module.css";
export function PlaytestTable({
  controller: c,
  deck,
  onDeckReload,
}: {
  controller: PlaytestController;
  deck: DeckState;
  onDeckReload: () => void;
}) {
  const v = c.view!,
    s = v.session,
    f = s.frame,
    playing = f.opening.stage === "playing";
  const [hover, setHover] = useState<string | null>(null),
    [restart, setRestart] = useState(false),
    [token, setToken] = useState(false);
  const [mode, setMode] = useState<Mode>(f.opening.mode),
    [drawCount, setDrawCount] = useState("3");
  const [width, setWidth] = useLocalStorage<number | null>(
    "playtest.panelWidth",
    (raw) => (raw && Number.isFinite(Number(raw)) ? Number(raw) : null),
    (n) => (n == null ? "" : String(n)),
  );
  const limits = useMemo(
    () => ({
      min: tokenPx("--panel-w-min"),
      max: tokenPx("--panel-w-max"),
      initial: tokenPx("--inspection-w"),
    }),
    [],
  );
  const act = (action: PlaytestAction) => {
    if (!c.pending) void c.act(action);
  };
  const selected = c.ui.selected.filter((id) => !!f.objects[id]);
  const pinned = c.ui.pinned && f.objects[c.ui.pinned] ? c.ui.pinned : null;
  const preview = hover && f.objects[hover] ? hover : null;
  const busy = c.pending > 0;
  const clearSelection = () => {
    setHover(null);
    c.editUi({ selected: [], pinned: null });
  };
  useReportFocus(s.deck_id, s.deck_name, [
    ...new Set(selected.map((id) => f.objects[id].oracle_id)),
  ]);
  function select(id: string, multiple: boolean) {
    c.editUi({
      selected: selectedAfterClick(c.ui.selected, id, multiple),
      pinned: id,
    });
  }
  function move(ids: string[], zone: Zone, row?: TableRow, index?: number) {
    act({ type: "move", ids, zone, row, index });
  }
  function drop(id: string, zone: Zone, row?: TableRow) {
    if (f.objects[id]) move(selected.includes(id) ? selected : [id], zone, row);
  }
  function card(id: string, full = false) {
    const obj = f.objects[id],
      card = s.cards[obj.oracle_id];
    const bottomIndex = f.opening.bottom.indexOf(id);
    return (
      <TextCard
        key={id}
        observation={observeObject(v, id)}
        full={full}
        selected={selected.includes(id)}
        onSelect={(multiple) => select(id, multiple)}
        onPreview={() => setHover(id)}
        draggable={playing}
        onDrop={(dragged) => {
          if (!playing || !f.objects[dragged]) return;
          const zone = objectZone(v, id)!;
          const ids = selected.includes(dragged) ? selected : [dragged];
          if (ids.includes(id)) return;
          move(
            ids,
            zone,
            zone === "battlefield" ? obj.row : undefined,
            f.zones[zone].filter((x) => !ids.includes(x)).indexOf(id),
          );
        }}
        footer={
          <>
            <span>
              {selected.includes(id)
                ? "Selected"
                : obj.token
                  ? "Token"
                  : objectZone(v, id) === "battlefield"
                    ? obj.tapped
                      ? "Tapped"
                      : "Untapped"
                    : ""}
            </span>
            {!playing &&
            v.computed.bottom_required > 0 &&
            f.zones.hand.includes(id) ? (
              <Button
                size="sm"
                disabled={
                  busy ||
                  (bottomIndex < 0 &&
                    f.opening.bottom.length >= v.computed.bottom_required)
                }
                onClick={() =>
                  act({
                    type: "bottom",
                    ids:
                      bottomIndex >= 0
                        ? f.opening.bottom.filter((x) => x !== id)
                        : [...f.opening.bottom, id],
                  })
                }
              >
                {bottomIndex >= 0
                  ? `Bottom ${bottomIndex + 1} · remove`
                  : "Choose to bottom"}
              </Button>
            ) : (
              objectZone(v, id) === "battlefield" && (
                <Button
                  size="sm"
                  disabled={busy}
                  aria-pressed={obj.tapped}
                  onClick={() =>
                    act({ type: "tap", ids: [id], tapped: !obj.tapped })
                  }
                >
                  {obj.tapped ? "Untap" : "Tap"}
                </Button>
              )
            )}
          </>
        }
      />
    );
  }
  useHotkeys({
    d: () => playing && act({ type: "draw", count: 1 }),
    "shift+d": () =>
      playing &&
      act({ type: "draw", count: Math.max(1, Number(drawCount) || 1) }),
    s: () => playing && act({ type: "shuffle" }),
    u: () => playing && act({ type: "untap-all" }),
    n: () => playing && act({ type: "next-turn" }),
    t: () =>
      playing &&
      selected.length > 0 &&
      selected.every((id) => f.zones.battlefield.includes(id)) &&
      act({
        type: "tap",
        ids: selected,
        tapped: !selected.every((id) => f.objects[id].tapped),
      }),
    z: () => v.computed.can_undo && act({ type: "undo" }),
    "shift+z": () => v.computed.can_redo && act({ type: "redo" }),
    "mod+z": (e) => {
      e.preventDefault();
      if (v.computed.can_undo) act({ type: "undo" });
    },
    "mod+shift+z": (e) => {
      e.preventDefault();
      if (v.computed.can_redo) act({ type: "redo" });
    },
    escape: () => (c.ui.viewer ? c.editUi({ viewer: null }) : clearSelection()),
  });
  const modeOptions = [
    { value: "commander" as const, label: "Commander mulligans" },
    { value: "practice" as const, label: "Practice redraws" },
  ];
  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <div>
          <a href={href({ page: "deck", deckId: s.deck_id })}>Deck overview</a>
          <h1>
            {s.deck_name} <span>· Playtest</span>
          </h1>
          <p>Solo table · manual play · tested r{s.deck_revision}</p>
        </div>
        <PlayerSummary
          name="You"
          life={f.life}
          turn={f.turn}
          status={playing ? "Early-turn testing" : "Choosing an opening hand"}
          onLifeChange={
            playing ? (value) => act({ type: "life", value }) : undefined
          }
        />
        <Button
          onClick={() => {
            setMode(f.opening.mode);
            setRestart(true);
          }}
          disabled={busy}
        >
          New opening test
        </Button>
      </header>
      <div className={styles.toolbar}>
        <Button
          variant="primary"
          kbd="D"
          disabled={!playing || busy || !v.computed.counts.library}
          onClick={() => act({ type: "draw", count: 1 })}
        >
          Draw
        </Button>
        <TextField
          type="number"
          aria-label="Number of cards to draw"
          value={drawCount}
          min={1}
          max={1000}
          onChange={(e) => setDrawCount(e.target.value)}
          className={styles.numberInput}
        />
        <Button
          kbd="⇧D"
          disabled={
            !playing ||
            busy ||
            !Number.isInteger(Number(drawCount)) ||
            Number(drawCount) < 1 ||
            Number(drawCount) > 1000
          }
          onClick={() => act({ type: "draw", count: Number(drawCount) })}
        >
          Draw several
        </Button>
        <Button
          kbd="S"
          disabled={!playing || busy}
          onClick={() => act({ type: "shuffle" })}
        >
          Shuffle
        </Button>
        <Button
          kbd="U"
          disabled={!playing || busy}
          onClick={() => act({ type: "untap-all" })}
        >
          Untap all
        </Button>
        <Button
          kbd="N"
          disabled={!playing || busy}
          onClick={() => act({ type: "next-turn" })}
        >
          Next turn
        </Button>
        <span className={styles.divider} />
        <Button
          kbd="Z"
          disabled={busy || !v.computed.can_undo}
          onClick={() => act({ type: "undo" })}
        >
          Undo
        </Button>
        <Button
          kbd="⇧Z"
          disabled={busy || !v.computed.can_redo}
          onClick={() => act({ type: "redo" })}
        >
          Redo
        </Button>
        <Button disabled={!playing || busy} onClick={() => setToken(true)}>
          Create token
        </Button>
        <Button onClick={() => clearSelection()}>Deck observation</Button>
        <span className={styles.save} role="status">
          {busy || c.status === "saving"
            ? "Saving…"
            : c.status === "failed"
              ? "Save failed"
              : "Saved"}
        </span>
      </div>
      {deck.deck.revision !== s.deck_revision && (
        <Banner tone="warn">
          Testing r{s.deck_revision}; the saved deck is now r
          {deck.deck.revision}. Continue this snapshot or start a new opening
          test.
        </Banner>
      )}
      {c.error && (
        <Banner tone="error">
          {c.error}
          <Button size="sm" onClick={() => void c.retrySave()} disabled={busy}>
            Retry session save
          </Button>
          <Button size="sm" onClick={() => void c.reload()} disabled={busy}>
            Reload saved table
          </Button>
        </Banner>
      )}
      {!playing && (
        <div className={styles.opening}>
          <div className={styles.openingTitle}>
            <strong>Opening hand</strong>
            <Segmented
              label="Mulligan mode"
              options={modeOptions}
              value={f.opening.mode}
              onChange={(mode) => act({ type: "mode", mode })}
            />
            <span>
              {f.opening.mode === "practice"
                ? "Unlimited practice redraws"
                : `Mulligan ${f.opening.mulligans} · ${v.computed.bottom_required} to bottom`}
            </span>
            <Button
              disabled={busy || !v.computed.can_mulligan}
              onClick={() => act({ type: "mulligan" })}
            >
              {f.opening.mode === "practice" ? "Redraw seven" : "Mulligan"}
            </Button>
            <Button
              variant="primary"
              disabled={busy || !v.computed.can_keep}
              onClick={() => act({ type: "keep" })}
            >
              Keep hand
            </Button>
          </div>
          <p>
            {v.computed.bottom_required
              ? `Choose ${v.computed.bottom_required} cards in bottom order (first chosen is nearer the library's top). Keep is available once all are chosen.`
              : "Read your seven cards, then keep or redraw. The first Commander mulligan is free."}
          </p>
          {f.opening.bottom.length > 0 && (
            <div className={styles.bottom}>
              {f.opening.bottom.map((id, i) => (
                <Button
                  key={id}
                  size="sm"
                  onClick={() =>
                    act({
                      type: "bottom",
                      ids: f.opening.bottom.filter((x) => x !== id),
                    })
                  }
                >
                  {i + 1}. {s.cards[f.objects[id].oracle_id].name} ×
                </Button>
              ))}
            </div>
          )}
        </div>
      )}
      <div
        className={styles.workspace}
        style={
          width == null
            ? undefined
            : ({ "--inspection-w": `${width}px` } as CSSProperties)
        }
      >
        <div className={`${styles.table} ${!playing ? styles.choosing : ""}`}>
          <nav className={styles.zones} aria-label="Table zones">
            {(
              [
                "library",
                "command",
                "companion",
                "graveyard",
                "exile",
                "stack",
              ] as Zone[]
            ).map((zone) => (
              <Button
                key={zone}
                aria-pressed={c.ui.viewer === zone}
                onClick={() =>
                  c.editUi({ viewer: c.ui.viewer === zone ? null : zone })
                }
                onDragOver={(e) => {
                  if (playing) e.preventDefault();
                }}
                onDrop={(e) => {
                  const id = e.dataTransfer.getData("application/x-mtg-object");
                  if (id) drop(id, zone);
                }}
              >
                {ZONE_LABELS[zone]}{" "}
                <span className={styles.mono}>{v.computed.counts[zone]}</span>
              </Button>
            ))}
          </nav>
          <div className={styles.battlefield} aria-label="Battlefield">
            {playing &&
              (Object.keys(ROW_LABELS) as TableRow[]).map((row) => (
                <TableZone
                  key={row}
                  label={ROW_LABELS[row]}
                  showHeader={false}
                  count={
                    f.zones.battlefield.filter(
                      (id) => f.objects[id].row === row,
                    ).length
                  }
                  emptyText=""
                  onDrop={(id) => drop(id, "battlefield", row)}
                >
                  {f.zones.battlefield
                    .filter((id) => f.objects[id].row === row)
                    .map((id) => card(id))}
                </TableZone>
              ))}
          </div>
          <TableZone
            label="Hand"
            count={v.computed.counts.hand}
            variant="hand"
            onDrop={playing ? (id) => drop(id, "hand") : undefined}
            emptyText="Draw cards to add to your hand"
          >
            {f.zones.hand.map((id) => card(id, true))}
          </TableZone>
        </div>
        <aside
          className={styles.panel}
          aria-label="Inspection panel"
          onMouseEnter={() => {
            if (pinned) setHover(null);
          }}
          onFocusCapture={() => {
            if (pinned) setHover(null);
          }}
        >
          <ResizeHandle
            label="Resize inspection panel"
            edge="left"
            value={width ?? limits.initial}
            min={limits.min}
            max={limits.max}
            onChange={setWidth}
            onReset={() => setWidth(null)}
          />
          <InspectionPanel
            controller={c}
            deck={deck}
            objectId={pinned ?? preview}
            pinned={!!pinned}
            onDeckReload={onDeckReload}
            onCreateToken={() => setToken(true)}
            onClearSelection={clearSelection}
          />
        </aside>
        {pinned && preview && preview !== pinned && (
          <div
            className={styles.preview}
            aria-label="Temporary rules preview"
            tabIndex={0}
            onMouseLeave={() => setHover(null)}
          >
            <p>Temporary preview · selection stays pinned</p>
            <TextCard observation={observeObject(v, preview)} full />
          </div>
        )}
        {c.ui.viewer && (
          <ZoneViewer
            key={c.ui.viewer}
            controller={c}
            zone={c.ui.viewer}
            onPreview={setHover}
            onSelect={select}
            onMove={move}
          />
        )}
      </div>
      {selected.length > 0 && playing && (
        <div className={styles.selection}>
          <strong>{selected.length} selected</strong>
          <Button size="sm" onClick={() => clearSelection()}>
            Clear
          </Button>
          <Button
            size="sm"
            disabled={
              busy || selected.some((id) => !f.zones.battlefield.includes(id))
            }
            onClick={() => act({ type: "tap", ids: selected, tapped: true })}
          >
            Tap selected
          </Button>
          <Button
            size="sm"
            disabled={
              busy || selected.some((id) => !f.zones.battlefield.includes(id))
            }
            onClick={() => act({ type: "tap", ids: selected, tapped: false })}
          >
            Untap selected
          </Button>
          {(
            [
              "hand",
              "battlefield",
              "graveyard",
              "exile",
              "command",
              "stack",
            ] as Zone[]
          ).map((zone) => (
            <Button
              key={zone}
              size="sm"
              disabled={busy}
              onClick={() => move(selected, zone)}
            >
              {ZONE_LABELS[zone]}
            </Button>
          ))}
          <Button
            size="sm"
            disabled={busy}
            onClick={() => move(selected, "library", undefined, 0)}
          >
            Library top
          </Button>
          <Button
            size="sm"
            disabled={busy}
            onClick={() => move(selected, "library")}
          >
            Library bottom
          </Button>
        </div>
      )}
      <footer className={styles.footer}>
        Next turn untaps and advances. Draw separately.{" "}
        <span>
          Shift-click to select several · T to tap/untap selected · Esc to clear
        </span>
      </footer>
      {restart && (
        <ConfirmDialog
          title="Start a new opening test?"
          confirmLabel="Replace current session"
          tone="danger"
          onClose={() => setRestart(false)}
          onConfirm={() => {
            setRestart(false);
            void c.start(mode, true);
          }}
        >
          <p>
            This replaces the current table and game undo history. Saved
            memories and tags remain.
            {c.ui.draft.text
              ? " Your unfinished observation is retained with its tested revision."
              : ""}
          </p>
          <Segmented
            label="New session mode"
            value={mode}
            onChange={setMode}
            options={modeOptions}
          />
        </ConfirmDialog>
      )}
      {token && (
        <TokenDialog
          onClose={() => setToken(false)}
          onCreate={(oracle_id) => {
            setToken(false);
            act({ type: "token", oracle_id });
          }}
        />
      )}
    </div>
  );
}
