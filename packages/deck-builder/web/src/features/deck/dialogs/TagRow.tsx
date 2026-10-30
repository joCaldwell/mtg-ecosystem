import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { Tag } from "../../../api/client.ts";
import { Button, TextField } from "../../../ui/index.ts";
import { ConfirmStrip } from "./ConfirmStrip.tsx";
import styles from "./TagsSection.module.css";

interface Props {
  tag: Tag;
  /** Resolves false when the server refused, so the typed text stays for fixing. */
  onRename: (tagId: number, name: string) => Promise<boolean>;
  onDelete: (tagId: number) => void;
  /** An edit that was refused and then taken back leaves no error behind. */
  onRevert: () => void;
}

export function TagRow({ tag, onRename, onDelete, onRevert }: Props) {
  const [name, setName] = useState(tag.name);
  const [confirming, setConfirming] = useState(false);
  const saving = useRef(false);
  useEffect(() => setName(tag.name), [tag.name]);

  async function commit() {
    if (saving.current) return;
    const next = name.trim();
    if (next === tag.name || next === "") return setName(tag.name);
    saving.current = true;
    try {
      await onRename(tag.id, next);
    } finally {
      saving.current = false;
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      void commit();
    } else if (e.key === "Escape" && name !== tag.name) {
      e.stopPropagation();
      setName(tag.name);
      onRevert();
    }
  }

  return (
    <li className={styles.item}>
      <div className={styles.row}>
        <TextField
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => void commit()}
          onKeyDown={onKeyDown}
          aria-label={`Name of the tag ${tag.name}`}
        />
        <Button size="sm" variant="reject" onClick={() => setConfirming(true)} aria-label={`Delete tag ${tag.name}`}>
          Delete
        </Button>
      </div>
      {confirming ? (
        <ConfirmStrip confirmLabel="Delete tag" onCancel={() => setConfirming(false)} onConfirm={() => onDelete(tag.id)}>
          Delete the tag <strong>{tag.name}</strong>? It is taken off every card that has it. The cards stay in the
          deck.
        </ConfirmStrip>
      ) : null}
    </li>
  );
}
