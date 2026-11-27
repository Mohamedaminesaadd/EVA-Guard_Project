/**
 * Categorical palette for node identity in time-series charts.
 * Validated (light surface #FFFFFF): lightness band, chroma floor, adjacent CVD ΔE ≥ 8,
 * normal-vision ΔE ≥ 15, contrast ≥ 3:1. Status colours (green/amber/orange/red) are
 * deliberately excluded so a node line is never mistaken for a severity.
 * Colour follows the node (by its id), never its rank in a filtered list.
 */
export const NODE_SERIES_COLORS = ['#2E6FDB', '#0B9A8D', '#7A5AF8', '#946200', '#D9508F'] as const;

/** Fallback for nodes beyond the 5th: neutral, distinguished by line dash. */
export const NODE_OVERFLOW_COLOR = '#667085';

export function nodeColor(nodeId: string): string {
  const m = /(\d+)\s*$/.exec(nodeId);
  if (m) {
    const idx = parseInt(m[1], 10) - 1;
    if (idx >= 0 && idx < NODE_SERIES_COLORS.length) return NODE_SERIES_COLORS[idx];
    return NODE_OVERFLOW_COLOR;
  }
  let h = 0;
  for (const ch of nodeId) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return NODE_SERIES_COLORS[h % NODE_SERIES_COLORS.length];
}

export function nodeDash(nodeId: string): 'solid' | 'dashed' {
  return nodeColor(nodeId) === NODE_OVERFLOW_COLOR ? 'dashed' : 'solid';
}
