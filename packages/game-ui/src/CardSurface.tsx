import type { ComponentPropsWithoutRef } from 'react';
import surface from './surface.module.css';
import styles from './CardSurface.module.css';

type CardSurfaceProps = ({ as: 'button' } & ComponentPropsWithoutRef<'button'>)
  | ({ as?: 'div' | 'article' } & ComponentPropsWithoutRef<'div'>);

export function CardSurface(props: CardSurfaceProps) {
  if (props.as === 'button') {
    const { as: Button, className, ...button } = props;
    return <Button className={`${surface.surface} ${styles.surface} ${className ?? ''}`} {...button} />;
  }
  const { as: Element = 'div', className, ...element } = props;
  return <Element className={`${surface.surface} ${styles.surface} ${className ?? ''}`} {...element} />;
}
