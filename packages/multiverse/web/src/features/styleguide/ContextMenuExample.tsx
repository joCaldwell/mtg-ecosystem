import { useState } from "react";
import { Button, ContextMenu, Section, type ContextMenuTarget } from "../../ui/index.ts";

export function ContextMenuExample() {
  const [target, setTarget] = useState<ContextMenuTarget | null>(null);
  const [result, setResult] = useState("No action selected");
  return <Section title="ContextMenu" aside="Right-click or Shift+F10 · arrows navigate · Esc closes">
    <Button
      onContextMenu={(e) => {
        e.preventDefault();
        setTarget({ element: e.currentTarget, x: e.clientX, y: e.clientY });
      }}
      onKeyDown={(e) => {
        if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
        e.preventDefault();
        const bounds = e.currentTarget.getBoundingClientRect();
        setTarget({ element: e.currentTarget, x: bounds.left, y: bounds.top });
      }}
    >Right-click for actions</Button>
    <p>{result}</p>
    <ContextMenu target={target} label="Example actions" onClose={() => setTarget(null)} items={[
      { id: "copy", label: "Copy", onSelect: () => setResult("Copy selected") },
      { id: "unavailable", label: "Unavailable action", disabled: true, onSelect: () => undefined },
      { id: "delete", label: "Delete", tone: "cut", separator: true, onSelect: () => setResult("Delete selected") },
    ]} />
  </Section>;
}
