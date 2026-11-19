const icons = {
  music:
    "M9 18V5l11-2v13M9 18a3 2 0 1 1-6 0 3 2 0 1 1 6 0ZM20 16a3 2 0 1 1-6 0 3 2 0 1 1 6 0ZM9 9l11-2",
  phone: "M7 3h10a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm3 15h4",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-14v5l3 2",
  playlist: "M4 6h16M4 11h12M4 16h8m5-2 4 3-4 3Z",
  search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13ZM16 16l4 4",
  browse: "M4 5h16v14H4zM9.3 5v14m5.4-14v14",
  add: "M12 5v14M5 12h14",
  sync: "M4 8h15m-4-4 4 4-4 4m5 4H5m4-4-4 4 4 4",
  refresh: "M20 7v5h-5m4.7-1a8 8 0 1 0-1.3 6M20 6v6",
  art: "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4m-5-7h.01",
  settings:
    "M10 3h4l.6 2.4 2 .9 2.2-1.2 2 3.4-1.7 1.7.2 2.3 1.6 1.8-2 3.4-2.3-.9-2 .9-.6 2.3h-4l-.6-2.3-2-.9-2.3.9-2-3.4 1.6-1.8.2-2.3L3.2 8.5l2-3.4 2.2 1.2 2-.9L10 3Zm2 12a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
  eject: "M5 14h14L12 6ZM5 18h14",
  speaker: "M4 9h4l5-4v14l-5-4H4ZM17 8a6 6 0 0 1 0 8m2-10a9 9 0 0 1 0 12",
  wave: "M4 10v4m4-7v10m4-13v16m4-13v10m4-7v4",
  arrow: "m8 10 4 4 4-4",
  check: "m5 12 4 4 10-10",
  previous: "M5 6h2v12H5zM18 5v14L8 12z",
  next: "M17 6h2v12h-2zM6 5v14l10-7z",
  play: "m8 4 12 8-12 8z",
  pause: "M7 5h4v14H7zm6 0h4v14h-4z",
} as const;

export type IconName = keyof typeof icons;

export function Icon(props: { name: IconName; solid?: boolean }) {
  return (
    <svg
      class="library-icon"
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill={props.solid ? "currentColor" : "none"}
      stroke={props.solid ? "none" : "currentColor"}
      stroke-width="1.7"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d={icons[props.name]} />
    </svg>
  );
}
