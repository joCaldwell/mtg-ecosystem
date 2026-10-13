import type { ReactNode } from "react";
import { Badge } from "../../ui/index.ts";
import styles from "./TableZone.module.css";
export function TableZone({
  label,
  count,
  actions,
  children,
  onDrop,
  emptyText = "Move cards here",
  variant = "row",
  showHeader = true,
}: {
  label: string;
  count: number;
  actions?: ReactNode;
  children?: ReactNode;
  onDrop?: (id: string) => void;
  emptyText?: string;
  variant?: "row" | "hand";
  showHeader?: boolean;
}) {
  return (
    <section
      className={`${styles.zone} ${variant === "hand" ? styles.hand : ""}`}
      aria-label={label}
      onDragOver={(e) => {
        if (
          onDrop &&
          Array.from(e.dataTransfer.types).includes("application/x-mtg-object")
        ) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        }
      }}
      onDrop={(e) => {
        const id = e.dataTransfer.getData("application/x-mtg-object");
        if (id && onDrop) {
          e.preventDefault();
          onDrop(id);
        }
      }}
    >
      {showHeader && (
        <header className={styles.head}>
          <h2>{label}</h2>
          <Badge>{count}</Badge>
          <div className={styles.actions}>{actions}</div>
        </header>
      )}
      <div className={styles.cards}>
        {children}
        {!count && emptyText && <p className={styles.empty}>{emptyText}</p>}
      </div>
    </section>
  );
}
