import { createEffect, createSignal, onCleanup } from "solid-js";
import {
  commands,
  events,
  type TrackColumn,
  type TrackRow_Serialize,
  type TrackSort,
  type WindowSettings_Serialize,
} from "./bindings";
import { artUrl, type LibraryPages, openLibraryPages } from "./library/ipc";
import { type Facet, LibraryWindow, type SourceGroup, type Track } from "./library/LibraryWindow";

const sources: SourceGroup[] = [{ label: "Library", sources: [{ name: "Music", icon: "music" }] }];
const facets: Facet[] = ["Genre", "Artist", "Album"].map((label) => ({
  label,
  values: ["All"],
  active: "All",
}));

function displayTrack(row: TrackRow_Serialize): Track {
  return {
    id: row.id,
    name: row.title ?? "Untitled",
    seconds: Math.floor((row.duration_ms ?? 0) / 1000),
    artist: row.artist ?? "Unknown Artist",
    album: row.album ?? "Unknown Album",
    genre: row.genre ?? "",
    rating: Math.max(0, Math.min(5, Math.round(row.rating / 20))),
    plays: row.play_count,
    checked: row.checked,
  };
}

/** The desktop library window backed by the local Rust library. */
export function App() {
  const [settings, setSettings] = createSignal<WindowSettings_Serialize>();
  const [rows, setRows] = createSignal<TrackRow_Serialize[]>([]);
  const [selected, setSelected] = createSignal("");
  const [summary, setSummary] = createSignal("Opening library…");
  let view: LibraryPages | undefined;
  let stopLibraryEvents: (() => void) | undefined;
  let generation = 0;
  let disposed = false;
  let saves: Promise<void> = Promise.resolve();

  function reportError(message: string) {
    setSummary(message);
  }

  async function loadView(sort: TrackSort | undefined = settings()?.sort) {
    const request = ++generation;
    const result = await openLibraryPages({ sort });
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
    const page = await view.pageFor(0);
    if (request !== generation) return;
    if (page.status === "error") {
      reportError("Could not read the music library.");
      return;
    }
    setRows(page.data);
    const total = result.data.view.count.toLocaleString();
    setSummary(`${total} ${result.data.view.count === 1 ? "song" : "songs"}`);
  }

  createEffect(
    () => undefined,
    () => {
      const initialize = async () => {
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
        await loadView();
      };
      void initialize().catch(() => reportError("Could not open the music library."));
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
  });

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

  const selectedRow = () => rows().find((row) => row.id === selected());
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
      tracks={rows().map(displayTrack)}
      selectedId={selected()}
      sources={sources}
      activeSource="Music"
      facets={facets}
      playing={false}
      position={0}
      volume={65}
      artwork={selectedArtwork()}
      artworkTrack={artworkTrack()}
      showArtwork={settings()?.artwork_visible ?? true}
      showBrowser={settings()?.browser_visible ?? true}
      theme={settings()?.theme ?? "system"}
      showSystemTheme
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
      onSelect={(track) => setSelected(track.id)}
      onSort={(sort) => {
        updateSettings({ sort });
        void loadView(sort).catch(() => reportError("Could not sort the music library."));
      }}
      onToggleArtwork={() => updateSettings({ artwork_visible: !settings()?.artwork_visible })}
      onToggleBrowser={() => updateSettings({ browser_visible: !settings()?.browser_visible })}
      onTheme={(theme) => updateSettings({ theme })}
    />
  );
}
