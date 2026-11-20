import { For, Show } from "solid-js";
import type { BrowserOption, ImportSettings, ScanProgress, WindowTheme } from "../bindings";
import { ColumnBrowser, type Facet, type FacetLabel } from "./ColumnBrowser";
import { formatTrackTime, ResizeHandle } from "./controls";
import { Icon, type IconName } from "./Icon";
import {
  defaultColumnVisible,
  type Track,
  type TrackTableProps,
  trackColumns,
  VirtualTable,
} from "./VirtualTable";
import "./library.css";

export type { Track } from "./VirtualTable";

export interface SourceGroup {
  label: string;
  sources: readonly { name: string; icon: IconName; eject?: boolean }[];
}

export type Theme = "light" | "dark";

interface WindowProps extends TrackTableProps {
  artworkTrack?: Track;
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
  sidebarWidth?: number | null;
  browserHeight?: number | null;
  onSidebarResize?: (width: number) => void;
  onBrowserResize?: (height: number) => void;
  summary: string;
  onSource?: (source: string) => void;
  onFacet?: (label: FacetLabel, value: BrowserOption | null) => void;
  onTogglePlaying?: () => void;
  onPrevious?: () => void;
  onNext?: () => void;
  onPosition?: (seconds: number) => void;
  onVolume?: (level: number) => void;
  searchQuery?: string;
  onSearch?: (query: string) => void;
  onClearSearch?: () => void;
  onToggleArtwork: () => void;
  onToggleBrowser: () => void;
  onTheme: (theme: WindowTheme) => void;
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
            id="library-search"
            value={props.searchQuery}
            aria-label="Search library"
            placeholder="Search library"
            onInput={(event) => props.onSearch?.(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape" && props.onClearSearch) {
                props.onClearSearch();
                event.preventDefault();
              }
            }}
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
                <For each={trackColumns}>
                  {(column) => (
                    <label>
                      <input
                        type="checkbox"
                        checked={
                          props.columnPreferences?.find((item) => item.column === column.key)
                            ?.visible ?? defaultColumnVisible(column)
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
            <ColumnBrowser
              facets={props.facets}
              onSelect={props.onFacet}
              onResize={props.onBrowserResize}
            />
          </Show>
          <VirtualTable {...props} />
        </main>
      </div>
      <StatusBar {...props} />
    </div>
  );
}
