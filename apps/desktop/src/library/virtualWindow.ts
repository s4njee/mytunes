const OVERSCAN = 6;
// One header and up to two spacer rows also occupy the table.
export const MAX_RENDERED_TRACKS = 77;

/** The visible tracks and overscan, using a half-open range. */
export function virtualWindow(scrollTop: number, height: number, rowHeight: number, count: number) {
  const top = Math.max(0, Math.floor(scrollTop / rowHeight));
  const first = Math.max(0, Math.min(top, count) - OVERSCAN);
  const visible = Math.ceil(Math.max(1, height) / rowHeight) + 1;
  const end = Math.min(count, first + MAX_RENDERED_TRACKS, top + visible + OVERSCAN);
  return { first, end };
}
