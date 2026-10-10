import { useState } from "react";
import { cardImageUrl } from "./cardImage.ts";
import styles from "./CardImage.module.css";

type Status = "loading" | "loaded" | "failed";

function Picture({ name, width }: { name: string; width?: number | string }) {
  const [status, setStatus] = useState<Status>("loading");
  return (
    <div className={styles.frame} style={width === undefined ? undefined : { width }}>
      {status !== "loaded" && (
        <div className={styles.placeholder}>Card image loads from Scryfall when online</div>
      )}
      {status !== "failed" && (
        <img
          className={styles.image}
          // A cached image can finish before React attaches onLoad.
          ref={(img) => {
            if (img?.complete && img.naturalWidth > 0) setStatus("loaded");
          }}
          src={cardImageUrl(name)}
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
export function CardImage({ name, width }: { name: string; width?: number | string }) {
  // Keyed by name so a new card starts from "loading" instead of inheriting the last one's state.
  return <Picture key={name} name={name} width={width} />;
}
