import { createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { BrowserOption } from "../bindings";
import { ResizeHandle } from "./controls";
import { MAX_RENDERED_TRACKS, virtualWindow } from "./virtualWindow";

export type FacetLabel = "Genre" | "Artist" | "Album";
export const facetLabels: FacetLabel[] = ["Genre", "Artist", "Album"];

export interface Facet {
  label: FacetLabel;
  values: readonly BrowserOption[];
  active: BrowserOption | null;
  allLabel?: string;
}

const slots = Array.from({ length: MAX_RENDERED_TRACKS }, (_, index) => index);

function FacetColumn(props: {
  facet: Facet;
  onSelect?: (label: FacetLabel, value: BrowserOption | null) => void;
}) {
  const [top, setTop] = createSignal(0);
  const [height, setHeight] = createSignal(120);
  const [rowHeight, setRowHeight] = createSignal(25);
  let list: HTMLUListElement | undefined;
  let resize: ResizeObserver | undefined;
  onCleanup(() => resize?.disconnect());

  const count = () => props.facet.values.length + 1;
  const range = createMemo(() => virtualWindow(top(), height(), rowHeight(), count()));
  const attachList = (element: HTMLUListElement) => {
    list = element;
    const measure = () => {
      const style = getComputedStyle(element);
      setRowHeight(Number.parseFloat(style.getPropertyValue("--mt-desktop-size-facet-item")) || 25);
      setHeight(element.clientHeight);
    };
    measure();
    if (typeof ResizeObserver !== "undefined") {
      resize = new ResizeObserver(measure);
      resize.observe(element);
    }
  };

  function keyboard(event: KeyboardEvent, index: number) {
    if (!list) return;
    const offsets = { ArrowUp: index - 1, ArrowDown: index + 1, Home: 0, End: count() - 1 };
    if (!(event.key in offsets)) return;
    event.preventDefault();
    const next = Math.max(0, Math.min(offsets[event.key as keyof typeof offsets], count() - 1));
    const position = next * rowHeight();
    if (position < list.scrollTop) list.scrollTop = position;
    else if (position + rowHeight() > list.scrollTop + height()) {
      list.scrollTop = position + rowHeight() - height();
    }
    setTop(list.scrollTop);
    requestAnimationFrame(() =>
      list?.querySelector<HTMLButtonElement>(`[data-facet-index="${next}"]`)?.focus(),
    );
  }

  const active = (value: BrowserOption | null) =>
    value?.name === props.facet.active?.name &&
    value?.compilation === props.facet.active?.compilation;

  return (
    <section class="facet" aria-label={props.facet.label}>
      <h2>{props.facet.label}</h2>
      <ul ref={attachList} onScroll={(event) => setTop(event.currentTarget.scrollTop)}>
        <Show when={range().first > 0}>
          <li inert style={{ height: `${range().first * rowHeight()}px` }} />
        </Show>
        <For each={slots.slice(0, range().end - range().first)}>
          {(slot) => {
            const index = () => range().first + slot;
            const value = () => props.facet.values[index() - 1] ?? null;
            return (
              <li>
                <button
                  type="button"
                  data-facet-index={index()}
                  aria-pressed={active(value()) ? "true" : "false"}
                  onClick={() => props.onSelect?.(props.facet.label, value())}
                  onKeyDown={(event) => keyboard(event, index())}
                >
                  {value()?.name ??
                    props.facet.allLabel ??
                    `All (${props.facet.values.length.toLocaleString()})`}
                </button>
              </li>
            );
          }}
        </For>
        <Show when={range().end < count()}>
          <li inert style={{ height: `${(count() - range().end) * rowHeight()}px` }} />
        </Show>
      </ul>
    </section>
  );
}

export function ColumnBrowser(props: {
  facets: readonly Facet[];
  onSelect?: (label: FacetLabel, value: BrowserOption | null) => void;
  onResize?: (height: number) => void;
}) {
  return (
    <section class="column-browser" aria-label="Column browser">
      <For each={facetLabels}>
        {(label) => (
          <Show when={props.facets.find((facet) => facet.label === label)}>
            {(facet) => <FacetColumn facet={facet()} onSelect={props.onSelect} />}
          </Show>
        )}
      </For>
      <Show when={props.onResize}>
        {(resize) => <ResizeHandle axis="y" label="Resize column browser" onResize={resize()} />}
      </Show>
    </section>
  );
}
