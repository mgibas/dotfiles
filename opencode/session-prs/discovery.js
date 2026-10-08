import { addSessionPR, isDismissed, parsePullRequestURL } from "./registry.js";
import { validatePullRequest } from "./status.js";

const createCommand = /(?:^|[\s;&|(])gh\s+pr\s+create(?=\s|$)/;
const pullRequestURL = /https:\/\/github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+\b/g;
const permanentErrors = new Set(["PR author does not match the signed-in user", "PR not found or access denied"]);
const retryDelay = 60_000;

function outputText(output) {
  if (typeof output === "string") return output;
  if (!Array.isArray(output)) return "";
  return output.map((item) => (item?.type === "text" ? item.text : "")).join("\n");
}

function createdURLs(command, exit, output) {
  if (typeof command !== "string" || !createCommand.test(command) || exit !== 0) return [];
  return outputText(output).match(pullRequestURL) ?? [];
}

function messageURLs(message) {
  if (message.type === "shell") {
    return message.status === "exited" ? createdURLs(message.command, message.exit, message.output?.output) : [];
  }
  if (message.type !== "assistant") return [];
  return (message.content ?? []).flatMap((part) => part.type === "tool" && part.state?.status === "completed" ?
    createdURLs(part.state.input?.command, part.state.metadata?.exit, part.state.content) : []);
}

export function createdPullRequests(messages) {
  const references = new Map();
  for (const url of (messages ?? []).flatMap(messageURLs)) {
    try {
      const reference = parsePullRequestURL(url);
      references.set(reference.key, reference);
    } catch {
      continue;
    }
  }
  return [...references.values()];
}

export async function readCreatedPullRequests(client, sessionID, signal) {
  const references = new Map();
  const seen = new Set();
  let cursor;

  do {
    const response = await client.message.list({
      sessionID,
      limit: "100",
      ...(cursor ? { cursor } : { order: "asc" }),
    }, { signal });
    for (const reference of createdPullRequests(response.data)) references.set(reference.key, reference);
    cursor = response.cursor.next;
    if (cursor && seen.has(cursor)) throw new Error("Repeated session message cursor");
    if (cursor) seen.add(cursor);
  } while (cursor);

  return [...references.values()];
}

export function createDiscovery(registry, updateRegistry, { validate = validatePullRequest, now = Date.now } = {}) {
  const controller = new AbortController();
  const blockedUntil = new Map();

  async function register(sessionID, references, signal = controller.signal) {
    const aborted = () => controller.signal.aborted || signal.aborted;
    for (const reference of references) {
      const attempt = `${sessionID} ${reference.key}`;
      if ((blockedUntil.get(attempt) ?? 0) > now() || isDismissed(registry, sessionID, reference.key) ||
        registry.sessions[sessionID]?.some(({ key }) => key === reference.key)) continue;
      blockedUntil.set(attempt, Infinity);
      try {
        const validated = await validate(reference, {
          signal: AbortSignal.any([controller.signal, signal, AbortSignal.timeout(30_000)]),
        });
        if (aborted()) {
          blockedUntil.delete(attempt);
          return;
        }
        await updateRegistry((draft) => {
          if (!isDismissed(draft, sessionID, reference.key)) addSessionPR(draft, sessionID, validated, { source: "created" });
        });
      } catch (error) {
        if (aborted()) {
          blockedUntil.delete(attempt);
          return;
        }
        if (!permanentErrors.has(error.message)) blockedUntil.set(attempt, now() + retryDelay);
      }
    }
  }

  return { register, dispose: () => controller.abort() };
}
