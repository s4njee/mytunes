import { render } from "@solidjs/web";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

if (import.meta.env.MODE === "mock") {
  const { DesktopMock } = await import("./mock/DesktopMock");
  render(() => <DesktopMock />, root);
} else {
  render(() => <App />, root);
}
