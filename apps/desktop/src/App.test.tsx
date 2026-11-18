import { fireEvent, render, screen } from "@solidjs/testing-library";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";

afterEach(() => clearMocks());

test("Ping core shows the reply from the Rust ping command", async () => {
  mockIPC((cmd) => {
    if (cmd === "ping") return { message: "pong", core_version: "0.0.1" };
    throw new Error(`unexpected command ${cmd}`);
  });

  render(() => <App />);
  fireEvent.click(screen.getByRole("button", { name: "Ping core" }));

  expect((await screen.findByTestId("reply")).textContent).toBe("pong (core 0.0.1)");
});
