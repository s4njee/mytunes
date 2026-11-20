import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, expect, test, vi } from "vitest";
import { type Track, VirtualTable } from "./VirtualTable";

afterEach(cleanup);

const fixture: Track = {
  id: "",
  name: "",
  seconds: 120,
  artist: "Artist",
  album: "Album",
  genre: "Genre",
  rating: 5,
  plays: 0,
  checked: true,
};

test("a 50,000-song view pools rows and preserves selection and focus while scrolling", async () => {
  const onPlayIndex = vi.fn();
  const onToggle = vi.fn();
  const onTrackWindow = vi.fn();
  const onColumnVisibility = vi.fn();
  const { container } = render(() => (
    <VirtualTable
      tracks={[]}
      trackCount={50_000}
      trackViewId="test-view"
      trackAt={(index) =>
        index >= 0 ? { ...fixture, id: String(index), name: `Song ${index}` } : undefined
      }
      selectedId=""
      onSelect={() => {}}
      onPlayIndex={onPlayIndex}
      onToggleTrackPlayback={onToggle}
      onTrackWindow={onTrackWindow}
      onColumnVisibility={onColumnVisibility}
    />
  ));
  const grid = screen.getByRole("grid", { name: "Songs" });
  const scroll = container.querySelector<HTMLElement>(".track-scroll");
  if (!scroll) throw new Error("Missing track viewport");
  const row = (index: number) => grid.querySelector<HTMLElement>(`[data-track-index="${index}"]`);
  const count = () => grid.querySelectorAll("tr").length;

  expect(grid.getAttribute("aria-rowcount")).toBe("50001");
  expect(count()).toBeLessThanOrEqual(80);
  fireEvent.click(screen.getByRole("button", { name: "Song 0" }));
  await waitFor(() => expect(row(0)?.getAttribute("aria-selected")).toBe("true"));
  fireEvent.click(screen.getByRole("button", { name: "Song 2" }), { ctrlKey: true });
  await waitFor(() => expect(row(2)?.getAttribute("aria-selected")).toBe("true"));
  expect(row(0)?.getAttribute("aria-selected")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Song 4" }), { shiftKey: true });
  await waitFor(() => expect(row(3)?.getAttribute("aria-selected")).toBe("true"));
  expect(row(0)?.getAttribute("aria-selected")).toBe("false");
  expect(document.activeElement).toBe(grid);

  scroll.scrollTop = 33_000 * 34;
  fireEvent.scroll(scroll);
  await waitFor(() => expect(row(33_000)).not.toBeNull());
  expect(count()).toBeLessThanOrEqual(80);
  expect(row(4)).toBeNull();
  expect(document.activeElement).toBe(grid);
  expect(onTrackWindow).toHaveBeenLastCalledWith(32_994, expect.any(Number));
  scroll.scrollTop = 0;
  fireEvent.scroll(scroll);
  await waitFor(() => expect(row(4)?.getAttribute("aria-selected")).toBe("true"));
  fireEvent.keyDown(grid, { key: "Enter" });
  fireEvent.keyDown(grid, { key: " " });
  expect(onPlayIndex).toHaveBeenCalledWith(4);
  expect(onToggle).toHaveBeenCalledWith(4);

  fireEvent.contextMenu(screen.getByRole("columnheader", { name: "Time" }));
  const artist = await screen.findByRole("menuitemcheckbox", { name: "Album Artist" });
  expect(artist.getAttribute("aria-checked")).toBe("false");
  fireEvent.click(artist);
  expect(onColumnVisibility).toHaveBeenCalledWith("album_artist", true);
});
