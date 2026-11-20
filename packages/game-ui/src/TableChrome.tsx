import type { ComponentPropsWithoutRef, ReactNode, RefObject } from 'react';
import surface from './surface.module.css';
import styles from './TableChrome.module.css';

export function TableHeader({ title, headingLevel = 1, leading, status, actions, className }: {
  title: string;
  headingLevel?: 1 | 2;
  leading?: ReactNode;
  status?: ReactNode;
  actions?: ReactNode;
  className?: string | undefined;
}) {
  const Heading = headingLevel === 1 ? 'h1' : 'h2';
  return <header className={`${styles.header} ${className ?? ''}`}>
    <div className={styles.identity}>{leading}<Heading>{title}</Heading></div>
    {status}
    {actions && <div className={styles.actions}>{actions}</div>}
  </header>;
}

export function TableMat({ className, ...props }: ComponentPropsWithoutRef<'div'>) {
  return <div className={`${styles.mat} ${className ?? ''}`} {...props} />;
}

export function TableRail({ as: Element = 'div', className, ...props }: ComponentPropsWithoutRef<'aside'> & { as?: 'aside' | 'div' }) {
  return <Element className={`${styles.rail} ${className ?? ''}`} {...props} />;
}

export function TablePrompt({ sectionRef, className, ...props }: ComponentPropsWithoutRef<'section'> & { sectionRef?: RefObject<HTMLElement | null> }) {
  return <section ref={sectionRef} className={`${styles.prompt} ${className ?? ''}`} {...props} />;
}

export function TableSectionHeading({ title, id, count, actions, className }: {
  title: string;
  id?: string;
  count?: number;
  actions?: ReactNode;
  className?: string | undefined;
}) {
  return <div className={`${surface.surface} ${styles.sectionHeading} ${className ?? ''}`}>
    <h2 id={id}>{title}{count !== undefined && <span aria-hidden="true">{count}</span>}</h2>
    {actions}
  </div>;
}
