import { For, Show } from "solid-js";
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
  track: Track;
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
  theme: Theme;
  summary: string;
  onSource: (source: string) => void;
  onFacet: (label: string, value: string) => void;
  onSelect: (track: Track) => void;
  onPlayTrack: (track: Track) => void;
  onTogglePlaying: () => void;
  onPrevious: () => void;
  onNext: () => void;
  onPosition: (seconds: number) => void;
  onVolume: (level: number) => void;
  onSearch?: (query: string) => void;
  onToggleArtwork: () => void;
  onToggleBrowser: () => void;
  onTheme: (theme: Theme) => void;
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
          onClick={props.onPrevious}
        >
          <Icon name="previous" solid />
        </button>
        <button
          type="button"
          class="control play-control"
          aria-label={props.playing ? "Pause" : "Play"}
          onClick={props.onTogglePlaying}
        >
          <Icon name={props.playing ? "pause" : "play"} solid />
        </button>
        <button type="button" class="control" aria-label="Next track" onClick={props.onNext}>
          <Icon name="next" solid />
        </button>
      </div>
      <label class="volume">
        <Icon name="speaker" />
        <input
          type="range"
          aria-label="Volume"
          value={props.volume}
          style={{ "--range-progress": `${props.volume}%` }}
          onInput={(event) => props.onVolume(event.currentTarget.valueAsNumber)}
        />
      </label>
    </section>
  );
}

function PlaybackDisplay(props: WindowProps) {
  return (
    <section class="playback-display" aria-label="Now playing">
      <div class="display-badge" aria-hidden="true">
        <Icon name="wave" />
      </div>
      <div class="display-body">
        <div class="display-track">
          <strong title={props.track.name}>{props.track.name}</strong>
          <span title={`${props.track.artist} · ${props.track.album}`}>
            {props.track.artist} · {props.track.album}
          </span>
        </div>
        <div class="display-progress">
          <span>{formatTrackTime(props.position)}</span>
          <input
            type="range"
            min={0}
            max={props.track.seconds}
            value={props.position}
            style={{ "--range-progress": `${(100 * props.position) / props.track.seconds}%` }}
            aria-label="Playback position"
            onInput={(event) => props.onPosition(event.currentTarget.valueAsNumber)}
          />
          <span>{formatTrackTime(props.track.seconds)}</span>
        </div>
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
                        onClick={() => props.onSource(source.name)}
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
              <strong>{props.track.album}</strong>
              <span>{props.track.artist}</span>
            </figcaption>
          </figure>
        )}
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
                      onClick={() => props.onFacet(facet.label, value)}
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
    </section>
  );
}

function TrackListing(props: WindowProps) {
  return (
    <div class="track-scroll">
      <table class="track-table" aria-label="Songs">
        <colgroup>
          <col class="track-checkbox-column" />
          <col class="track-name-column" />
          <col class="track-time-column" />
          <col class="track-artist-column" />
          <col class="track-album-column" />
          <col class="track-genre-column" />
          <col class="track-rating-column" />
          <col class="track-plays-column" />
        </colgroup>
        <thead>
          <tr>
            <th scope="col" aria-label="Include in sync">
              <Icon name="check" />
            </th>
            <th scope="col" aria-sort="ascending">
              <span class="column-name">
                Name <Icon name="arrow" />
              </span>
            </th>
            <th scope="col" class="track-numeric">
              Time
            </th>
            <th scope="col">Artist</th>
            <th scope="col">Album</th>
            <th scope="col">Genre</th>
            <th scope="col">Rating</th>
            <th scope="col" class="track-numeric">
              Plays
            </th>
          </tr>
        </thead>
        <tbody>
          <For each={props.tracks}>
            {(track) => (
              <tr class={track.id === props.selectedId ? "selected" : undefined}>
                <td class="include-track">
                  <input
                    type="checkbox"
                    checked={track.checked}
                    aria-label={`Sync ${track.name}`}
                  />
                </td>
                <td>
                  <button
                    type="button"
                    class="track-name"
                    aria-current={track.id === props.selectedId ? "true" : undefined}
                    onClick={() => props.onSelect(track)}
                    onDblClick={() => props.onPlayTrack(track)}
                  >
                    <span class="track-speaker">
                      <Show when={track.id === props.track.id && props.playing}>
                        <Icon name="speaker" />
                      </Show>
                    </span>
                    <span>{track.name}</span>
                  </button>
                </td>
                <td class="track-numeric">{formatTrackTime(track.seconds)}</td>
                <td title={track.artist}>{track.artist}</td>
                <td title={track.album}>{track.album}</td>
                <td title={track.genre}>{track.genre}</td>
                <td>
                  <span class="rating" role="img" aria-label={`${track.rating} out of 5 stars`}>
                    <span aria-hidden="true">{"★".repeat(track.rating)}</span>
                    <span class="rating-empty" aria-hidden="true">
                      {"★".repeat(5 - track.rating)}
                    </span>
                  </span>
                </td>
                <td class="track-numeric">{track.plays}</td>
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
        <button type="button" class="control" aria-label="Add folder" disabled>
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
      </span>
      <details class="appearance">
        <summary class="control" aria-label="Appearance" title="Appearance">
          <Icon name="settings" />
        </summary>
        <section class="appearance-options" aria-label="Theme">
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
      </details>
    </footer>
  );
}

/** Desktop layout shared by the fixture preview and the connected library. */
export function LibraryWindow(props: WindowProps) {
  return (
    <div class="library-window">
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
