import { createSignal, Show } from "solid-js";
import { commands } from "./bindings";

/** M0 shell: proves the webview can call the Rust core through typed bindings. */
export function App() {
  const [reply, setReply] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const ping = async () => {
    try {
      const result = await commands.ping();
      if (result.status === "error") {
        setReply(null);
        setError("The desktop core could not finish the request.");
        return;
      }
      const pong = result.data;
      setReply(`${pong.message} (core ${pong.core_version})`);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <main class="shell">
      <h1>MyTunes</h1>
      <button type="button" onClick={ping}>
        Ping core
      </button>
      <Show when={reply()}>{(text) => <p data-testid="reply">{text()}</p>}</Show>
      <Show when={error()}>{(text) => <p role="alert">{text()}</p>}</Show>
    </main>
  );
}
