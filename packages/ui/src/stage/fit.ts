// SPDX-License-Identifier: GPL-3.0-or-later
// Svelte action: sizes text so it fills its parent without overflowing.

export interface FitParams {
  /** Re-fit when this changes. */
  text: string;
  /** Fixed size in percent of the parent's height; `null` fits automatically. */
  size: number | null;
  /** Largest automatic size, in percent of the parent's height. */
  max?: number;
}

export function fitText(node: HTMLElement, params: FitParams) {
  let current = params;
  const fit = () => {
    const parent = node.parentElement;
    if (!parent) return;
    const h = parent.clientHeight;
    if (current.size !== null) {
      node.style.fontSize = `${(current.size / 100) * h}px`;
      return;
    }
    // Binary search for the largest size that fits.
    let lo = 4;
    let hi = Math.max(8, current.max !== undefined ? (current.max / 100) * h : h);
    while (hi - lo > 0.5) {
      const mid = (lo + hi) / 2;
      node.style.fontSize = `${mid}px`;
      const fits =
        node.scrollHeight <= parent.clientHeight && node.scrollWidth <= parent.clientWidth;
      if (fits) lo = mid;
      else hi = mid;
    }
    node.style.fontSize = `${lo}px`;
  };
  const observer = new ResizeObserver(fit);
  if (node.parentElement) observer.observe(node.parentElement);
  // Wait for web fonts so the measurement is right.
  void document.fonts?.ready.then(fit);
  fit();
  return {
    update(next: FitParams) {
      current = next;
      fit();
    },
    destroy() {
      observer.disconnect();
    },
  };
}
