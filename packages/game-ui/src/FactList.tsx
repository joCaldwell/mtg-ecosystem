/** Generic display primitive; no game or protocol knowledge. */
export function FactList({ items, className }: { items: readonly { readonly label: string; readonly value: string }[]; className?: string | undefined }) {
  return <dl className={className}>{items.map(item => <div key={item.label}><dt>{item.label}</dt><dd>{item.value}</dd></div>)}</dl>;
}
