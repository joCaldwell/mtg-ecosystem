import { useState } from "react";
import type { Zone } from "../../api/client.ts";
import { ZONE_LABELS } from "../../domain/table.ts";
import { ManaCost } from "../../mtg/index.ts";
import { Button, Select, TextField } from "../../ui/index.ts";
import type { PlaytestController } from "./usePlaytest.ts";
import styles from "./ZoneViewer.module.css";
export function ZoneViewer({
  controller: c,
  zone,
  onPreview,
  onSelect,
  onMove,
  onContextMenu,
}: {
  controller: PlaytestController;
  zone: Zone;
  onPreview: (id: string | null) => void;
  onSelect: (id: string, multiple: boolean) => void;
  onMove: (ids: string[], zone: Zone, row?: undefined, index?: number) => void;
  onContextMenu: (id: string, element: HTMLElement, x: number, y: number, ids: string[]) => void;
}) {
  const [query, setQuery] = useState(""),
    [top, setTop] = useState("5"),
    [kind, setKind] = useState<"top" | "search">("top"),
    [selected, setSelected] = useState<string[]>([]);
  const s = c.view!.session,
    f = s.frame,
    order = f.zones[zone],
    busy = c.pending > 0,
    playing = f.opening.stage === "playing";
  const topCount = Math.max(1, Math.min(1000, Number(top) || 1));
  const visible = (
    zone === "library" && kind === "top" ? order.slice(0, topCount) : order
  ).filter((id) =>
    s.cards[f.objects[id].oracle_id].name
      .toLocaleLowerCase()
      .includes(query.toLocaleLowerCase()),
  );
  const liveSelected = selected.filter((id) => order.includes(id));
  const chosen = order.filter((id) => liveSelected.includes(id));
  function move(target: Zone, index?: number) {
    if (chosen.length) onMove(chosen, target, undefined, index);
  }
  return (
    <section
      className={styles.drawer}
      aria-label={`${ZONE_LABELS[zone]} viewer`}
    >
      <header className={styles.header}>
        <h2>
          {ZONE_LABELS[zone]} · {order.length}
        </h2>
        {zone === "library" && (
          <>
            <Select
              aria-label="Library viewer mode"
              value={kind}
              onChange={(e) => {
                setKind(e.target.value as "top" | "search");
                setSelected([]);
              }}
            >
              <option value="top">Ordered top cards</option>
              <option value="search">Search whole library</option>
            </Select>
            {kind === "top" && (
              <TextField
                type="number"
                min={1}
                max={1000}
                aria-label="Top cards to reveal"
                className={styles.number}
                value={top}
                onChange={(e) => setTop(e.target.value)}
              />
            )}
          </>
        )}
        <TextField
          aria-label={`Search ${ZONE_LABELS[zone]}`}
          placeholder="Find by name"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <Button size="sm" onClick={() => c.editUi({ viewer: null })}>
          Close
        </Button>
      </header>
      <div className={styles.list}>
        {visible.map((id) => {
          const obj = f.objects[id],
            card = s.cards[obj.oracle_id];
          return (
            <div
              key={id}
              data-object-id={id}
              className={styles.item}
              role="group"
              tabIndex={0}
              aria-label={card.name}
              onContextMenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onContextMenu(id, e.currentTarget, e.clientX, e.clientY, chosen.includes(id) ? chosen : [id]);
              }}
              onKeyDown={(e) => {
                if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
                e.preventDefault();
                e.stopPropagation();
                const bounds = e.currentTarget.getBoundingClientRect();
                onContextMenu(id, e.currentTarget, bounds.left, bounds.top, chosen.includes(id) ? chosen : [id]);
              }}
              onMouseEnter={() => onPreview(id)}
              onFocus={() => onPreview(id)}
            >
              <input
                type="checkbox"
                aria-label={`Select ${card.name} for zone action`}
                checked={liveSelected.includes(id)}
                onChange={() =>
                  setSelected((prev) =>
                    prev.includes(id)
                      ? prev.filter((x) => x !== id)
                      : [...prev, id],
                  )
                }
              />
              <span className={styles.mono}>{order.indexOf(id) + 1}</span>
              <button
                type="button"
                className={styles.name}
                onClick={(e) =>
                  onSelect(id, e.shiftKey || e.ctrlKey || e.metaKey)
                }
              >
                {card.name}
              </button>
              <ManaCost cost={card.mana_cost ?? null} />
              {zone === "library" && (
                <>
                  <Button
                    size="sm"
                    disabled={busy || !playing || order.indexOf(id) === 0}
                    aria-label={`Move ${card.name} earlier in library`}
                    onClick={() =>
                      onMove([id], "library", undefined, order.indexOf(id) - 1)
                    }
                  >
                    ↑
                  </Button>
                  <Button
                    size="sm"
                    disabled={
                      busy || !playing || order.indexOf(id) === order.length - 1
                    }
                    aria-label={`Move ${card.name} later in library`}
                    onClick={() =>
                      onMove([id], "library", undefined, order.indexOf(id) + 1)
                    }
                  >
                    ↓
                  </Button>
                </>
              )}
            </div>
          );
        })}
        {!visible.length && <p>No cards match this view.</p>}
      </div>
      <footer className={styles.footer}>
        <span>
          {chosen.length} selected · library order is never sorted by search
        </span>
        <Button size="sm" onClick={() => setSelected(visible)}>
          Select visible
        </Button>
        <Button size="sm" onClick={() => setSelected([])}>
          Clear
        </Button>
        {(
          [
            "hand",
            "battlefield",
            "graveyard",
            "exile",
            "command",
            "companion",
            "stack",
          ] as Zone[]
        ).map((target) => (
          <Button
            size="sm"
            key={target}
            disabled={!chosen.length || busy || !playing}
            onClick={() => move(target)}
          >
            {ZONE_LABELS[target]}
          </Button>
        ))}
        <Button
          size="sm"
          disabled={!chosen.length || busy || !playing}
          onClick={() => move("library", 0)}
        >
          Library top
        </Button>
        <Button
          size="sm"
          disabled={!chosen.length || busy || !playing}
          onClick={() => move("library")}
        >
          Library bottom
        </Button>
      </footer>
    </section>
  );
}
