import type { ReactNode } from 'react';
import styles from './surface.module.css';

/** Scoped presentation defaults; never changes the host document or account. */
export function GameSurface({ children, className }: { children: ReactNode; className?: string | undefined }) {
  return <div className={`${styles.surface} ${className ?? ''}`}>{children}</div>;
}
