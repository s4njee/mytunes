export function formatTrackTime(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function ResizeHandle(props: {
  axis: "x" | "y";
  label: string;
  onResize: (size: number) => void;
}) {
  let start: { point: number; size: number } | undefined;
  const position = (event: PointerEvent) => (props.axis === "x" ? event.clientX : event.clientY);
  const dimension = (handle: HTMLButtonElement) => {
    const bounds = handle.parentElement?.getBoundingClientRect();
    return Math.round((props.axis === "x" ? bounds?.width : bounds?.height) ?? 0);
  };
  return (
    <button
      type="button"
      class={`resize-handle resize-${props.axis}`}
      aria-label={props.label}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        start = { point: position(event), size: dimension(event.currentTarget) };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerUp={(event) => {
        if (!start) return;
        const change = Math.round(position(event) - start.point);
        if (change) props.onResize(start.size + change);
        start = undefined;
      }}
      onPointerCancel={() => {
        start = undefined;
      }}
      onKeyDown={(event) => {
        const less = props.axis === "x" ? "ArrowLeft" : "ArrowUp";
        const more = props.axis === "x" ? "ArrowRight" : "ArrowDown";
        if (event.key === less || event.key === more) {
          event.preventDefault();
          props.onResize(dimension(event.currentTarget) + (event.key === less ? -8 : 8));
        }
      }}
    />
  );
}
