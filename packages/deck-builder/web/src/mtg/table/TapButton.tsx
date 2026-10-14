import { Button } from "../../ui/index.ts";
import styles from "./TapButton.module.css";

/** Card orientation depicts current state; the accessible name states the action. */
export function TapButton({ name, tapped, disabled, onToggle }: {
  name: string;
  tapped: boolean;
  disabled?: boolean;
  onToggle: () => void;
}) {
  return (
    <Button
      className={styles.control}
      data-tap-control
      aria-label={`${tapped ? "Untap" : "Tap"} ${name}`}
      aria-pressed={tapped}
      title={tapped ? "Tapped · click to untap" : "Untapped · click to tap"}
      disabled={disabled}
      onClick={onToggle}
    >
      <svg className={styles.icon} viewBox="0 0 24 24" fill="none" stroke="currentColor"
        strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
        <rect x="6" y="3" width="12" height="18" rx="2" />
        <path d="M9 7h6M9 11h6" />
      </svg>
    </Button>
  );
}
