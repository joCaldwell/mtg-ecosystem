import type { MouseEvent, ReactNode } from "react";
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
  draggable?: boolean;
  onDrop?: (id: string) => void;
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
  draggable = false,
  onDrop,
  footer,
}: TextCardProps) {
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
      tabIndex={onSelect ? 0 : undefined}
      role="group"
      aria-label={`${selected ? "Selected, " : ""}${face.name}${object?.token ? ", token" : ""}`}
      aria-disabled={disabled || undefined}
      title={disabledReason}
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
        )
          e.preventDefault();
      }}
      onDrop={(e) => {
        const id = e.dataTransfer.getData("application/x-mtg-object");
        if (id && onDrop && !disabled) {
          e.preventDefault();
          e.stopPropagation();
          onDrop(id);
        }
      }}
      draggable={draggable && !disabled}
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
