import { For, Show } from "solid-js";
import type {
  ColumnPreference_Serialize,
  ImportSettings,
  ScanProgress,
  TrackColumn,
  TrackSort,
  WindowTheme,
} from "../bindings";
import { Icon, type IconName } from "./Icon";
import "./library.css";

export interface Track {
  id: string;
  name: string;
  seconds: number;
  artist: string;
  album: string;
  genre: string;
  rating: number;
  plays: number;
  checked: boolean;
}

export interface Facet {
  label: string;
  values: readonly string[];
  active: string;
}

export interface SourceGroup {
  label: string;
  sources: readonly { name: string; icon: IconName; eject?: boolean }[];
}

export type Theme = "light" | "dark";

interface WindowProps {
  tracks: readonly Track[];
  track?: Track;
  artworkTrack?: Track;
  selectedId: string;
  sources: readonly SourceGroup[];
  activeSource: string;
  facets: readonly Facet[];
  playing: boolean;
  position: number;
  volume: number;
  artwork?: string;
  showArtwork: boolean;
  showBrowser: boolean;
  theme: WindowTheme;
  showSystemTheme?: boolean;
  scanProgress?: ScanProgress;
  importNotice?: string;
  importSettings?: ImportSettings;
  preferenceBusy?: boolean;
  preferenceError?: string;
  onAddFolder?: () => void;
  onCopyFiles?: (enabled: boolean) => void;
  onWatchFolder?: () => void;
  onRemoveWatch?: (path: string) => void;
  sort?: TrackSort;
  columnPreferences?: ColumnPreference_Serialize[];
  sidebarWidth?: number | null;
  browserHeight?: number | null;
  onSidebarResize?: (width: number) => void;
  onBrowserResize?: (height: number) => void;
  onColumnResize?: (column: TrackColumn, width: number) => void;
  onColumnVisibility?: (column: TrackColumn, visible: boolean) => void;
  onSort?: (sort: TrackSort) => void;
  summary: string;
  onSource?: (source: string) => void;
  onFacet?: (label: string, value: string) => void;
  onSelect: (track: Track) => void;
  onPlayTrack?: (track: Track) => void;
  onTogglePlaying?: () => void;
  onPrevious?: () => void;
  onNext?: () => void;
  onPosition?: (seconds: number) => void;
  onVolume?: (level: number) => void;
  onSearch?: (query: string) => void;
  onToggleArtwork: () => void;
  onToggleBrowser: () => void;
  onTheme: (theme: WindowTheme) => void;
}

export function formatTrackTime(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function Transport(props: WindowProps) {
  return (
    <section class="transport" aria-label="Playback controls">
      <div class="transport-buttons">
        <button
          type="button"
          class="control"
          aria-label="Previous track"
          disabled={!props.onPrevious}
          onClick={props.onPrevious}
        >
          <Icon name="previous" solid />
        </button>
        <button
          type="button"
          class="control play-control"
          aria-label={props.playing ? "Pause" : "Play"}
          disabled={!props.onTogglePlaying}
          onClick={props.onTogglePlaying}
        >
          <Icon name={props.playing ? "pause" : "play"} solid />
        </button>
        <button
          type="button"
          class="control"
          aria-label="Next track"
          disabled={!props.onNext}
          onClick={props.onNext}
        >
          <Icon name="next" solid />
        </button>
      </div>
      <label class="volume">
        <Icon name="speaker" />
        <input
          type="range"
          aria-label="Volume"
          disabled={!props.onVolume}
          value={props.volume}
          style={{ "--range-progress": `${props.volume}%` }}
          onInput={(event) => props.onVolume?.(event.currentTarget.valueAsNumber)}
        />
      </label>
    </section>
  );
}

function PlaybackDisplay(props: WindowProps) {
  return (
    <section class="playback-display" aria-label={props.scanProgress ? "Import" : "Now playing"}>
      <div class="display-badge" aria-hidden="true">
        <Icon name="wave" />
      </div>
      <div class="display-body">
        <Show
          when={props.scanProgress}
          fallback={
            <>
              <div class="display-track">
                <strong title={props.track?.name}>{props.track?.name ?? "No track playing"}</strong>
                <Show when={props.track}>
                  {(track) => (
                    <span title={`${track().artist} · ${track().album}`}>
                      {track().artist} · {track().album}
                    </span>
                  )}
                </Show>
              </div>
              <div class="display-progress">
                <span>{formatTrackTime(props.position)}</span>
                <input
                  type="range"
                  min={0}
                  max={props.track?.seconds || 1}
                  disabled={!props.onPosition || !props.track}
                  value={props.position}
                  style={{
                    "--range-progress": `${(100 * props.position) / (props.track?.seconds || 1)}%`,
                  }}
                  aria-label="Playback position"
                  onInput={(event) => props.onPosition?.(event.currentTarget.valueAsNumber)}
                />
                <span>{formatTrackTime(props.track?.seconds ?? 0)}</span>
              </div>
            </>
          }
        >
          {(progress) => (
            <>
              <div class="display-track" role="status">
                <strong>
                  {progress().total_estimate > 0
                    ? `Importing ${progress().scanned.toLocaleString()} of ${progress().total_estimate.toLocaleString()}`
                    : "Importing…"}
                </strong>
              </div>
              <progress
                class="import-progress"
                aria-label="Import progress"
                max={Math.max(1, progress().total_estimate)}
                value={progress().total_estimate > 0 ? progress().scanned : undefined}
              />
            </>
          )}
        </Show>
      </div>
    </section>
  );
}

function Toolbar(props: WindowProps) {
  return (
    <header class="library-toolbar">
      <Transport {...props} />
      <PlaybackDisplay {...props} />
      <div class="library-tools">
        <label class="library-search">
          <Icon name="search" />
          <input
            type="search"
            aria-label="Search library"
            placeholder="Search library"
            onInput={(event) => props.onSearch?.(event.currentTarget.value)}
          />
        </label>
        <button
          type="button"
          class="control"
          aria-pressed={props.showBrowser ? "true" : "false"}
          onClick={props.onToggleBrowser}
        >
          <Icon name="browse" />
          Browse
        </button>
      </div>
    </header>
  );
}

function ResizeHandle(props: { axis: "x" | "y"; label: string; onResize: (size: number) => void }) {
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

function SourceList(props: WindowProps) {
  return (
    <aside class="library-sidebar" aria-label="Library sidebar">
      <div class="library-name">
        <span class="library-mark">
          <Icon name="music" />
        </span>
        MyTunes
      </div>
      <nav class="source-navigation" aria-label="Sources">
        <For each={props.sources}>
          {(group) => (
            <section class="source-group" aria-label={group.label}>
              <h2>{group.label}</h2>
              <ul>
                <For each={group.sources}>
                  {(source) => (
                    <li class="source-item">
                      <button
                        type="button"
                        class="source-button"
                        aria-current={props.activeSource === source.name ? "true" : undefined}
                        onClick={() => props.onSource?.(source.name)}
                      >
                        <Icon name={source.icon} /> <span>{source.name}</span>
                      </button>
                      <Show when={source.eject}>
                        <button
                          type="button"
                          class="eject-button"
                          aria-label="Eject device"
                          disabled
                        >
                          <Icon name="eject" />
                        </button>
                      </Show>
                    </li>
                  )}
                </For>
              </ul>
            </section>
          )}
        </For>
      </nav>
      <Show when={props.showArtwork && props.artwork}>
        {(artwork) => (
          <figure class="album-art">
            <img src={artwork()} alt="" />
            <figcaption>
              <strong>{(props.artworkTrack ?? props.track)?.album}</strong>
              <span>{(props.artworkTrack ?? props.track)?.artist}</span>
            </figcaption>
          </figure>
        )}
      </Show>
      <Show when={props.onSidebarResize}>
        {(resize) => <ResizeHandle axis="x" label="Resize sidebar" onResize={resize()} />}
      </Show>
    </aside>
  );
}

function ColumnBrowser(props: WindowProps) {
  return (
    <section class="column-browser" aria-label="Column browser">
      <For each={props.facets}>
        {(facet) => (
          <section class="facet" aria-label={facet.label}>
            <h2>{facet.label}</h2>
            <ul>
              <For each={facet.values}>
                {(value) => (
                  <li>
                    <button
                      type="button"
                      aria-pressed={facet.active === value ? "true" : "false"}
                      onClick={() => props.onFacet?.(facet.label, value)}
                    >
                      {value}
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </section>
        )}
      </For>
      <Show when={props.onBrowserResize}>
        {(resize) => <ResizeHandle axis="y" label="Resize column browser" onResize={resize()} />}
      </Show>
    </section>
  );
}

const columns = [
  { key: "checked", label: "Include in sync", width: "checkbox", numeric: false },
  { key: "title", label: "Name", width: "name", numeric: false },
  { key: "duration", label: "Time", width: "time", numeric: true },
  { key: "artist", label: "Artist", width: "artist", numeric: false },
  { key: "album", label: "Album", width: "album", numeric: false },
  { key: "genre", label: "Genre", width: "genre", numeric: false },
  { key: "rating", label: "Rating", width: "rating", numeric: false },
  { key: "play_count", label: "Plays", width: "plays", numeric: true },
] as const;

type VisibleColumn = (typeof columns)[number]["key"];

function TrackCell(props: WindowProps & { row: Track; column: VisibleColumn }) {
  const row = () => props.row;
  switch (props.column) {
    case "checked":
      return (
        <td class="include-track">
          <input type="checkbox" checked={row().checked} aria-label={`Sync ${row().name}`} />
        </td>
      );
    case "title":
      return (
        <td>
          <button
            type="button"
            class="track-name"
            aria-current={row().id === props.selectedId ? "true" : undefined}
            onClick={() => props.onSelect(row())}
            onDblClick={() => props.onPlayTrack?.(row())}
          >
            <span class="track-speaker">
              <Show when={row().id === props.track?.id && props.playing}>
                <Icon name="speaker" />
              </Show>
            </span>
            <span>{row().name}</span>
          </button>
        </td>
      );
    case "duration":
      return <td class="track-numeric">{formatTrackTime(row().seconds)}</td>;
    case "artist":
    case "album":
    case "genre": {
      const field = props.column;
      return <td title={row()[field]}>{row()[field]}</td>;
    }
    case "rating":
      return (
        <td>
          <span class="rating" role="img" aria-label={`${row().rating} out of 5 stars`}>
            <span aria-hidden="true">{"★".repeat(row().rating)}</span>
            <span class="rating-empty" aria-hidden="true">
              {"★".repeat(5 - row().rating)}
            </span>
          </span>
        </td>
      );
    case "play_count":
      return <td class="track-numeric">{row().plays}</td>;
  }
}

function TrackListing(props: WindowProps) {
  const preference = (column: TrackColumn) =>
    props.columnPreferences?.find((item) => item.column === column);
  const visible = () => columns.filter((column) => preference(column.key)?.visible !== false);
  const direction = (column: TrackColumn) =>
    props.sort?.column === column ? props.sort.direction : undefined;
  return (
    <div class="track-scroll">
      <table class="track-table" aria-label="Songs">
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
                    when={props.onSort && column.key !== "checked"}
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
                      onClick={() =>
                        props.onSort?.({
                          column: column.key,
                          direction:
                            direction(column.key) === "ascending" ? "descending" : "ascending",
                        })
                      }
                    >
                      {column.label}
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
          <Show when={props.tracks.length === 0}>
            <tr>
              <td class="empty-library" colspan={Math.max(1, visible().length)}>
                No songs in this view
              </td>
            </tr>
          </Show>
          <For each={props.tracks}>
            {(track) => (
              <tr class={track.id === props.selectedId ? "selected" : undefined}>
                <For each={visible()}>
                  {(column) => <TrackCell {...props} row={track} column={column.key} />}
                </For>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </div>
  );
}

function StatusBar(props: WindowProps) {
  return (
    <footer class="library-status">
      <section class="library-actions" aria-label="Library controls">
        <button
          type="button"
          class="control"
          aria-label="Add folder"
          onClick={props.onAddFolder}
          disabled={!props.onAddFolder}
        >
          <Icon name="add" />
        </button>
        <button type="button" class="control" aria-label="Sync" disabled>
          <Icon name="sync" />
        </button>
        <button type="button" class="control" aria-label="Rescan library" disabled>
          <Icon name="refresh" />
        </button>
        <button
          type="button"
          class="control"
          aria-label="Show album artwork"
          aria-pressed={props.showArtwork ? "true" : "false"}
          onClick={props.onToggleArtwork}
        >
          <Icon name="art" />
        </button>
      </section>
      <span class="library-summary" role="status">
        {props.summary}
        <Show when={props.importNotice}>{(notice) => <> · {notice()}</>}</Show>
      </span>
      <details class="appearance">
        <summary
          class="control"
          aria-label={props.onWatchFolder ? "Preferences" : "Appearance"}
          title={props.onWatchFolder ? "Preferences" : "Appearance"}
        >
          <Icon name="settings" />
        </summary>
        <div class="appearance-options">
          <section class="theme-options" aria-label="Theme">
            <Show when={props.showSystemTheme}>
              <button
                type="button"
                aria-pressed={props.theme === "system" ? "true" : "false"}
                onClick={() => props.onTheme("system")}
              >
                System
              </button>
            </Show>
            <button
              type="button"
              aria-pressed={props.theme === "light" ? "true" : "false"}
              onClick={() => props.onTheme("light")}
            >
              Light
            </button>
            <button
              type="button"
              aria-pressed={props.theme === "dark" ? "true" : "false"}
              onClick={() => props.onTheme("dark")}
            >
              Dark
            </button>
          </section>
          <Show when={props.onColumnVisibility}>
            {(visible) => (
              <fieldset class="column-options">
                <legend>Columns</legend>
                <For each={columns}>
                  {(column) => (
                    <label>
                      <input
                        type="checkbox"
                        checked={
                          props.columnPreferences?.find((item) => item.column === column.key)
                            ?.visible !== false
                        }
                        onChange={(event) => visible()(column.key, event.currentTarget.checked)}
                      />
                      {column.label}
                    </label>
                  )}
                </For>
              </fieldset>
            )}
          </Show>
          <Show when={props.onWatchFolder}>
            <fieldset class="import-options" disabled={props.preferenceBusy}>
              <legend>Library</legend>
              <label class="copy-files">
                <input
                  type="checkbox"
                  checked={props.importSettings?.copy_files ?? false}
                  disabled={!props.importSettings}
                  onChange={(event) => {
                    const enabled = event.currentTarget.checked;
                    event.currentTarget.checked = props.importSettings?.copy_files ?? false;
                    props.onCopyFiles?.(enabled);
                  }}
                />
                Copy files to MyTunes Media folder when adding
              </label>
              <span class="watched-folders-label">Watched folders</span>
              <ul class="watched-folders" aria-label="Watched folders">
                <For each={props.importSettings?.watched_folders ?? []}>
                  {(path) => (
                    <li>
                      <span title={path}>{path}</span>
                      <button
                        type="button"
                        aria-label={`Remove watched folder ${path}`}
                        onClick={() => props.onRemoveWatch?.(path)}
                      >
                        Remove
                      </button>
                    </li>
                  )}
                </For>
              </ul>
              <button type="button" class="add-watch" onClick={props.onWatchFolder}>
                Add Watched Folder…
              </button>
              <Show when={props.preferenceError}>{(error) => <p role="alert">{error()}</p>}</Show>
            </fieldset>
          </Show>
        </div>
      </details>
    </footer>
  );
}

/** Desktop layout shared by the fixture preview and the connected library. */
export function LibraryWindow(props: WindowProps) {
  return (
    <div
      class="library-window"
      style={{
        "--window-sidebar": props.sidebarWidth ? `${props.sidebarWidth}px` : undefined,
        "--window-browser": props.browserHeight ? `${props.browserHeight}px` : undefined,
      }}
    >
      <Toolbar {...props} />
      <div class="library-body">
        <SourceList {...props} />
        <main class="library-content" aria-label="Music library">
          <Show when={props.showBrowser}>
            <ColumnBrowser {...props} />
          </Show>
          <TrackListing {...props} />
        </main>
      </div>
      <StatusBar {...props} />
    </div>
  );
}
