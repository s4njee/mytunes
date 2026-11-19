import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";
import type { WindowSettings_Serialize } from "./bindings";

afterEach(() => {
  clearMocks();
  delete document.documentElement.dataset.theme;
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
