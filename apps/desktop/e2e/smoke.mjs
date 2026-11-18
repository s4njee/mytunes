#!/usr/bin/env node
// End-to-end smoke test: launches the real desktop app through tauri-driver
// (W3C WebDriver), clicks "Ping core" and checks the reply came from Rust.
//
// Needs: a debug build (`pnpm tauri build --debug --no-bundle`), `tauri-driver`
// (`cargo install tauri-driver --locked`) and the platform WebDriver
// (WebKitWebDriver on Linux, msedgedriver on Windows; macOS is unsupported).
//
// Usage: node e2e/smoke.mjs [path-to-app-binary]

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
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
      await fetch(`${base}/status`);
      return;
    } catch {
      await sleep(100);
    }
  }
  throw new Error("tauri-driver did not start");
}

const driver = spawn("tauri-driver", ["--port", String(PORT)], {
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
  const button = await find("button");
  await wd("POST", `${s}/element/${button}/click`, {});
  const reply = await wd("GET", `${s}/element/${await find("[data-testid=reply]")}/text`);

  console.log(`window title: ${title}`);
  console.log(`reply: ${reply}`);
  ok = title === "MyTunes" && /^pong \(core \d+\.\d+\.\d+\)$/.test(reply);
} catch (e) {
  console.error(e);
} finally {
  if (session) await wd("DELETE", `/session/${session}`).catch(() => {});
  driver.kill();
}

console.log(ok ? "SMOKE OK" : "SMOKE FAILED");
process.exit(ok ? 0 : 1);
