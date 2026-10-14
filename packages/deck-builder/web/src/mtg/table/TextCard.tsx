import { useState, type MouseEvent, type ReactNode } from "react";
import type { CardObservation } from "../../domain/table.ts";
import { faceOf } from "../../domain/table.ts";
import { ManaCost } from "../ManaCost.tsx";
import { OracleText } from "../OracleText.tsx";
import styles from "./TextCard.module.css";
export interface TextCardProps {
  observation: CardObservation;
  full?: boolean;
  selected?: boolean;
  targeted?: boolean;
  disabled?: boolean;
  disabledReason?: string;
  onSelect?: (multiple: boolean) => void;
  onPreview?: () => void;
  onPreviewEnd?: () => void;
  onContextMenu?: (element: HTMLElement, x: number, y: number) => void;
  draggable?: boolean;
  onDrop?: (id: string, side: "before" | "after") => void;
  footer?: ReactNode;
}
/** Display only the observation supplied by the host; hidden cards have no facts. */
export function TextCard({
  observation,
  full = false,
  selected = false,
  targeted = false,
  disabled = false,
  disabledReason,
  onSelect,
  onPreview,
  onPreviewEnd,
  onContextMenu,
  draggable = false,
  onDrop,
  footer,
}: TextCardProps) {
  const [dropSide, setDropSide] = useState<"before" | "after" | null>(null);
  if (observation.visibility === "hidden")
    return (
      <article
        className={`${styles.card} ${styles.hidden}`}
        aria-label={observation.label ?? "Hidden card"}
      >
        {observation.label ?? "Anonymous card"}
      </article>
    );
  const { card, object, owner, controller, attachedTo, attachments } =
      observation,
    face = faceOf(card, object?.face);
  const pick = (e: MouseEvent) => {
    if (
      disabled ||
      (e.target as HTMLElement).closest("button, input, select, textarea, a")
    )
      return;
    onSelect?.(e.shiftKey || e.ctrlKey || e.metaKey);
  };
  return (
    <article
      className={`${styles.card} ${full ? styles.full : styles.compact} ${targeted ? styles.targeted : ""}`}
      data-selected={selected}
      data-tapped={object?.tapped ?? false}
      data-object-id={object?.id}
      data-drop-side={dropSide ?? undefined}
      tabIndex={onSelect ? 0 : undefined}
      role="group"
      aria-label={`${selected ? "Selected, " : ""}${face.name}${object?.token ? ", token" : ""}`}
      aria-disabled={disabled || undefined}
      title={disabledReason}
      onContextMenu={(e) => {
        if (!onContextMenu || disabled) return;
        e.preventDefault();
        e.stopPropagation();
        onContextMenu(e.currentTarget, e.clientX, e.clientY);
      }}
      onClick={pick}
      onMouseEnter={onPreview}
      onMouseLeave={onPreviewEnd}
      onFocus={onPreview}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node))
          onPreviewEnd?.();
      }}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget || disabled) return;
        if (onContextMenu && (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10"))) {
          e.preventDefault();
          e.stopPropagation();
          const bounds = e.currentTarget.getBoundingClientRect();
          onContextMenu(e.currentTarget, bounds.left, bounds.top);
          return;
        }
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          onSelect?.(e.shiftKey || e.ctrlKey || e.metaKey);
        }
      }}
      onDragOver={(e) => {
        if (
          onDrop &&
          !disabled &&
          Array.from(e.dataTransfer.types).includes("application/x-mtg-object")
        ) {
          e.preventDefault();
          e.stopPropagation();
          e.dataTransfer.dropEffect = "move";
          const bounds = e.currentTarget.getBoundingClientRect();
          setDropSide(
            e.clientX < bounds.left + bounds.width / 2 ? "before" : "after",
          );
        }
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null))
          setDropSide(null);
      }}
      onDrop={(e) => {
        setDropSide(null);
        const id = e.dataTransfer.getData("application/x-mtg-object");
        if (id && onDrop && !disabled) {
          e.preventDefault();
          e.stopPropagation();
          const bounds = e.currentTarget.getBoundingClientRect();
          onDrop(
            id,
            e.clientX < bounds.left + bounds.width / 2 ? "before" : "after",
          );
        }
      }}
      draggable={draggable && !disabled}
      onDragEnd={() => setDropSide(null)}
      onDragStart={(e) => {
        if (object) {
          e.dataTransfer.setData("application/x-mtg-object", object.id);
          e.dataTransfer.effectAllowed = "move";
        }
      }}
    >
      <header className={styles.head}>
        <strong>{face.name}</strong>
        <ManaCost cost={face.mana_cost ?? null} />
      </header>
      <div className={styles.type}>{face.type_line}</div>
      {full && (
        <div className={styles.rules}>
          <OracleText text={face.oracle_text ?? ""} bare />
        </div>
      )}
      <div className={styles.facts}>
        {(face.power != null || face.toughness != null) && (
          <span>
            Printed {face.power ?? "—"}/{face.toughness ?? "—"}
          </span>
        )}
        {face.loyalty != null && <span>Printed loyalty {face.loyalty}</span>}
        {object && (object.power != null || object.toughness != null) && (
          <span>
            Manual {object.power ?? face.power ?? "—"}/
            {object.toughness ?? face.toughness ?? "—"}
          </span>
        )}
        {object?.token && <span>Token copy</span>}
        {object &&
          Object.entries(object.counters ?? {}).map(([name, n]) => (
            <span key={name}>
              {name}: {n}
            </span>
          ))}
        {object?.attached_to && (
          <span>Attached{attachedTo ? ` to ${attachedTo}` : ""}</span>
        )}
        {attachments?.map((name, i) => (
          <span key={i}>Attachment: {name}</span>
        ))}
        {owner && <span>Owner: {owner}</span>}
        {controller && <span>Controller: {controller}</span>}
      </div>
      {footer && <footer className={styles.footer}>{footer}</footer>}
      {disabledReason && <small>{disabledReason}</small>}
    </article>
  );
}
