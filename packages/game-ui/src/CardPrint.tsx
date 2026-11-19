import { useState } from 'react';

function printSource(value: string | undefined): string | undefined {
  if (!value) return;
  try {
    const url = new URL(value);
    if (url.origin === 'https://cards.scryfall.io' && !url.username && !url.password && /^\/normal\/(front|back)\//.test(url.pathname)) return value;
  } catch {
    return undefined;
  }
}

/** Display a supplied printing without substituting a lookup or cropping its credit. */
export function CardPrint({ source, className }: { source: string | undefined; className: string | undefined }) {
  const src = printSource(source);
  return src ? <PrintImage key={src} src={src} className={className} /> : null;
}

function PrintImage({ src, className }: { src: string; className: string | undefined }) {
  const [state, setState] = useState<'pending' | 'ready' | 'failed'>('pending');
  return state !== 'failed' && <img className={className} data-loaded={state === 'ready' || undefined}
    src={src} alt="" decoding="async" loading="lazy" referrerPolicy="no-referrer"
    onLoad={() => setState('ready')} onError={() => setState('failed')} />;
}
