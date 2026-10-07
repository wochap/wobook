export interface Span {
  text: string;
  hit: boolean;
}

/**
 * Splits `text` (the haystack segment `[start, end)` shifted by `skip` chars
 * of prefix not displayed) into highlighted and plain spans. Indices are
 * char offsets into the whole haystack.
 */
export function spans(
  text: string,
  segment: [number, number],
  indices: number[],
  skip = 0,
): Span[] {
  const chars = Array.from(text);
  const marked = new Set<number>();
  for (const i of indices) {
    if (i >= segment[0] && i < segment[1]) marked.add(i - segment[0] - skip);
  }
  const out: Span[] = [];
  chars.forEach((ch, i) => {
    const hit = marked.has(i);
    const last = out[out.length - 1];
    if (last && last.hit === hit) last.text += ch;
    else out.push({ text: ch, hit });
  });
  return out;
}

export function render(el: HTMLElement, parts: Span[]) {
  el.replaceChildren(
    ...parts.map((p) => {
      if (!p.hit) return document.createTextNode(p.text);
      const mark = document.createElement("mark");
      mark.textContent = p.text;
      return mark;
    }),
  );
}
