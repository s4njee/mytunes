import type { OpenedView, SelectionStats, ViewTotals } from "../bindings";

const decimal = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const storage = [
  [1_000_000_000_000, "TB"],
  [1_000_000_000, "GB"],
  [1_000_000, "MB"],
  [1_000, "KB"],
  [1, "B"],
] as const;
const size = new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 });

function durationLabel(milliseconds: number) {
  const minutes = milliseconds / 60_000;
  if (minutes >= 1_440) {
    const days = decimal.format(minutes / 1_440);
    return `${days} ${days === "1" ? "day" : "days"}`;
  }
  if (minutes >= 60) {
    const hours = decimal.format(minutes / 60);
    return `${hours} ${hours === "1" ? "hour" : "hours"}`;
  }
  return minutes >= 1 || milliseconds === 0
    ? `${decimal.format(minutes)} min`
    : `${decimal.format(milliseconds / 1_000)} sec`;
}

function sizeLabel(bytes: number) {
  const [scale, unit] = storage.find(([threshold]) => bytes >= threshold) ?? [1, "B"];
  return `${size.format(bytes / scale)} ${unit}`;
}

function totalsLabel(totals: ViewTotals) {
  return `${durationLabel(totals.duration_ms)}, ${sizeLabel(totals.size_bytes)}`;
}

export function viewSummary(view: OpenedView, selected?: SelectionStats) {
  const count = view.count.toLocaleString();
  if (selected && selected.count > 1) {
    return `${selected.count.toLocaleString()} of ${count} songs selected, ${totalsLabel(selected.totals)}`;
  }
  return `${count} ${view.count === 1 ? "song" : "songs"}, ${totalsLabel(view.totals)}`;
}
