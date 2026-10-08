export function parsePullRequestURL(value) {
  let url;
  try {
    url = new URL(value.trim());
  } catch {
    throw new Error("Use a GitHub PR URL");
  }
  const match = url.pathname.match(/^\/([\w.-]+)\/([\w.-]+)\/pull\/([1-9]\d*)(?:\/(?:files|commits|checks))?\/?$/);
  if (url.protocol !== "https:" || url.hostname !== "github.com" || url.port ||
    url.username || url.password || !match || !Number.isSafeInteger(Number(match[3]))) {
    throw new Error("Use a GitHub PR URL");
  }
  const repository = `${match[1]}/${match[2]}`;
  const number = Number(match[3]);
  return {
    key: `${repository.toLowerCase()}#${number}`,
    repository,
    number,
    url: `https://github.com/${repository}/pull/${number}`,
  };
}

export function parsePRCommand(input = "") {
  const parts = input.trim().replace(/^\/pr(?:\s+|$)/, "").split(/\s+/).filter(Boolean);
  const action = parts[0] ?? "help";
  if (!["add", "remove", "list", "help"].includes(action) || parts.length > 2 ||
    (["list", "help"].includes(action) && parts.length > 1)) {
    throw new Error("Use /pr add <url>, /pr remove <url>, or /pr list");
  }
  return { action, url: parts[1] };
}

function dismissedKeys(registry, sessionID) {
  return registry.dismissed?.[sessionID] ?? [];
}

export function isDismissed(registry, sessionID, key) {
  return dismissedKeys(registry, sessionID).includes(key);
}

function restoreSessionPR(registry, sessionID, key) {
  if (!isDismissed(registry, sessionID, key)) return;
  const remaining = dismissedKeys(registry, sessionID).filter((dismissed) => dismissed !== key);
  if (remaining.length) registry.dismissed[sessionID] = remaining;
  else delete registry.dismissed[sessionID];
}

export function addSessionPR(registry, sessionID, reference, { source = "manual", addedAt = Date.now() } = {}) {
  if (source === "manual") restoreSessionPR(registry, sessionID, reference.key);
  const references = registry.sessions[sessionID] ?? [];
  if (references.some(({ key }) => key === reference.key)) return false;
  registry.sessions[sessionID] = [...references, { ...reference, source, addedAt }];
  return true;
}

export function removeSessionPR(registry, sessionID, key) {
  const references = registry.sessions[sessionID] ?? [];
  if (!references.some((reference) => reference.key === key)) return false;
  const remaining = references.filter((reference) => reference.key !== key);
  if (remaining.length) registry.sessions[sessionID] = remaining;
  else delete registry.sessions[sessionID];
  registry.dismissed ??= {};
  registry.dismissed[sessionID] = [...dismissedKeys(registry, sessionID), key];
  return true;
}
