// Mana costs come out of the database as Scryfall symbol strings — "{3}{W}{U}",
// "{B/G}", "{W/P}". Rendering them as text made every row noisy and impossible
// to scan; rendering them as pips means the curve and the colour spread of a
// list are readable at a glance without reading a single character.

const PIP: Record<string, { bg: string; fg: string }> = {
  W: { bg: "#f4eeda", fg: "#2c2617" },
  U: { bg: "#3f92d2", fg: "#04121e" },
  B: { bg: "#4c4653", fg: "#e0dae6" },
  R: { bg: "#dc6a52", fg: "#2a0f09" },
  G: { bg: "#46a06b", fg: "#062011" },
  C: { bg: "#b3b9c3", fg: "#191c22" },
  S: { bg: "#b3b9c3", fg: "#191c22" },
  P: { bg: "#6a6472", fg: "#e0dae6" },
};

// Generic/numeric ({2}, {X}) has no colour of its own — real cards print it on
// the same light grey as colorless.
const GENERIC = { bg: "#b3b9c3", fg: "#191c22" };

function faceOf(part: string) {
  return PIP[part] ?? GENERIC;
}

export function ManaCost({ cost, className = "" }: { cost?: string | null; className?: string }) {
  const symbols = cost?.match(/\{[^}]+\}/g);
  if (!symbols?.length) return null;
  return (
    <span className={`mana ${className}`} title={cost ?? undefined}>
      {symbols.map((sym, i) => {
        const body = sym.slice(1, -1);
        const parts = body.split("/");
        if (parts.length === 1) {
          const face = faceOf(body);
          return (
            <i key={i} className="pip" style={{ background: face.bg, color: face.fg }}>
              {body}
            </i>
          );
        }
        // Hybrid and Phyrexian: two-tone, no glyph. The tooltip carries the
        // exact cost, and at this size a legible "W/P" is not achievable.
        const a = faceOf(parts[0]);
        const b = faceOf(parts[1]);
        return (
          <i
            key={i}
            className="pip pip-split"
            style={{ background: `linear-gradient(135deg, ${a.bg} 0 50%, ${b.bg} 50% 100%)` }}
          />
        );
      })}
    </span>
  );
}
