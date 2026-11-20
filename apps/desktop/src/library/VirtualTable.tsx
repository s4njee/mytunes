import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { ColumnPreference_Serialize, TrackColumn, TrackSort } from "../bindings";
import { formatTrackTime, ResizeHandle } from "./controls";
import { Icon } from "./Icon";
import { TrackMenu, type TrackMenuOption } from "./TrackMenu";
import { MAX_RENDERED_TRACKS, virtualWindow } from "./virtualWindow";

export interface Track {
  id: string;
  name: string;
  seconds: number;
  artist: string;
  album: string;
  albumArtist?: string;
  genre: string;
  year?: number;
  trackNumber?: number;
  lastPlayed?: number;
  dateAdded?: number;
  kind?: string;
  size?: number;
  rating: number;
  plays: number;
  checked: boolean;
}

export interface TrackTableProps {
  tracks: readonly Track[];
  trackCount?: number;
  trackAt?: (index: number) => Track | undefined;
  trackViewId?: string;
  trackScrollKey?: string;
  onTrackWindow?: (first: number, end: number) => void;
  onSelectionChange?: (indexes: number[]) => void;
  onPlayIndex?: (index: number) => void;
  onToggleTrackPlayback?: (index: number) => void;
  onRevealTrack?: (track: Track) => void;
  onTrackChecked?: (track: Track, checked: boolean) => void;
  onRemoveTracks?: (viewId: string, indexes: number[], trash: boolean) => void;
  track?: Track;
  selectedId: string;
  sort?: TrackSort;
  columnPreferences?: ColumnPreference_Serialize[];
  onColumnResize?: (column: TrackColumn, width: number) => void;
  onColumnVisibility?: (column: TrackColumn, visible: boolean) => void;
  onSort?: (sort: TrackSort) => void;
  onSelect: (track: Track) => void;
  onPlayTrack?: (track: Track) => void;
}

export const trackColumns = [
  { key: "checked", label: "Include in sync", width: "checkbox", numeric: false },
  { key: "title", label: "Name", width: "name", numeric: false },
  { key: "duration", label: "Time", width: "time", numeric: true },
  { key: "artist", label: "Artist", width: "artist", numeric: false },
  { key: "album", label: "Album", width: "album", numeric: false },
  {
    key: "album_artist",
    label: "Album Artist",
    width: "album-artist",
    numeric: false,
    extra: true,
  },
  { key: "genre", label: "Genre", width: "genre", numeric: false },
  { key: "year", label: "Year", width: "year", numeric: true, extra: true },
  { key: "track_number", label: "Track #", width: "track-number", numeric: true, extra: true },
  { key: "rating", label: "Rating", width: "rating", numeric: false },
  { key: "play_count", label: "Plays", width: "plays", numeric: true },
  { key: "last_played", label: "Last Played", width: "last-played", numeric: false, extra: true },
  { key: "date_added", label: "Date Added", width: "date-added", numeric: false, extra: true },
  { key: "kind", label: "Kind", width: "kind", numeric: false, extra: true },
  { key: "size", label: "Size", width: "size", numeric: true, extra: true },
] as const;

type VisibleColumn = (typeof trackColumns)[number]["key"];
const slots = Array.from({ length: MAX_RENDERED_TRACKS }, (_, index) => index);
const defaultRowHeight = 34;
const date = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

export function defaultColumnVisible(column: (typeof trackColumns)[number]) {
  return !("extra" in column);
}

function TrackCell(props: {
  row?: Track;
  column: VisibleColumn;
  grid: boolean;
  selectedId: string;
  playingId?: string;
  onSelect: TrackTableProps["onSelect"];
  onPlayTrack?: TrackTableProps["onPlayTrack"];
  onTrackChecked?: TrackTableProps["onTrackChecked"];
}) {
  const row = () => props.row;
  switch (props.column) {
    case "checked":
      return (
        <td class="include-track">
          <input
            type="checkbox"
            disabled={!row()}
            tabindex={props.grid ? -1 : undefined}
            checked={row()?.checked ?? false}
            aria-label={row() ? `Sync ${row()?.name}` : "Include in sync"}
            onChange={(event) => {
              const track = row();
              if (!track || !props.onTrackChecked) return;
              const checked = event.currentTarget.checked;
              event.currentTarget.checked = track.checked;
              props.onTrackChecked(track, checked);
            }}
          />
        </td>
      );
    case "title":
      return (
        <td>
          <button
            type="button"
            class="track-name"
            aria-current={row()?.id === props.selectedId ? "true" : undefined}
            disabled={!row()}
            tabindex={props.grid ? -1 : undefined}
            onClick={() => {
              const track = row();
              if (!props.grid && track) props.onSelect(track);
            }}
            onDblClick={() => {
              const track = row();
              if (!props.grid && track) props.onPlayTrack?.(track);
            }}
          >
            <span class="track-speaker">
              <Show when={row() && row()?.id === props.playingId}>
                <Icon name="speaker" />
              </Show>
            </span>
            <span>{row()?.name ?? "…"}</span>
          </button>
        </td>
      );
    case "duration":
      return <td class="track-numeric">{row() ? formatTrackTime(row()?.seconds ?? 0) : ""}</td>;
    case "artist":
    case "album":
    case "genre": {
      const field = props.column;
      return <td title={row()?.[field]}>{row()?.[field]}</td>;
    }
    case "album_artist":
      return <td title={row()?.albumArtist}>{row()?.albumArtist}</td>;
    case "year":
      return <td class="track-numeric">{row()?.year}</td>;
    case "track_number":
      return <td class="track-numeric">{row()?.trackNumber}</td>;
    case "last_played":
    case "date_added": {
      const field = props.column === "last_played" ? "lastPlayed" : "dateAdded";
      const value = () => row()?.[field];
      return <td>{value() == null ? "" : date.format(value())}</td>;
    }
    case "kind":
      return <td title={row()?.kind}>{row()?.kind}</td>;
    case "size":
      return (
        <td class="track-numeric">
          {row()?.size == null ? "" : `${row()?.size?.toLocaleString()} B`}
        </td>
      );
    case "rating": {
      const rating = () => row()?.rating ?? 0;
      return (
        <td>
          <Show when={row()}>
            <span class="rating" role="img" aria-label={`${rating()} out of 5 stars`}>
              <span aria-hidden="true">{"★".repeat(rating())}</span>
              <span class="rating-empty" aria-hidden="true">
                {"★".repeat(5 - rating())}
              </span>
            </span>
          </Show>
        </td>
      );
    }
    case "play_count":
      return <td class="track-numeric">{row()?.plays}</td>;
  }
}

/** Pool a fixed number of track rows over the full scroll extent. */
export function VirtualTable(props: TrackTableProps) {
  const count = () => props.trackCount ?? props.tracks.length;
  const track = (index: number) => props.trackAt?.(index) ?? props.tracks[index];
  const interactive = () => props.onPlayIndex !== undefined;
  const [scrollTop, setScrollTop] = createSignal(0);
  const [height, setHeight] = createSignal(600);
  const [rowHeight, setRowHeight] = createSignal(defaultRowHeight);
  const [cursor, setCursor] = createSignal(-1);
  const [selected, setSelected] = createSignal(new Set<number>());
  const [menu, setMenu] = createSignal<{
    x: number;
    y: number;
    kind: "columns" | "track";
    track?: Track;
    source: HTMLElement;
  }>();
  const [removal, setRemoval] = createSignal<{ viewId: string; indexes: number[] }>();
  const [trash, setTrash] = createSignal(false);
  let anchor = -1;
  let viewport: HTMLDivElement | undefined;
  let resize: ResizeObserver | undefined;
  let table: HTMLTableElement | undefined;
  let confirm: HTMLDialogElement | undefined;
  const range = createMemo(() => virtualWindow(scrollTop(), height(), rowHeight(), count()));
  const extent = () => slots.slice(0, range().end - range().first);
  const attachViewport = (element: HTMLDivElement) => {
    viewport = element;
    const measure = () => {
      const style = getComputedStyle(element);
      setRowHeight(
        Number.parseFloat(style.getPropertyValue("--mt-desktop-size-row")) || defaultRowHeight,
      );
      const header = element.querySelector("thead");
      setHeight(element.clientHeight - (header?.offsetHeight ?? 0));
    };
    measure();
    if (typeof ResizeObserver !== "undefined") {
      resize = new ResizeObserver(measure);
      resize.observe(element);
    }
  };
  onCleanup(() => resize?.disconnect());
  createEffect(
    () => props.trackScrollKey,
    () => {
      if (viewport) viewport.scrollTop = 0;
      setScrollTop(0);
    },
  );
  createEffect(
    () => props.trackViewId,
    () => {
      setCursor(-1);
      setSelected(new Set<number>());
      anchor = -1;
    },
  );
  createEffect(
    () => [props.trackViewId, range().first, range().end] as const,
    ([, first, end]) => props.onTrackWindow?.(first, end),
  );
  createEffect(
    () => track(cursor()),
    (row) => {
      if (row) props.onSelect(row);
    },
  );
  const preference = (column: TrackColumn) =>
    props.columnPreferences?.find((item) => item.column === column);
  const visible = () =>
    trackColumns.filter(
      (column) => preference(column.key)?.visible ?? defaultColumnVisible(column),
    );
  const direction = (column: TrackColumn) =>
    props.sort?.column === column ? props.sort.direction : undefined;
  const rowAt = (target: EventTarget | null) => {
    const value = (target as Element | null)?.closest<HTMLElement>("[data-track-index]")?.dataset
      .trackIndex;
    return value == null ? undefined : Number(value);
  };

  function select(index: number, shift: boolean, toggle: boolean) {
    const next = toggle ? new Set(selected()) : new Set<number>();
    if (shift && anchor >= 0) {
      const end = Math.max(anchor, index);
      for (let row = Math.min(anchor, index); row <= end; row += 1) next.add(row);
    } else {
      if (toggle && next.has(index)) next.delete(index);
      else next.add(index);
      anchor = index;
    }
    setSelected(next);
    setCursor(index);
    if (props.onSelectionChange) props.onSelectionChange(Array.from(next));
  }

  function scrollTo(index: number) {
    if (!viewport) return;
    const top = index * rowHeight();
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight() > viewport.scrollTop + height()) {
      viewport.scrollTop = top + rowHeight() - height();
    }
    setScrollTop(viewport.scrollTop);
  }

  function openMenu(target: Element, x: number, y: number) {
    const header = target.closest("th");
    if (header && props.onColumnVisibility && table) {
      const source = header.querySelector("button") ?? table;
      setMenu({ kind: "columns", x, y, source });
      return true;
    }
    const index = rowAt(target) ?? (target === table ? cursor() : -1);
    const row = track(index);
    if (row && props.onRevealTrack && table) {
      setMenu({ kind: "track", x, y, track: row, source: table });
      return true;
    }
    return false;
  }

  function menuItems(value: NonNullable<ReturnType<typeof menu>>): TrackMenuOption[] {
    if (value.kind === "columns") {
      return trackColumns.map((column) => {
        const checked = preference(column.key)?.visible ?? defaultColumnVisible(column);
        return {
          label: column.label,
          checked,
          action: () => props.onColumnVisibility?.(column.key, !checked),
        };
      });
    }
    const row = value.track;
    if (!row) return [];
    return [
      { label: "Show in File Manager", action: () => props.onRevealTrack?.(row) },
      {
        label: "Include in sync",
        checked: row.checked,
        action: () => props.onTrackChecked?.(row, !row.checked),
      },
    ];
  }

  function openRemoval() {
    const viewId = props.trackViewId;
    if (!confirm || !props.onRemoveTracks || !viewId || selected().size === 0) return;
    setTrash(false);
    setRemoval({ viewId, indexes: Array.from(selected()) });
    confirm.returnValue = "";
    confirm.showModal();
  }

  function keyboard(event: KeyboardEvent) {
    const target = event.target as Element;
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      const rect = target.getBoundingClientRect();
      if (openMenu(target, rect.left + 8, rect.top + 24)) event.preventDefault();
      return;
    }
    if (!interactive() || event.target !== table || count() === 0) return;
    const index = cursor();
    let next = index;
    switch (event.key) {
      case "ArrowDown":
        next += 1;
        break;
      case "ArrowUp":
        next -= 1;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = count() - 1;
        break;
      case "PageDown":
        next += Math.max(1, Math.floor(height() / rowHeight()));
        break;
      case "PageUp":
        next -= Math.max(1, Math.floor(height() / rowHeight()));
        break;
      case "Enter":
        props.onPlayIndex?.(Math.max(0, index));
        event.preventDefault();
        return;
      case " ":
        props.onToggleTrackPlayback?.(Math.max(0, index));
        event.preventDefault();
        return;
      case "Delete":
      case "Backspace":
        openRemoval();
        event.preventDefault();
        return;
      default:
        return;
    }
    next = Math.max(0, Math.min(count() - 1, next));
    select(next, event.shiftKey, false);
    scrollTo(next);
    event.preventDefault();
  }

  const activeDescendant = () =>
    cursor() >= range().first && cursor() < range().end ? `track-row-${cursor()}` : undefined;
  const gridAttributes = () =>
    interactive()
      ? {
          role: "grid" as const,
          tabindex: 0,
          "aria-multiselectable": "true" as const,
          "aria-activedescendant": activeDescendant(),
        }
      : {};

  return (
    <div
      class="track-scroll"
      ref={attachViewport}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <table
        ref={(element) => {
          table = element;
        }}
        class="track-table"
        {...gridAttributes()}
        aria-label="Songs"
        aria-rowcount={count() + 1}
        onKeyDown={keyboard}
        onClick={(event) => {
          if (!interactive() || (event.target as Element).closest("input")) return;
          const index = rowAt(event.target);
          if (index === undefined) return;
          select(index, event.shiftKey, event.metaKey || event.ctrlKey);
          table?.focus({ preventScroll: true });
        }}
        onDblClick={(event) => {
          if ((event.target as Element).closest("input")) return;
          const index = rowAt(event.target);
          if (index !== undefined) props.onPlayIndex?.(index);
        }}
        onContextMenu={(event) => {
          if (openMenu(event.target as Element, event.clientX, event.clientY))
            event.preventDefault();
        }}
      >
        <colgroup>
          <For each={visible()}>
            {(column) => (
              <col
                class={`track-${column.width}-column`}
                style={{
                  width: preference(column.key)?.width
                    ? `${preference(column.key)?.width}px`
                    : undefined,
                }}
              />
            )}
          </For>
        </colgroup>
        <thead>
          <tr>
            <For each={visible()}>
              {(column) => (
                <th
                  scope="col"
                  class={column.numeric ? "track-numeric" : undefined}
                  aria-sort={
                    direction(column.key) ??
                    (!props.onSort && column.key === "title" ? "ascending" : undefined)
                  }
                  aria-label={column.key === "checked" ? column.label : undefined}
                >
                  <Show
                    when={props.onSort}
                    fallback={
                      column.key === "checked" ? (
                        <Icon name="check" />
                      ) : (
                        <span class="column-name">
                          {column.label}
                          <Show when={column.key === "title"}>
                            <Icon name="arrow" />
                          </Show>
                        </span>
                      )
                    }
                  >
                    <button
                      type="button"
                      class="column-name"
                      aria-label={column.key === "checked" ? column.label : undefined}
                      onClick={() =>
                        props.onSort?.({
                          column: column.key,
                          direction:
                            direction(column.key) === "ascending" ? "descending" : "ascending",
                        })
                      }
                    >
                      <Show when={column.key === "checked"} fallback={column.label}>
                        <Icon name="check" />
                      </Show>
                      <Show when={direction(column.key)}>
                        <Icon name="arrow" />
                      </Show>
                    </button>
                  </Show>
                  <Show when={props.onColumnResize}>
                    {(resize) => (
                      <ResizeHandle
                        axis="x"
                        label={`Resize ${column.label} column`}
                        onResize={(value) => resize()(column.key, value)}
                      />
                    )}
                  </Show>
                </th>
              )}
            </For>
          </tr>
        </thead>
        <tbody>
          <Show when={count() === 0}>
            <tr>
              <td class="empty-library" colspan={Math.max(1, visible().length)}>
                No songs in this view
              </td>
            </tr>
          </Show>
          <Show when={range().first > 0}>
            <tr inert>
              <td
                class="track-spacer"
                colspan={Math.max(1, visible().length)}
                style={{ height: `${range().first * rowHeight()}px` }}
              />
            </tr>
          </Show>
          <For each={extent()}>
            {(slot) => {
              const index = () => range().first + slot;
              const row = () => track(index());
              const isSelected = () =>
                interactive() ? selected().has(index()) : row()?.id === props.selectedId;
              return (
                <tr
                  id={`track-row-${index()}`}
                  data-track-index={index()}
                  aria-rowindex={index() + 2}
                  aria-selected={interactive() ? (isSelected() ? "true" : "false") : undefined}
                  class={{
                    selected: isSelected(),
                    "alternate-row": index() % 2 === 1,
                  }}
                >
                  <For each={visible()}>
                    {(column) => (
                      <TrackCell
                        row={row()}
                        column={column.key}
                        grid={interactive()}
                        selectedId={props.selectedId}
                        playingId={props.track?.id}
                        onSelect={props.onSelect}
                        onPlayTrack={props.onPlayTrack}
                        onTrackChecked={props.onTrackChecked}
                      />
                    )}
                  </For>
                </tr>
              );
            }}
          </For>
          <Show when={range().end < count()}>
            <tr inert>
              <td
                class="track-spacer"
                colspan={Math.max(1, visible().length)}
                style={{ height: `${(count() - range().end) * rowHeight()}px` }}
              />
            </tr>
          </Show>
        </tbody>
      </table>
      <Show when={menu()}>
        {(value) => (
          <TrackMenu
            label={value().kind === "columns" ? "Track columns" : "Track actions"}
            x={value().x}
            y={value().y}
            options={menuItems(value())}
            onClose={(restoreFocus) => {
              const source = value().source;
              setMenu(undefined);
              if (restoreFocus) source.focus({ preventScroll: true });
            }}
          />
        )}
      </Show>
      <Show when={props.onRemoveTracks}>
        <dialog
          class="remove-tracks"
          aria-labelledby="remove-tracks-heading"
          ref={(element) => {
            confirm = element;
          }}
          onClose={(event) => {
            const chosen = removal();
            if (chosen && event.currentTarget.returnValue === "remove") {
              props.onRemoveTracks?.(chosen.viewId, chosen.indexes, trash());
            }
            setRemoval(undefined);
            table?.focus({ preventScroll: true });
          }}
        >
          <h2 id="remove-tracks-heading">Remove from Library?</h2>
          <p>
            {removal()?.indexes.length === 1
              ? "Remove the selected song from your library."
              : `Remove ${removal()?.indexes.length.toLocaleString() ?? ""} selected songs from your library.`}
          </p>
          <label>
            <input
              type="checkbox"
              checked={trash()}
              onChange={(event) => setTrash(event.currentTarget.checked)}
            />
            Also move files to Trash
          </label>
          <form method="dialog">
            <button type="submit" value="cancel" autofocus>
              Cancel
            </button>
            <button type="submit" value="remove">
              Remove
            </button>
          </form>
        </dialog>
      </Show>
    </div>
  );
}
