import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";
import type { ScanUpdate, ViewSpec_Deserialize, WindowSettings_Serialize } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
  delete document.documentElement.dataset.theme;
});

test("the LCD shows import progress and the library keeps the duplicate notice", async () => {
  let updates: Channel<ScanUpdate> | undefined;
  mockIPC(
    (command, args) => {
      if (command === "subscribe_scan") {
        updates = (args as { channel: Channel<ScanUpdate> }).channel;
        return null;
      }
      if (command === "import_settings_get") return { copy_files: false, watched_folders: [] };
      if (command === "subscribe_player") return null;
      if (command === "browser_facets") return { genres: [], artists: [], albums: [] };
      if (command === "window_settings_get") return {};
      if (command === "view_open") return { view_id: "1", count: 0, revision: 0 };
      if (command === "view_rows") return [];
      throw new Error(`unexpected command ${command}`);
    },
    { shouldMockEvents: true },
  );
  render(() => <App />);
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("0 songs"));
  expect(updates).toBeDefined();
  updates?.onmessage({
    state: "scanning",
    progress: { scanned: 312, total_estimate: 1204, current_path: "/music/Track.wav" },
  });
  const progress = await screen.findByRole("progressbar", { name: "Import progress" });
  expect(progress.getAttribute("value")).toBe("312");
  expect(progress.getAttribute("max")).toBe("1204");
  updates?.onmessage({ state: "finished", duplicates: 1, errors: 0 });
  await waitFor(() => {
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("Duplicate found");
  });
});

test("browser selections compose with debounced search and the search shortcuts", async () => {
  const specs: ViewSpec_Deserialize[] = [];
  mockIPC(
    (command, args) => {
      if (command === "window_settings_get") return { browser_visible: true };
      if (command === "import_settings_get") return { copy_files: false, watched_folders: [] };
      if (command === "subscribe_player" || command === "subscribe_scan") return null;
      if (command === "view_open") {
        specs.push((args as { spec: ViewSpec_Deserialize }).spec);
        return { view_id: String(specs.length), count: 0, revision: 0 };
      }
      if (command === "browser_facets") {
        return {
          genres: [{ name: "Jazz", compilation: false }],
          artists: [{ name: "Compilations", compilation: true }],
          albums: [{ name: "Collection", compilation: false }],
        };
      }
      throw new Error(`unexpected command ${command}`);
    },
    { shouldMockEvents: true },
  );
  render(() => <App />);
  fireEvent.click(await screen.findByRole("button", { name: "Jazz" }));
  await waitFor(() => expect(specs.at(-1)?.genre).toBe("Jazz"));
  fireEvent.click(screen.getByRole("button", { name: "Compilations" }));
  await waitFor(() => expect(specs.at(-1)).toMatchObject({ genre: "Jazz", compilation: true }));
  fireEvent.click(screen.getByRole("button", { name: "Collection" }));
  await waitFor(() => expect(specs.at(-1)?.album).toBe("Collection"));

  const input = screen.getByRole<HTMLInputElement>("searchbox", { name: "Search library" });
  fireEvent.keyDown(document, { key: "f", ctrlKey: true });
  expect(document.activeElement).toBe(input);
  fireEvent.input(input, { target: { value: "b" } });
  fireEvent.input(input, { target: { value: "blue" } });
  expect(specs.at(-1)?.query).toBeUndefined();
  await waitFor(() =>
    expect(specs.at(-1)).toMatchObject({
      genre: "Jazz",
      compilation: true,
      album: "Collection",
      query: "blue",
    }),
  );
  expect(specs.some((spec) => spec.query === "b")).toBe(false);
  fireEvent.keyDown(input, { key: "Escape" });
  await waitFor(() => {
    expect(input.value).toBe("");
    expect(specs.at(-1)?.query).toBeUndefined();
  });
});

test("the desktop renders saved settings and updates the backend when appearance changes", async () => {
  const settings: WindowSettings_Serialize = {
    theme: "dark",
    sidebar_width: 250,
    browser_height: 190,
    browser_visible: false,
    artwork_visible: true,
    columns: [{ column: "genre", visible: false, width: null }],
    sort: { column: "album", direction: "descending" },
  };
  const saved: unknown[] = [];
  mockIPC(
    (command, args) => {
      if (command === "window_settings_get") return settings;
      if (command === "import_settings_get") return { copy_files: false, watched_folders: [] };
      if (command === "subscribe_player") return null;
      if (command === "browser_facets") return { genres: [], artists: [], albums: [] };
      if (command === "subscribe_scan") return null;
      if (command === "window_settings_save") {
        saved.push(args);
        return settings;
      }
      if (command === "view_open") {
        expect(args).toEqual({ spec: { sort: settings.sort } });
        return { view_id: "1", count: 0, revision: 0 };
      }
      if (command === "view_rows") return [];
      throw new Error(`unexpected command ${command}`);
    },
    { shouldMockEvents: true },
  );

  const { container } = render(() => <App />);
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("0 songs"));
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(screen.queryByRole("region", { name: "Column browser" })).toBeNull();
  expect(screen.queryByRole("columnheader", { name: "Genre" })).toBeNull();
  expect(screen.getByRole("columnheader", { name: "Album" }).getAttribute("aria-sort")).toBe(
    "descending",
  );
  expect(
    container
      .querySelector<HTMLElement>(".library-window")
      ?.style.getPropertyValue("--window-sidebar"),
  ).toBe("250px");

  fireEvent.click(screen.getByRole("button", { name: "Browse" }));
  await waitFor(() =>
    expect(saved).toEqual([{ settings: { ...settings, browser_visible: true } }]),
  );
  expect(screen.getByRole("region", { name: "Column browser" })).toBeDefined();
});
