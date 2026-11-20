import { Channel } from "@tauri-apps/api/core";
import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import {
  type BrowserFacets,
  type BrowserOption,
  commands,
  events,
  type ImportSettings,
  type PlayerAction,
  type PlayerEvent_Deserialize,
  type ScanUpdate,
  type TrackColumn,
  type TrackRow_Serialize,
  type TrackSort,
  type ViewSpec_Deserialize,
  type WindowSettings_Serialize,
} from "./bindings";
import { type FacetLabel, facetLabels } from "./library/ColumnBrowser";
import { artUrl, LIBRARY_PAGE_SIZE, type LibraryPages, openLibraryPages } from "./library/ipc";
import { LibraryWindow, type SourceGroup, type Track } from "./library/LibraryWindow";

const sources: SourceGroup[] = [{ label: "Library", sources: [{ name: "Music", icon: "music" }] }];
type BrowseSelection = Record<FacetLabel, BrowserOption | null>;
const emptyFacets: BrowserFacets = { genres: [], artists: [], albums: [] };

function displayTrack(row: TrackRow_Serialize): Track {
  return {
    id: row.id,
    name: row.title ?? "Untitled",
    seconds: Math.floor((row.duration_ms ?? 0) / 1000),
    artist: row.artist ?? "Unknown Artist",
    album: row.album ?? "Unknown Album",
    albumArtist: row.album_artist ?? undefined,
    genre: row.genre ?? "",
    year: row.year ?? undefined,
    trackNumber: row.track_no ?? undefined,
    dateAdded: row.date_added ?? undefined,
    lastPlayed: row.last_played_at ?? undefined,
    kind: row.codec
      ? `${row.codec.toUpperCase()}${row.bitrate ? ` · ${row.bitrate.toLocaleString()} kbps` : ""}`
      : undefined,
    size: row.size_bytes ?? undefined,
    rating: Math.max(0, Math.min(5, Math.round(row.rating / 20))),
    plays: row.play_count,
    checked: row.checked,
  };
}

/** The desktop library window backed by the local Rust library. */
export function App() {
  const [settings, setSettings] = createSignal<WindowSettings_Serialize>();
  const [browser, setBrowser] = createSignal(emptyFacets);
  const [selection, setSelection] = createSignal<BrowseSelection>({
    Genre: null,
    Artist: null,
    Album: null,
  });
  const [search, setSearch] = createSignal("");
  const [importSettings, setImportSettings] = createSignal<ImportSettings>();
  const [scan, setScan] = createSignal<ScanUpdate>();
  const [preferenceBusy, setPreferenceBusy] = createSignal(false);
  const [preferenceError, setPreferenceError] = createSignal("");
  const [visibleRows, setVisibleRows] = createSignal({
    offset: 0,
    rows: [] as TrackRow_Serialize[],
  });
  const [openedView, setOpenedView] = createSignal<LibraryPages["view"]>();
  const [trackScrollKey, setTrackScrollKey] = createSignal("");
  const [selectedRow, setSelectedRow] = createSignal<TrackRow_Serialize>();
  const [player, setPlayer] = createSignal<PlayerEvent_Deserialize>();
  const [playingTrack, setPlayingTrack] = createSignal<TrackRow_Serialize>();
  const [actionNotice, setActionNotice] = createSignal("");
  const [summary, setSummary] = createSignal("Opening library…");
  let view: LibraryPages | undefined;
  let stopLibraryEvents: (() => void) | undefined;
  let stopBrowserEvents: (() => void) | undefined;
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let committedSearch = "";
  let facetRequest = "";
  let currentRevision = 0;
  let generation = 0;
  let pageRequest = "";
  let disposed = false;
  let saves: Promise<void> = Promise.resolve();

  function reportError(message: string) {
    setSummary(message);
  }

  function currentSpec(
    sort: TrackSort | undefined = settings()?.sort,
    choices = selection(),
  ): ViewSpec_Deserialize {
    return {
      sort,
      ...(committedSearch && { query: committedSearch }),
      ...(choices.Genre && { genre: choices.Genre.name }),
      ...(choices.Artist?.compilation && { compilation: true }),
      ...(choices.Artist &&
        !choices.Artist.compilation && {
          artist: choices.Artist.name,
          compilation: false,
        }),
      ...(choices.Album && { album: choices.Album.name }),
    };
  }

  function applySearch(value: string) {
    const query = value.trim();
    if (query === committedSearch) return;
    committedSearch = query;
    void loadView().catch(() => reportError("Could not search the music library."));
  }

  function chooseFacet(label: FacetLabel, value: BrowserOption | null) {
    const choices = { ...selection(), [label]: value };
    if (label === "Genre") choices.Artist = null;
    if (label !== "Album") choices.Album = null;
    setSelection(choices);
    void loadView(currentSpec(settings()?.sort, choices)).catch(() =>
      reportError("Could not filter the music library."),
    );
  }

  function loadFacets(spec: ViewSpec_Deserialize) {
    const browse = { ...spec, sort: undefined, album: undefined };
    const key = `${currentRevision}:${JSON.stringify(browse)}`;
    if (key === facetRequest) return;
    facetRequest = key;
    void commands
      .browserFacets(browse)
      .then((result) => {
        if (disposed || key !== facetRequest) return;
        if (result.status === "ok") setBrowser(result.data);
        else {
          facetRequest = "";
          setActionNotice("Could not read the column browser.");
        }
      })
      .catch(() => {
        if (!disposed && key === facetRequest) {
          facetRequest = "";
          setActionNotice("Could not read the column browser.");
        }
      });
  }

  function toggleBrowser() {
    const current = settings();
    if (!current) return;
    updateSettings({ browser_visible: !current.browser_visible });
    if (!current.browser_visible) loadFacets(currentSpec());
  }

  async function loadVisible(first: number, end: number) {
    const current = view;
    if (!current || first >= end) return;
    const start = Math.floor(first / LIBRARY_PAGE_SIZE) * LIBRARY_PAGE_SIZE;
    const last = Math.floor((end - 1) / LIBRARY_PAGE_SIZE) * LIBRARY_PAGE_SIZE;
    const key = `${current.view.view_id}:${start}:${last}`;
    if (pageRequest === key) return;
    pageRequest = key;
    const pages = [];
    for (let offset = start; offset <= last; offset += LIBRARY_PAGE_SIZE) {
      pages.push(current.pageFor(offset));
    }
    try {
      const results = await Promise.all(pages);
      if (pageRequest !== key || disposed) return;
      const rows = [];
      for (const result of results) {
        if (result.status === "error") {
          pageRequest = "";
          if (result.error.kind !== "view_expired")
            reportError("Could not read the music library.");
          return;
        }
        rows.push(...result.data);
      }
      setVisibleRows({ offset: start, rows });
    } catch {
      if (pageRequest === key && !disposed) {
        pageRequest = "";
        reportError("Could not read the music library.");
      }
    }
  }

  async function trackAction(
    action: () => ReturnType<typeof commands.playView>,
    errorMessage: string,
  ) {
    setActionNotice("");
    try {
      if ((await action()).status === "error") setActionNotice(errorMessage);
    } catch {
      setActionNotice(errorMessage);
    }
  }

  function playIndex(index: number) {
    const current = view;
    if (current && index < current.view.count) {
      void trackAction(
        () => commands.playView(current.view.view_id, index),
        "Could not play this track.",
      );
    }
  }

  function togglePlaying(index = 0) {
    if (player()?.track_id) {
      void trackAction(commands.playerToggle, "Could not change playback.");
    } else {
      playIndex(index);
    }
  }

  function controlPlayer(action: PlayerAction) {
    void trackAction(() => commands.playerControl(action), "Could not change playback.");
  }

  async function removeTracks(viewId: string, indexes: number[], trash: boolean) {
    setActionNotice("");
    try {
      const result = await commands.tracksRemove(viewId, indexes, trash);
      if (result.status === "error") {
        setActionNotice("Could not remove the selected tracks.");
      } else {
        setSelectedRow(undefined);
        if (result.data.trash_errors > 0) {
          const count = result.data.trash_errors;
          setActionNotice(
            `${count.toLocaleString()} ${count === 1 ? "file" : "files"} could not be moved to Trash.`,
          );
        }
      }
    } catch {
      setActionNotice("Could not remove the selected tracks.");
    }
  }

  async function updateImportSettings(
    action: () => ReturnType<typeof commands.importAddWatch>,
    errorMessage: string,
  ) {
    setPreferenceBusy(true);
    setPreferenceError("");
    try {
      const result = await action();
      if (result.status === "ok") {
        if (result.data) setImportSettings(result.data);
      } else setPreferenceError(errorMessage);
    } catch {
      setPreferenceError(errorMessage);
    } finally {
      setPreferenceBusy(false);
    }
  }

  const scanProgress = () => {
    const update = scan();
    return update?.state === "scanning" ? update.progress : undefined;
  };
  const scanNotice = () => {
    const update = scan();
    if (update?.state === "failed") return "Could not import music.";
    if (update?.state !== "finished") return "";
    const messages = [];
    if (update.duplicates === 1) messages.push("Duplicate found");
    if (update.duplicates > 1)
      messages.push(`Duplicates found (${update.duplicates.toLocaleString()})`);
    if (update.errors > 0) {
      messages.push(
        `${update.errors.toLocaleString()} ${update.errors === 1 ? "item" : "items"} could not be imported`,
      );
    }
    return messages.join(" · ");
  };

  async function loadView(spec = currentSpec()) {
    const request = ++generation;
    const result = await openLibraryPages(spec);
    if (result.status === "error") {
      if (request === generation) reportError("Could not open the music library.");
      return;
    }
    if (request !== generation) {
      result.data.close();
      return;
    }
    view?.close();
    view = result.data;
    currentRevision = result.data.view.revision;
    pageRequest = "";
    setVisibleRows({ offset: 0, rows: [] });
    setTrackScrollKey(JSON.stringify(spec));
    setOpenedView(result.data.view);
    const total = result.data.view.count.toLocaleString();
    setSummary(`${total} ${result.data.view.count === 1 ? "song" : "songs"}`);
    if (settings()?.browser_visible ?? true) loadFacets(spec);
  }

  createEffect(
    () => undefined,
    () => {
      const initialize = async () => {
        const transport = new Channel<PlayerEvent_Deserialize>();
        transport.onmessage = (update) => {
          if (disposed) return;
          setPlayer(update);
          if (update.error) setActionNotice("Could not play the track.");
        };
        void commands.subscribePlayer(transport).catch(() => {
          if (!disposed) setActionNotice("Could not read playback state.");
        });
        const updates = new Channel<ScanUpdate>();
        updates.onmessage = (update) => {
          if (!disposed) setScan(update);
        };
        void commands
          .subscribeScan(updates)
          .then((result) => {
            if (!disposed && result.status === "error") setScan({ state: "failed" });
          })
          .catch(() => {
            if (!disposed) setScan({ state: "failed" });
          });
        void updateImportSettings(
          commands.importSettingsGet,
          "Could not read library preferences.",
        );
        const loaded = await commands.windowSettingsGet();
        if (disposed) return;
        if (loaded.status === "error") {
          reportError("Could not read window settings.");
          return;
        }
        setSettings(loaded.data);
        const stop = await events.libraryChanged.listen(() => {
          void loadView().catch(() => reportError("Could not refresh the music library."));
        });
        if (disposed) {
          stop();
          return;
        }
        stopLibraryEvents = stop;
        const stopBrowse = await events.toggleColumnBrowser.listen(toggleBrowser);
        if (disposed) {
          stopBrowse();
          return;
        }
        stopBrowserEvents = stopBrowse;
        await loadView();
      };
      void initialize().catch(() => reportError("Could not open the music library."));
    },
  );

  createEffect(
    () => player()?.track_id,
    (id) => {
      if (!id) return;
      void commands
        .trackGet(id)
        .then((result) => {
          if (!disposed && player()?.track_id === id && result.status === "ok") {
            setPlayingTrack(result.data);
          }
        })
        .catch(() => {
          if (!disposed) setActionNotice("Could not read the playing track.");
        });
    },
  );

  createEffect(
    () => settings()?.theme,
    (theme) => {
      if (theme && theme !== "system") document.documentElement.dataset.theme = theme;
      else delete document.documentElement.dataset.theme;
    },
  );

  onCleanup(() => {
    disposed = true;
    generation += 1;
    view?.close();
    stopLibraryEvents?.();
    stopBrowserEvents?.();
    clearTimeout(searchTimer);
    document.removeEventListener("keydown", searchShortcut);
  });

  function searchShortcut(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      document.getElementById("library-search")?.focus();
    }
  }
  document.addEventListener("keydown", searchShortcut);

  const updateSettings = (patch: Partial<WindowSettings_Serialize>) => {
    const current = settings();
    if (!current) return;
    const next = { ...current, ...patch };
    setSettings(next);
    saves = saves
      .then(async () => {
        if ((await commands.windowSettingsSave(next)).status === "error") {
          reportError("Could not save window settings.");
        }
      })
      .catch(() => reportError("Could not save window settings."));
  };

  const updateColumn = (column: TrackColumn, patch: { width?: number; visible?: boolean }) => {
    const columns = settings()?.columns ?? [];
    const old = columns.find((item) => item.column === column);
    updateSettings({
      columns: [
        ...columns.filter((item) => item.column !== column),
        {
          column,
          visible: patch.visible ?? old?.visible ?? true,
          width: patch.width ?? old?.width ?? null,
        },
      ],
    });
  };

  const displayRows = createMemo(() => visibleRows().rows.map(displayTrack));
  const nowPlaying = () => {
    const row = playingTrack();
    return row && row.id === player()?.track_id ? displayTrack(row) : undefined;
  };
  const selectedArtwork = () => {
    const id = selectedRow()?.artwork_id;
    return id ? artUrl(id, 300) : undefined;
  };
  const artworkTrack = () => {
    const row = selectedRow();
    return row ? displayTrack(row) : undefined;
  };

  return (
    <LibraryWindow
      tracks={[]}
      trackCount={openedView()?.count ?? 0}
      trackViewId={openedView()?.view_id}
      trackScrollKey={trackScrollKey()}
      trackAt={(index) => displayRows()[index - visibleRows().offset]}
      onTrackWindow={(first, end) => void loadVisible(first, end)}
      onPlayIndex={playIndex}
      onToggleTrackPlayback={togglePlaying}
      onRevealTrack={(track) =>
        void trackAction(
          () => commands.trackReveal(track.id),
          "Could not show this track in File Manager.",
        )
      }
      onTrackChecked={(track, checked) =>
        void trackAction(
          () => commands.trackSetChecked(track.id, checked),
          "Could not update sync selection.",
        )
      }
      onRemoveTracks={(...selection) => void removeTracks(...selection)}
      selectedId={selectedRow()?.id ?? ""}
      sources={sources}
      activeSource="Music"
      facets={facetLabels.map((label, index) => ({
        label,
        values: [browser().genres, browser().artists, browser().albums][index] ?? [],
        active: selection()[label],
      }))}
      onFacet={chooseFacet}
      searchQuery={search()}
      onSearch={(query) => {
        setSearch(query);
        clearTimeout(searchTimer);
        searchTimer = setTimeout(() => applySearch(query), 80);
      }}
      onClearSearch={() => {
        setSearch("");
        clearTimeout(searchTimer);
        applySearch("");
      }}
      track={nowPlaying()}
      playing={player()?.state === "playing"}
      position={Math.floor((player()?.position_ms ?? 0) / 1_000)}
      onTogglePlaying={() => togglePlaying()}
      onPrevious={player()?.track_id ? () => controlPlayer({ kind: "previous" }) : undefined}
      onNext={player()?.track_id ? () => controlPlayer({ kind: "next" }) : undefined}
      onPosition={(seconds) => controlPlayer({ kind: "seek", position_ms: seconds * 1_000 })}
      volume={player()?.volume_percent ?? 65}
      onVolume={(percent) => controlPlayer({ kind: "volume", percent })}
      artwork={selectedArtwork()}
      artworkTrack={artworkTrack()}
      showArtwork={settings()?.artwork_visible ?? true}
      showBrowser={settings()?.browser_visible ?? true}
      theme={settings()?.theme ?? "system"}
      showSystemTheme
      scanProgress={scanProgress()}
      importNotice={[scanNotice(), actionNotice()].filter(Boolean).join(" · ")}
      importSettings={importSettings()}
      preferenceBusy={preferenceBusy()}
      preferenceError={preferenceError()}
      onAddFolder={() => {
        void commands
          .importChooseFolder()
          .then((result) => {
            if (result.status === "error") setScan({ state: "failed" });
          })
          .catch(() => setScan({ state: "failed" }));
      }}
      onCopyFiles={(copyFiles) =>
        void updateImportSettings(
          () => commands.importSetCopy(copyFiles),
          "Could not save the copy preference.",
        )
      }
      onWatchFolder={() =>
        void updateImportSettings(commands.importAddWatch, "Could not watch this folder.")
      }
      onRemoveWatch={(path) =>
        void updateImportSettings(
          () => commands.importRemoveWatch(path),
          "Could not remove this watched folder.",
        )
      }
      sort={settings()?.sort}
      columnPreferences={settings()?.columns}
      sidebarWidth={settings()?.sidebar_width}
      browserHeight={settings()?.browser_height}
      onSidebarResize={(width) =>
        updateSettings({ sidebar_width: Math.max(150, Math.min(400, width)) })
      }
      onBrowserResize={(height) =>
        updateSettings({ browser_height: Math.max(96, Math.min(440, height)) })
      }
      onColumnResize={(column, width) =>
        updateColumn(column, { width: Math.max(36, Math.min(800, width)) })
      }
      onColumnVisibility={(column, visible) => updateColumn(column, { visible })}
      summary={summary()}
      onSelect={(track) => setSelectedRow(visibleRows().rows.find((row) => row.id === track.id))}
      onSort={(sort) => {
        updateSettings({ sort });
        void loadView(currentSpec(sort)).catch(() =>
          reportError("Could not sort the music library."),
        );
      }}
      onToggleArtwork={() => updateSettings({ artwork_visible: !settings()?.artwork_visible })}
      onToggleBrowser={toggleBrowser}
      onTheme={(theme) => updateSettings({ theme })}
    />
  );
}
