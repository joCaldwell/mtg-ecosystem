import { useEffect, useState } from "react";
import { useCardLookup } from "./CardLookup.tsx";
import type { CardInfo } from "./CardInfo.ts";
import { cardImageUrl } from "./cardImage.ts";
import styles from "./CardImage.module.css";

type Status = "loading" | "loaded" | "failed";

function Picture({ name, knownUrl, width }: { name: string; knownUrl?: string; width?: number | string }) {
  const { lookup } = useCardLookup();
  const [url, setUrl] = useState<string | undefined>(knownUrl);
  const [status, setStatus] = useState<Status>("loading");
  useEffect(() => {
    if (knownUrl) return;
    let current = true;
    void lookup(name).then(card => {
      if (!current) return;
      const stored = card ? cardImageUrl(card) : undefined;
      setUrl(stored);
      if (!stored) setStatus("failed");
    });
    return () => { current = false; };
  }, [lookup, name, knownUrl]);
  return (
    <div className={styles.frame} style={width === undefined ? undefined : { width }}>
      {status !== "loaded" && (
        <div className={styles.placeholder}>Card image loads from Scryfall when online</div>
      )}
      {status !== "failed" && url && (
        <img
          className={styles.image}
          // A cached image can finish before React attaches onLoad.
          ref={(img) => {
            if (img?.complete && img.naturalWidth > 0) setStatus("loaded");
          }}
          src={url}
          alt={name}
          loading="lazy"
          onLoad={() => setStatus("loaded")}
          onError={() => setStatus("failed")}
        />
      )}
    </div>
  );
}

/**
 * A card's picture at the card's fixed aspect ratio. Until it loads, and if it
 * never does, a dashed placeholder of the same footprint stands in so nothing
 * jumps. `width` is a CSS length or a pixel number; default fills the parent.
 */
export function CardImage({ name, card, width }: { name: string; card?: CardInfo | null; width?: number | string }) {
  const knownUrl = card ? cardImageUrl(card) : undefined;
  // A different card or supplied URL starts a fresh load/error state.
  return <Picture key={`${name}:${knownUrl ?? ""}`} name={name} knownUrl={knownUrl} width={width} />;
}
