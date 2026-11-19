import { clearMocks, mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import { events } from "../bindings";
import { artUrl, openLibraryPages } from "./ipc";

afterEach(clearMocks);

test("reuses page requests and invalidates a cached view on a library revision", async () => {
  const calls: unknown[] = [];
  mockIPC(
    (command, args) => {
      if (command === "view_open") return { view_id: "23", count: 600, revision: 4 };
      if (command === "view_rows") {
        calls.push(args);
        return [];
      }
      throw new Error(`unexpected command ${command}`);
    },
    { shouldMockEvents: true },
  );
  const opened = await openLibraryPages({ query: "blue" });
  if (opened.status !== "ok") throw new Error("view failed to open");
  const view = opened.data;
  try {
    await Promise.all([view.pageFor(205), view.pageFor(250)]);
    await view.pageFor(255);
    expect(calls).toEqual([{ viewId: "23", offset: 200, limit: 200 }]);

    await events.libraryChanged.emit({ revision: 4 });
    expect((await view.pageFor(255)).status).toBe("ok");
    await events.libraryChanged.emit({ revision: 5 });
    expect(await view.pageFor(255)).toEqual({ status: "error", error: { kind: "view_expired" } });
    expect(calls).toHaveLength(1);
  } finally {
    view.close();
  }
});

test("the thumbnail URL uses the platform form of the Tauri scheme", () => {
  mockConvertFileSrc("windows");
  expect(artUrl("abc", 600)).toBe("http://mtart.localhost/abc/600");
  mockConvertFileSrc("linux");
  expect(artUrl("abc", 300)).toBe("mtart://localhost/abc/300");
});
