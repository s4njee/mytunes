#!/usr/bin/env node
// End-to-end smoke test: launches the real desktop app through tauri-driver
// (W3C WebDriver), checks typed library IPC and blocks remote content.
//
// Needs: a debug build (`pnpm tauri build --debug --no-bundle`), `tauri-driver`
// (`cargo install tauri-driver --locked`) and the platform WebDriver
// (WebKitWebDriver on Linux, msedgedriver on Windows; macOS is unsupported).
//
// Usage: node e2e/smoke.mjs [path-to-app-binary]

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

const exe = process.platform === "win32" ? ".exe" : "";
const app = resolve(process.argv[2] ?? `../../target/debug/mytunes-desktop${exe}`);
if (!existsSync(app)) {
  console.error(
    `App binary not found: ${app}. Build it with \`pnpm tauri build --debug --no-bundle\`.`,
  );
  process.exit(2);
}

const PORT = 4444;
const base = `http://127.0.0.1:${PORT}`;
const W3C_ELEMENT = "element-6066-11e4-a52f-4f735466cecf";
// W3C drivers use W3C_ELEMENT; some WebKit builds return a single other key.
const elementId = (v) => v?.[W3C_ELEMENT] ?? v?.ELEMENT ?? Object.values(v ?? {})[0];

async function wd(method, path, body) {
  const res = await fetch(base + path, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json();
  if (!res.ok || json.value?.error)
    throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitForDriver() {
  for (let i = 0; i < 50; i++) {
    try {
      const status = await fetch(`${base}/status`);
      if (status.ok) return;
    } catch {
      // The driver is still starting.
    }
    await sleep(100);
  }
  throw new Error("tauri-driver did not start");
}

const appData = mkdtempSync(resolve(tmpdir(), "mytunes-e2e-"));
let remoteRequests = 0;
const server = createServer((_request, response) => {
  remoteRequests += 1;
  response.writeHead(200, { "content-type": "text/html" });
  response.end("Remote content");
});
await new Promise((ready, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", ready);
});
const remote = `http://127.0.0.1:${server.address().port}/remote-test`;
const driver = spawn("tauri-driver", ["--port", String(PORT)], {
  env: { ...process.env, XDG_DATA_HOME: appData, XDG_CONFIG_HOME: appData, APPDATA: appData },
  stdio: ["ignore", "inherit", "inherit"],
});
let session;
let ok = false;
try {
  await waitForDriver();
  session = (
    await wd("POST", "/session", {
      capabilities: { alwaysMatch: { "tauri:options": { application: app } } },
    })
  ).sessionId;
  const s = `/session/${session}`;

  const find = async (css) => {
    for (let i = 0; i < 50; i++) {
      try {
        const id = elementId(
          await wd("POST", `${s}/element`, { using: "css selector", value: css }),
        );
        if (id) return id;
      } catch {
        await sleep(100);
      }
    }
    throw new Error(`element not found: ${css}`);
  };

  const title = await wd("GET", `${s}/title`);
  await find(".track-table");
  await find(".library-summary");

  const ipc = await wd("POST", `${s}/execute/async`, {
    script: `
      const done = arguments[arguments.length - 1];
      (async () => {
        const internals = window.__TAURI_INTERNALS__;
        const pong = await internals.invoke("ping");
        const settings = await internals.invoke("window_settings_get");
        const view = await internals.invoke("view_open", { spec: {} });
        const page = await internals.invoke("view_rows", { viewId: view.view_id, offset: 0, limit: 200 });
        let oversized;
        try {
          await internals.invoke("view_rows", { viewId: view.view_id, offset: 0, limit: 201 });
        } catch (error) {
          oversized = error;
        }
        let player;
        const received = new Promise(resolve => {
          const callback = internals.transformCallback(payload => resolve(payload.message ?? payload), true);
          player = "__CHANNEL__:" + callback;
        });
        await internals.invoke("subscribe_player", { channel: player });
        return { pong, settings, view, page, oversized, initialPlayer: await received };
      })().then(done, error => done({ error: String(error) }));
    `,
    args: [],
  });
  if (
    ipc.error ||
    !Number.isInteger(ipc.view?.revision) ||
    !Array.isArray(ipc.page) ||
    ipc.page.length > 200 ||
    ipc.oversized?.kind !== "page_limit" ||
    ipc.initialPlayer?.state !== "empty" ||
    ipc.pong?.message !== "pong" ||
    !ipc.settings?.sort
  ) {
    throw new Error(`Unexpected desktop IPC response: ${JSON.stringify(ipc)}`);
  }

  const saved = await wd("POST", `${s}/execute/async`, {
    script: `
      const [settings, done] = arguments;
      window.__TAURI_INTERNALS__.invoke("window_settings_save", {
        settings: {
          ...settings,
          theme: "dark",
          sidebar_width: 254,
          browser_visible: false,
          columns: [{ column: "genre", width: null, visible: false }, { column: "duration", width: 90, visible: true }],
          sort: { column: "album", direction: "descending" },
        },
      }).then(done, error => done({ error: String(error) }));
    `,
    args: [ipc.settings],
  });
  if (saved.theme !== "dark") throw new Error(`Save layout failed: ${JSON.stringify(saved)}`);
  await wd("POST", `${s}/refresh`, {});
  const restored = await wd("POST", `${s}/execute/async`, {
    script: `
      const done = arguments[arguments.length - 1];
      let remaining = 40;
      const check = () => {
        if (document.documentElement.dataset.theme === "dark") {
          const sidebar = document.querySelector(".library-sidebar")?.getBoundingClientRect();
          done(Boolean(sidebar?.width === 254 &&
            !document.querySelector(".column-browser") &&
            !Array.from(document.querySelectorAll(".track-table th")).some(cell => cell.textContent.trim() === "Genre") &&
            document.querySelector(".track-time-column")?.style.width === "90px" &&
            document.querySelector('th[aria-sort="descending"]')?.textContent.trim() === "Album"));
        } else if (remaining-- > 0) setTimeout(check, 50);
        else done(false);
      };
      check();
    `,
    args: [],
  });
  if (!restored) throw new Error("The window did not restore its saved layout");

  const violation = await wd("POST", `${s}/execute/async`, {
    script: `
      const [url, done] = arguments;
      document.addEventListener("securitypolicyviolation", event => {
        if (event.blockedURI === url) done(event.effectiveDirective);
      });
      const image = new Image();
      image.src = url;
      document.body.append(image);
      setTimeout(() => done("no CSP violation"), 2000);
    `,
    args: [remote],
  });
  if (violation !== "img-src") throw new Error(`Remote image protection: ${violation}`);

  await wd("POST", `${s}/execute/sync`, {
    script: "window.location.href = arguments[0];",
    args: [remote],
  });
  await sleep(250);
  const url = await wd("GET", `${s}/url`);
  if (url === remote || remoteRequests !== 0)
    throw new Error("The webview requested remote content");

  console.log(`window title: ${title}`);
  const message = await wd("GET", `${s}/element/${await find(".library-summary")}/text`);
  console.log(`library status: ${message}`);
  console.log(
    "Typed IPC, player subscription, restored layout and remote content protection passed",
  );
  ok = title === "MyTunes" && message === "0 songs, 0 min, 0 B";
} catch (e) {
  console.error(e);
} finally {
  if (session) await wd("DELETE", `/session/${session}`).catch(() => {});
  driver.kill();
  server.close();
  rmSync(appData, { recursive: true, force: true });
}

console.log(ok ? "SMOKE OK" : "SMOKE FAILED");
process.exit(ok ? 0 : 1);
