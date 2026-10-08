import { Plugin } from "@opencode/plugin/tui";
import { ErrorBoundary, For, Show, createEffect, createSignal, onCleanup, untrack } from "solid-js";
import { createPRCommand } from "./commands.js";
import { createDiscovery, createdPullRequests, readCreatedPullRequests } from "./discovery.js";
import { parsePullRequestURL } from "./registry.js";
import { readPullRequests } from "./status.js";

type PullRequest = Awaited<ReturnType<typeof readPullRequests>>[number];
type Reference = ReturnType<typeof parsePullRequestURL> & { title: string; source: string; addedAt: number };

export default Plugin.define({
  id: "mg.session-prs",
  setup(context) {
    const [registry, updateRegistry] = context.storage.store("registry", {
      initial: { sessions: {} as Record<string, Reference[]>, dismissed: {} as Record<string, string[]> },
    });
    const command = createPRCommand(context, registry, updateRegistry);
    const discovery = createDiscovery(registry, updateRegistry);
    const stopCommands = context.ui.slot({
      append: "app",
      render: () => {
        context.keymap.layer(() => ({
          mode: "global",
          commands: [{
            id: "mg.session-prs.manage",
            title: "Manage session PRs",
            group: "Session PRs",
            palette: true,
            slash: { name: "pr", arguments: true },
            run: command.run,
          }],
        }));
        return null;
      },
    });

    function PullRequests(props: { sessionID: string }) {
      const [rows, setRows] = createSignal<PullRequest[]>([]);
      const [notice, setNotice] = createSignal("");

      createEffect(() => {
        const sessionID = props.sessionID;
        const controller = new AbortController();
        void readCreatedPullRequests(context.client, sessionID, AbortSignal.any([
          controller.signal, AbortSignal.timeout(30_000),
        ])).then((found) => discovery.register(sessionID, found, controller.signal)).catch(() => {});
        onCleanup(() => controller.abort());
      });

      createEffect(() => {
        const sessionID = props.sessionID;
        const found = createdPullRequests(context.data.session.message.list(sessionID));
        untrack(() => void discovery.register(sessionID, found));
      });

      createEffect(() => {
        const sessionID = props.sessionID;
        const references = (registry.sessions[sessionID] ?? []).map((reference) => ({ ...reference }));
        const controller = new AbortController();
        let disposed = false;
        let timer: ReturnType<typeof setTimeout>;
        let previous: PullRequest[] = [];
        setRows([]);
        setNotice(references.length ? "Loading PRs…" : "No session PRs · /pr add <url>");

        async function refresh() {
          if (disposed) return;
          try {
            const result = await readPullRequests(references, {
              signal: controller.signal,
              previous,
            });
            if (disposed) return;
            previous = result;
            setRows(previous);
            setNotice("");
          } catch {
            if (!disposed) setNotice("Could not refresh session PRs");
          } finally {
            if (!disposed) timer = setTimeout(() => void refresh(), 60_000);
          }
        }

        if (references.length) void refresh();

        onCleanup(() => {
          disposed = true;
          controller.abort();
          clearTimeout(timer);
        });
      });

      const color = (tone: string) => {
        if (tone === "muted") return context.theme.text.muted;
        if (tone === "accent") return (context.theme.hue.purple ?? context.theme.hue.accent)[300];
        if (tone === "info") return context.theme.text.feedback.info.base;
        return context.theme.text.feedback[tone as "success" | "warning" | "error"].base;
      };

      return (
        <box flexDirection="column" gap={1} marginTop={1}>
          <text fg={context.theme.text.base}><b>Session PRs</b></text>
          <For each={rows().slice(0, 5)}>
            {(pr) => (
              <box flexDirection="column">
                <text fg={context.theme.text.base}>
                  <a href={pr.url}>{pr.repository.split("/")[1]}#{pr.number}</a>
                </text>
                <text fg={context.theme.text.muted} wrapMode="word">
                  <a href={pr.url}>{pr.title}</a>
                </text>
                <Show when={pr.review && pr.checks}>
                  <text>
                    <span style={{ fg: color(pr.review!.tone), bold: true }}>{pr.review!.label}</span>
                    <span style={{ fg: context.theme.text.muted }}> · </span>
                    <span style={{ fg: color(pr.checks!.tone), bold: true }}>{pr.checks!.label}</span>
                  </text>
                </Show>
                <Show when={pr.error}>
                  <text fg={color("warning")} wrapMode="word">
                    {pr.stale ? "Stale · " : ""}{pr.error}
                  </text>
                </Show>
              </box>
            )}
          </For>
          <Show when={rows().length > 5}>
            <text fg={context.theme.text.muted}>+{rows().length - 5} more</text>
          </Show>
          <Show when={notice()}>
            <text fg={context.theme.text.muted} wrapMode="word">{notice()}</text>
          </Show>
        </box>
      );
    }

    const stopSidebar = context.ui.slot({
      prepend: "sidebar.content",
      render: (props) => (
        <ErrorBoundary fallback={(error) => (
          <text fg={context.theme.text.feedback.error.base} wrapMode="word">Session PRs failed: {String(error?.message ?? error)}</text>
        )}>
          <PullRequests sessionID={props.sessionID} />
        </ErrorBoundary>
      ),
    });
    return () => {
      command.dispose();
      discovery.dispose();
      stopCommands();
      stopSidebar();
    };
  },
});
