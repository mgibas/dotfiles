import { addSessionPR, parsePRCommand, parsePullRequestURL, removeSessionPR } from "./registry.js";
import { validatePullRequest } from "./status.js";

export function createPRCommand(context, registry, updateRegistry, { validate = validatePullRequest } = {}) {
  const controller = new AbortController();

  async function run(input) {
    try {
      const route = context.ui.router.current();
      if (route.type !== "session") throw new Error("Open a session to manage its PRs");
      const sessionID = route.sessionID;
      const command = parsePRCommand(input);

      if (command.action === "help") {
        await context.ui.dialog.alert({
          title: "Session PR commands",
          message: "/pr add <url> — link an existing PR\n/pr remove <url> — remove a link\n/pr list — show this session’s links\n\nThese commands do not change GitHub.",
        });
        return;
      }

      if (command.action === "list") {
        const references = registry.sessions[sessionID] ?? [];
        await context.ui.dialog.alert({
          title: "Session PRs",
          message: references.length ? references.map((pr) => `${pr.repository}#${pr.number}\n${pr.title}\n${pr.url}`).join("\n\n") : "No PRs are linked to this session",
        });
        return;
      }

      let url = command.url;
      if (!url && command.action === "add") {
        url = await context.ui.dialog.prompt({ title: "Add session PR", placeholder: "GitHub PR URL" });
      }
      if (!url && command.action === "remove") {
        const references = registry.sessions[sessionID] ?? [];
        if (!references.length) throw new Error("No PRs are linked to this session");
        url = await context.ui.dialog.select({
          title: "Remove session PR",
          options: references.map((pr) => ({ title: `${pr.repository}#${pr.number}`, description: pr.title, value: pr.url })),
        });
      }
      if (!url) return;
      const reference = parsePullRequestURL(url);
      controller.signal.throwIfAborted();

      if (command.action === "add") {
        if (registry.sessions[sessionID]?.some(({ key }) => key === reference.key)) {
          context.ui.toast.show({ message: "PR is already linked to this session", variant: "info" });
          return;
        }
        const validated = await validate(reference, {
          signal: AbortSignal.any([controller.signal, AbortSignal.timeout(30_000)]),
        });
        controller.signal.throwIfAborted();
        await updateRegistry((draft) => addSessionPR(draft, sessionID, validated));
        context.ui.toast.show({ message: `Linked ${reference.repository}#${reference.number}`, variant: "success" });
      } else {
        let removed = false;
        await updateRegistry((draft) => { removed = removeSessionPR(draft, sessionID, reference.key); });
        if (!removed) throw new Error("PR is not linked to this session");
        context.ui.toast.show({ message: `Removed ${reference.repository}#${reference.number}`, variant: "success" });
      }
    } catch (error) {
      if (!controller.signal.aborted) context.ui.toast.show({ message: error.message, variant: "error" });
    }
  }

  return { run, dispose: () => controller.abort() };
}
