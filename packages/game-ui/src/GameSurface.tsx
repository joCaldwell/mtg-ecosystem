import type { ComponentPropsWithoutRef } from 'react';
import styles from './surface.module.css';

/** Scoped presentation defaults; never changes the host document or account. */
export function GameSurface({ className, ...props }: ComponentPropsWithoutRef<'div'>) {
  return <div className={`${styles.surface} ${className ?? ''}`} {...props} />;
}
