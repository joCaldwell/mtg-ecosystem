import { useMemo, useState, type CSSProperties } from "react";
import type {
  DeckState,
  PlaytestAction,
  TableObject,
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
  faceOf,
} from "../../domain/table.ts";
import { PlayerSummary, TableZone, TextCard, TapButton } from "../../mtg/index.ts";
import {
  Banner,
  Button,
  ContextMenu,
  Dialog,
  Icon,
  ResizeHandle,
  Segmented,
  TextField,
  tokenPx,
  useHotkeys,
  useLocalStorage,
  type ContextMenuTarget,
  type ContextMenuProps,
} from "../../ui/index.ts";
import { SessionControls } from "./SessionControls.tsx";
import { InspectionPanel } from "./InspectionPanel.tsx";
import { ZoneViewer } from "./ZoneViewer.tsx";
import { TokenDialog } from "./TokenDialog.tsx";
import { NewGameDialog } from "./NewGameDialog.tsx";
import { RandomNumberDialog, type Roll } from "./RandomNumberDialog.tsx";
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
    [token, setToken] = useState(false),
    [drawing, setDrawing] = useState(false),
    [rolling, setRolling] = useState(false),
    [rolls, setRolls] = useState<Roll[]>([]);
  const [context, setContext] = useState<(ContextMenuTarget & { id: string; ids: string[] }) | null>(null);
  const [drawCount, setDrawCount] = useState("3");
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
  function openContext(id: string, element: HTMLElement, x: number, y: number,
    ids = selected.includes(id) ? selected : [id]) {
    setHover(null);
    if (pinned !== id || ids.length !== selected.length || ids.some((x) => !selected.includes(x)))
      c.editUi({ selected: ids, pinned: id });
    setContext({ id, ids, element, x, y });
  }
  const contextIds = context?.ids.filter((id) => !!f.objects[id]) ?? [];
  const contextObject = context && f.objects[context.id];
  const contextZone = contextObject ? objectZone(v, contextObject.id)! : null;
  const contextItems: ContextMenuProps["items"] = [];
  if (contextObject && contextZone && contextIds.length) {
    const single = contextIds.length === 1;
    if (playing) {
      if (contextIds.every((id) => f.zones.battlefield.includes(id))) {
        const tapped = contextIds.every((id) => f.objects[id].tapped);
        contextItems.push({ id: "tap", label: tapped ? "Untap" : "Tap", disabled: busy,
          onSelect: () => act({ type: "tap", ids: contextIds, tapped: !tapped }) });
      }
      for (const zone of ["hand", "battlefield", "graveyard", "exile", "command", "companion", "stack"] as Zone[]) {
        contextItems.push({ id: zone, label: `Move to ${ZONE_LABELS[zone].toLowerCase()}`, disabled: busy,
          separator: zone === "hand", onSelect: () => move(contextIds, zone) });
      }
      contextItems.push(
        { id: "library-top", label: "Move to library top", disabled: busy, onSelect: () => move(contextIds, "library", undefined, 0) },
        { id: "library-bottom", label: "Move to library bottom", disabled: busy, onSelect: () => move(contextIds, "library") },
      );
      if (contextIds.every((id) => f.zones.battlefield.includes(id))) {
        for (const row of Object.keys(ROW_LABELS) as TableRow[])
          contextItems.push({ id: row, label: `Row: ${ROW_LABELS[row]}`, separator: row === "creatures", disabled: busy,
            onSelect: () => move(contextIds, "battlefield", row) });
      }
    }
    if (playing) {
      if (single) contextItems.push({ id: "copy", label: "Create token copy", separator: true, disabled: busy,
        onSelect: () => act({ type: "token", copy_id: contextObject.id }) });
      if (contextIds.every((id) => f.objects[id].token))
        contextItems.push({ id: "remove-token", label: "Remove token", tone: "cut", disabled: busy,
          onSelect: () => act({ type: "remove-token", ids: contextIds }) });
    }
    contextItems.push({ id: "clear", label: "Clear selection", separator: true, onSelect: clearSelection });
  }
  function move(ids: string[], zone: Zone, row?: TableRow, index?: number) {
    act({ type: "move", ids, zone, row, index });
  }
  function draggedIds(id: string, zone: Zone) {
    // Rearranging a hand preserves its selected cards' table order and never
    // pulls a selection from another zone into the hand.
    if (zone === "hand" && f.zones.hand.includes(id))
      return selected.includes(id)
        ? f.zones.hand.filter((x) => selected.includes(x))
        : [id];
    return selected.includes(id) ? selected : [id];
  }
  function drop(id: string, zone: Zone, row?: TableRow) {
    if (!playing && (zone !== "hand" || !f.zones.hand.includes(id))) return;
    if (f.objects[id]) move(draggedIds(id, zone), zone, row);
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
        onContextMenu={(element, x, y) => openContext(id, element, x, y)}
        draggable={!busy && (playing || f.zones.hand.includes(id))}
        onDrop={
          busy || (!playing && !f.zones.hand.includes(id))
            ? undefined
            : (dragged, side) => {
                if (!f.objects[dragged]) return;
                const zone = objectZone(v, id)!;
                if (!playing && (zone !== "hand" || !f.zones.hand.includes(dragged)))
                  return;
                const ids = draggedIds(dragged, zone);
                if (ids.includes(id)) return;
                const index = f.zones[zone].filter((x) => !ids.includes(x)).indexOf(id);
                move(
                  ids,
                  zone,
                  zone === "battlefield" ? obj.row : undefined,
                  index + (side === "after" ? 1 : 0),
                );
              }
        }
        footer={
          <>
            <span>
              {obj.tapped && objectZone(v, id) === "battlefield" ? "Tapped" : ""}
            </span>
            {!playing &&
            f.zones.hand.includes(id) &&
            (bottomIndex >= 0 ||
              f.opening.bottom.length < v.computed.bottom_required) ? (
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
                <TapButton
                  name={faceOf(card, obj.face).name}
                  tapped={obj.tapped}
                  disabled={busy}
                  onToggle={() =>
                    act({ type: "tap", ids: [id], tapped: !obj.tapped })
                  }
                />
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
    r: () => setRolling(true),
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
  }, { enabled: !context });
  // The commander and companion sit on the table, not behind a button; their
  // zones join the strip only when the deck has nothing that lives there.
  const hasRole = (role: TableObject["role"]) =>
    Object.values(f.objects).some((o) => o.role === role);
  const railZones = (["command", "companion"] as Zone[]).filter(
    (zone) => v.computed.counts[zone] > 0 || hasRole(zone === "command" ? "commander" : "companion"),
  );
  const zoneButtons = (["library", "graveyard", "exile", "stack", "command", "companion"] as Zone[]).filter(
    (zone) =>
      !railZones.includes(zone) &&
      (zone !== "companion" || v.computed.counts.companion > 0),
  );
  const modeOptions = [
    { value: "commander" as const, label: "Commander mulligans" },
    { value: "practice" as const, label: "Practice redraws" },
  ];
  return (
    <div
      className={styles.page}
      onClick={(e) => {
        const target = e.target;
        if (!(target instanceof Element) || !e.currentTarget.contains(target)) return;
        // Controls and portal menus still need to act on the current selection.
        // Use click, not pointer-down, so starting a drag never clears its cards.
        if (target.closest('[data-object-id], button, a, input, textarea, select, label, summary, [role="separator"], [contenteditable="true"]'))
          return;
        if (selected.length || pinned) clearSelection();
        else setHover(null);
      }}
    >
      <header className={styles.header}>
        <div className={styles.identity}>
          <a
            className={styles.deckLink}
            href={href({ page: "deck", deckId: s.deck_id })}
            aria-label="Back to deck overview"
            title="Deck overview"
          >
            <Icon name="back" />
          </a>
          <div className={styles.title}>
            <h1>{s.deck_name}</h1>
            <span>
              Playtest · <span className={styles.mono}>r{s.deck_revision}</span>
            </span>
          </div>
        </div>
        <PlayerSummary
          name="You"
          showName={false}
          life={f.life}
          turn={playing ? f.turn : undefined}
          onLifeChange={
            playing ? (value) => act({ type: "life", value }) : undefined
          }
        />
        <SessionControls
          view={v}
          busy={busy}
          status={c.status}
          onAction={act}
          onDrawSeveral={() => setDrawing(true)}
          onCreateToken={() => setToken(true)}
          onRandomNumber={() => setRolling(true)}
          onRestart={() => setRestart(true)}
          onObservation={clearSelection}
        />
      </header>
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
        <div className={styles.opening} role="region" aria-label="Opening hand">
          <div className={styles.openingStatus}>
            <strong>
              {f.opening.mode === "practice"
                ? "Practice hand"
                : f.opening.mulligans === 0
                  ? "Opening hand"
                  : `Mulligan ${f.opening.mulligans}`}
            </strong>
            <span>
              {f.opening.mode === "practice"
                ? "Unlimited redraws"
                : f.opening.mulligans === 0
                  ? "First mulligan is free"
                  : v.computed.bottom_required
                    ? `Bottom ${f.opening.bottom.length} of ${v.computed.bottom_required}`
                    : "Free mulligan"}
            </span>
          </div>
          {f.opening.bottom.length > 0 && (
            <ol className={styles.bottom} aria-label="Cards to bottom, nearest the top first">
              {f.opening.bottom.map((id) => (
                <li key={id}>
                  <Button
                    size="sm"
                    aria-label={`Keep ${s.cards[f.objects[id].oracle_id].name} in hand`}
                    onClick={() =>
                      act({
                        type: "bottom",
                        ids: f.opening.bottom.filter((x) => x !== id),
                      })
                    }
                  >
                    {s.cards[f.objects[id].oracle_id].name}
                    <Icon name="close" size={12} />
                  </Button>
                </li>
              ))}
            </ol>
          )}
          <div className={styles.openingActions}>
            <Segmented
              label="Mulligan mode"
              options={modeOptions}
              value={f.opening.mode}
              onChange={(mode) => act({ type: "mode", mode })}
            />
            <Button
              disabled={busy || !v.computed.can_mulligan}
              onClick={() => act({ type: "mulligan" })}
            >
              {f.opening.mode === "practice" ? "Redraw seven" : "Mulligan"}
            </Button>
            <Button
              variant="primary"
              disabled={busy || !v.computed.can_keep}
              title={
                v.computed.can_keep
                  ? undefined
                  : `Choose ${v.computed.bottom_required - f.opening.bottom.length} more to bottom`
              }
              onClick={() => act({ type: "keep" })}
            >
              Keep hand
            </Button>
          </div>
        </div>
      )}
      <div
        className={styles.workspace}
        onDragStartCapture={() => setHover(null)}
        style={
          width == null
            ? undefined
            : ({ "--inspection-w": `${width}px` } as CSSProperties)
        }
      >
        <div className={`${styles.table} ${!playing ? styles.choosing : ""} ${playing && f.zones.battlefield.length === 0 && !c.ui.viewer ? styles.emptyBattlefield : ""}`}>
          <nav className={styles.zones} aria-label="Table zones">
            {zoneButtons.map((zone) => (
              <button
                key={zone}
                type="button"
                className={styles.zone}
                aria-pressed={c.ui.viewer === zone}
                onClick={() => {
                  setHover(null);
                  c.editUi({ viewer: c.ui.viewer === zone ? null : zone });
                }}
                onDragOver={(e) => {
                  if (playing) e.preventDefault();
                }}
                onDrop={(e) => {
                  const id = e.dataTransfer.getData("application/x-mtg-object");
                  if (id) drop(id, zone);
                }}
              >
                {ZONE_LABELS[zone]}
                <span className={styles.count}>{v.computed.counts[zone]}</span>
              </button>
            ))}
          </nav>
          <div className={styles.surface}>
            {railZones.length > 0 && (
              <div className={styles.rail}>
                {railZones.map((zone) => (
                  <TableZone
                    key={zone}
                    label={ZONE_LABELS[zone]}
                    count={v.computed.counts[zone]}
                    emptyText=""
                    onDrop={busy || !playing ? undefined : (id) => drop(id, zone)}
                  >
                    {f.zones[zone].map((id) => card(id))}
                  </TableZone>
                ))}
              </div>
            )}
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
            {c.ui.viewer && (
              <ZoneViewer
                key={c.ui.viewer}
                controller={c}
                zone={c.ui.viewer}
                onPreview={setHover}
                onSelect={select}
                onMove={move}
                onContextMenu={openContext}
              />
            )}
          </div>
          <TableZone
            label="Hand"
            count={v.computed.counts.hand}
            variant="hand"
            onDrop={busy ? undefined : (id) => drop(id, "hand")}
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
          <div className={styles.inspection}>
            <InspectionPanel
              controller={c}
              deck={deck}
              objectId={pinned ?? preview}
              pinned={!!pinned}
              onDeckReload={onDeckReload}
            />
          </div>
        </aside>
        {pinned && preview && preview !== pinned && (
          <div
            className={styles.preview}
            aria-label="Temporary rules preview"
            tabIndex={0}
            onMouseLeave={() => setHover(null)}
          >
            <TextCard observation={observeObject(v, preview)} full />
          </div>
        )}
      </div>
      <ContextMenu
        target={contextObject ? context : null}
        label={contextObject
          ? contextIds.length > 1 ? `${contextIds.length} selected cards`
            : faceOf(s.cards[contextObject.oracle_id], contextObject.face).name
          : "Card actions"}
        items={contextItems}
        onClose={() => setContext(null)}
      />
      {drawing && (
        <Dialog title="Draw cards" onClose={() => setDrawing(false)} footer={
          <Button variant="primary"
            disabled={busy || !playing || !Number.isInteger(Number(drawCount)) || Number(drawCount) < 1 || Number(drawCount) > 1000}
            onClick={() => { act({ type: "draw", count: Number(drawCount) }); setDrawing(false); }}
          >Draw cards</Button>
        }>
          <label className={styles.drawField}>
            Number of cards
            <TextField data-autofocus type="number" min={1} max={1000}
              aria-label="Number of cards to draw" value={drawCount}
              onChange={(e) => setDrawCount(e.target.value)} />
          </label>
        </Dialog>
      )}
      {restart && (
        <NewGameDialog
          deckName={s.deck_name}
          revision={deck.deck.revision}
          commanders={deck.cards.filter((x) => x.role === "commander" && !x.maybe)}
          initialMode={f.opening.mode}
          replaces="table"
          keepsDraft={!!c.ui.draft.text}
          busy={busy}
          onStart={(mode) => {
            setRestart(false);
            void c.start(mode, true);
          }}
          onClose={() => setRestart(false)}
        />
      )}
      {rolling && (
        <RandomNumberDialog
          rolls={rolls}
          onRoll={(roll) => setRolls((previous) => [roll, ...previous].slice(0, 6))}
          onClose={() => setRolling(false)}
        />
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
