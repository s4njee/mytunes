import { For, Show } from "solid-js";

export interface TrackMenuOption {
  label: string;
  checked?: boolean;
  action: () => void;
}

export function TrackMenu(props: {
  label: string;
  x: number;
  y: number;
  options: readonly TrackMenuOption[];
  onClose: (restoreFocus: boolean) => void;
}) {
  function choose(option: TrackMenuOption) {
    option.action();
    props.onClose(true);
  }

  return (
    <div
      class="track-menu"
      role="menu"
      aria-label={props.label}
      style={{
        left: `min(${props.x}px, calc(100vw - 15rem))`,
        top: `min(${props.y}px, calc(100vh - ${Math.min(320, 32 * props.options.length + 8)}px))`,
      }}
      ref={(menu) => queueMicrotask(() => menu.querySelector("button")?.focus())}
      onFocusOut={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) props.onClose(false);
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          props.onClose(true);
        }
        if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
        const items = Array.from(event.currentTarget.querySelectorAll("button"));
        const current = items.indexOf(document.activeElement as HTMLButtonElement);
        const offset = event.key === "ArrowDown" ? 1 : -1;
        items[(current + offset + items.length) % items.length]?.focus();
        event.preventDefault();
      }}
    >
      <For each={props.options}>
        {(option) => (
          <Show
            when={option.checked !== undefined}
            fallback={
              <button type="button" role="menuitem" onClick={() => choose(option)}>
                {option.label}
              </button>
            }
          >
            <button
              type="button"
              role="menuitemcheckbox"
              aria-checked={option.checked ? "true" : "false"}
              onClick={() => choose(option)}
            >
              <span aria-hidden="true">{option.checked ? "✓" : ""}</span>
              {option.label}
            </button>
          </Show>
        )}
      </For>
    </div>
  );
}
