import { convertFileSrc } from "@tauri-apps/api/core";
import {
  type AppError,
  commands,
  events,
  type OpenedView,
  type TrackRow_Serialize,
  type ViewSpec_Deserialize,
} from "../bindings";

const pageSize = 200;
const cachedPages = 8;
type Page = Awaited<ReturnType<typeof commands.viewRows>>;

function expired(): Page {
  return { status: "error", error: { kind: "view_expired" } };
}

/** The thumbnails are served as JPEGs by Tauri's platform-specific scheme. */
export function artUrl(id: string, size: 300 | 600): string {
  return `${convertFileSrc(id, "mtart")}/${size}`;
}

/** A retained library snapshot and its least-recently-used row pages. */
export class LibraryPages {
  readonly view: OpenedView;
  readonly #pages = new Map<number, TrackRow_Serialize[]>();
  readonly #pending = new Map<number, Promise<Page>>();
  readonly #unlisten: () => void;
  #expired = false;

  constructor(view: OpenedView, unlisten: () => void) {
    this.view = view;
    this.#unlisten = unlisten;
  }

  invalidate(revision: number) {
    if (revision > this.view.revision) this.#discard();
  }

  close() {
    this.#discard();
    this.#unlisten();
  }

  #discard() {
    this.#expired = true;
    this.#pages.clear();
    this.#pending.clear();
  }

  async pageFor(row: number): Promise<Page> {
    if (this.#expired) return expired();
    const offset = Math.floor(row / pageSize) * pageSize;
    const cached = this.#pages.get(offset);
    if (cached) {
      this.#pages.delete(offset);
      this.#pages.set(offset, cached);
      return { status: "ok", data: cached };
    }
    const pending = this.#pending.get(offset);
    if (pending) return pending;
    const request = commands
      .viewRows(this.view.view_id, offset, pageSize)
      .then((page): Page => {
        if (this.#expired) return expired();
        if (page.status === "ok") {
          this.#pages.set(offset, page.data);
          if (this.#pages.size > cachedPages) {
            const first = this.#pages.keys().next();
            if (!first.done) this.#pages.delete(first.value);
          }
        }
        return page;
      })
      .finally(() => this.#pending.delete(offset));
    this.#pending.set(offset, request);
    return request;
  }
}

type OpenResult = { status: "ok"; data: LibraryPages } | { status: "error"; error: AppError };

/** Subscribe before opening so an invalidation cannot arrive before the listener. */
export async function openLibraryPages(spec: ViewSpec_Deserialize): Promise<OpenResult> {
  let revision = 0;
  let pages: LibraryPages | undefined;
  const unlisten = await events.libraryChanged.listen(({ payload }) => {
    revision = Math.max(revision, payload.revision);
    pages?.invalidate(revision);
  });
  try {
    const result = await commands.viewOpen(spec);
    if (result.status === "error") return result;
    pages = new LibraryPages(result.data, unlisten);
    pages.invalidate(revision);
    return { status: "ok", data: pages };
  } finally {
    if (!pages) unlisten();
  }
}
