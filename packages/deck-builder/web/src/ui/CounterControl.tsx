import { Button } from "./Button.tsx";
import styles from "./CounterControl.module.css";
export function CounterControl({
  label,
  value,
  onChange,
  disabled = false,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  disabled?: boolean;
}) {
  return (
    <span className={styles.control} aria-label={label}>
      <Button
        size="sm"
        disabled={disabled}
        aria-label={`Decrease ${label}`}
        onClick={() => onChange(value - 1)}
      >
        −
      </Button>
      <output aria-label={label}>{value}</output>
      <Button
        size="sm"
        disabled={disabled}
        aria-label={`Increase ${label}`}
        onClick={() => onChange(value + 1)}
      >
        +
      </Button>
    </span>
  );
}
