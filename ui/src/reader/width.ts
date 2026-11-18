// The folder tree's width in the reader, in rem: chosen with the handle
// beside it, remembered in this browser only (a per-viewer convenience).

export const TREE_MIN = 12;
export const TREE_MAX = 36;
export const TREE_DEFAULT = 15;
const KEY = "kura:treeWidth";

/** Within the bounds, to a quarter rem; anything unreadable is the default. */
export function clampWidth(rem: number): number {
  if (!Number.isFinite(rem)) return TREE_DEFAULT;
  return Math.min(TREE_MAX, Math.max(TREE_MIN, Math.round(rem * 4) / 4));
}

export function loadWidth(): number {
  try {
    const v = localStorage.getItem(KEY);
    return v === null ? TREE_DEFAULT : clampWidth(Number(v));
  } catch {
    return TREE_DEFAULT;
  }
}

export function saveWidth(rem: number): void {
  try {
    localStorage.setItem(KEY, String(clampWidth(rem)));
  } catch {
    // Private mode: the width lasts for this page only.
  }
}
