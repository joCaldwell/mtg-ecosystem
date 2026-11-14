import { Button, useToast } from "../../ui/index.ts";

/** Copies `text` to the clipboard and says so. `label` names what is copied, for screen readers. */
export function CopyButton({ text, label }: { text: string; label: string }) {
  const { toast } = useToast();
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      toast("Copied.", { duration: 2000 });
    } catch {
      toast("Could not copy. Select the text and copy it by hand.");
    }
  }
  return (
    <Button size="sm" onClick={copy} aria-label={`Copy ${label}`}>
      Copy
    </Button>
  );
}
