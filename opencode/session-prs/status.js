import { execFile } from "node:child_process";
import { promisify } from "node:util";

const exec = promisify(execFile);

export function checkStatus(checks) {
  const failed = checks.filter(({ bucket }) => ["fail", "cancel"].includes(bucket)).length;
  const pending = checks.filter(({ bucket }) => bucket === "pending").length;
  if (failed) return { label: `${failed} failing`, tone: "error" };
  if (pending) return { label: `${pending} running`, tone: "warning" };
  if (checks.length) return { label: "checks passed", tone: "success" };
  return { label: "no checks", tone: "muted" };
}

export function reviewStatus(pr) {
  if (pr.state === "MERGED") return { label: "merged", tone: "accent" };
  if (pr.state === "CLOSED") return { label: "closed", tone: "error" };
  if (pr.isDraft) return { label: "draft", tone: "info" };
  const decision = pr.reviewDecision ||
    (pr.latestReviews?.some(({ state }) => state === "CHANGES_REQUESTED") ? "CHANGES_REQUESTED" :
      pr.latestReviews?.some(({ state }) => state === "APPROVED") ? "APPROVED" :
        pr.reviewRequests?.length ? "REVIEW_REQUIRED" : "NONE");
  if (decision === "CHANGES_REQUESTED") return { label: "changes requested", tone: "error" };
  if (decision === "APPROVED") return { label: "approved", tone: "success" };
  if (decision === "REVIEW_REQUIRED") return { label: "in review", tone: "warning" };
  return { label: "no review", tone: "muted" };
}

async function github(args, signal) {
  const { stdout } = await exec("gh", args, {
    signal,
    timeout: 15_000,
    maxBuffer: 2 * 1024 * 1024,
    encoding: "utf8",
  });
  return stdout;
}

async function readChecks(reference, signal, run) {
  let output;
  try {
    output = await run([
      "pr", "checks", String(reference.number), "--repo", reference.repository,
      "--json", "bucket",
    ], signal);
  } catch (error) {
    if (![1, 8].includes(error.code) || !error.stdout) throw error;
    output = error.stdout;
  }
  return checkStatus(JSON.parse(output));
}

async function signedInUser(signal, run) {
  const login = (await run(["api", "user", "--jq", ".login"], signal)).trim();
  if (!login) throw new Error("GitHub sign-in required");
  return login;
}

async function readPullRequest(reference, login, signal, run) {
  const pr = JSON.parse(await run([
    "pr", "view", String(reference.number), "--repo", reference.repository,
    "--json", "number,url,author,state,isDraft,reviewDecision,latestReviews,reviewRequests,title,statusCheckRollup",
  ], signal));
  if (pr.author?.login?.toLowerCase() !== login.toLowerCase()) {
    throw new Error("PR author does not match the signed-in user");
  }
  return pr;
}

export function githubError(error) {
  const detail = `${error.stderr ?? ""} ${error.message ?? ""}`;
  if (/PR author does not match/.test(detail)) return "PR author does not match the signed-in user";
  if (/sign-in|not signed in|not logged|gh auth login|authentication|bad credentials|401/i.test(detail)) {
    return "GitHub sign-in required";
  }
  if (/could not resolve|could not find|not found|404/i.test(detail)) return "PR not found or access denied";
  if (/timeout|timed out|ENOTFOUND|ECONN|network|error connecting/i.test(detail)) return "GitHub connection failed";
  return "GitHub request failed";
}

export async function validatePullRequest(reference, { signal, run = github } = {}) {
  try {
    const login = await signedInUser(signal, run);
    const pr = await readPullRequest(reference, login, signal, run);
    return { ...reference, title: pr.title };
  } catch (error) {
    if (signal?.aborted) throw error;
    throw new Error(githubError(error));
  }
}

export async function readPullRequests(references, { signal, run = github, previous = [] } = {}) {
  if (!references.length) return [];
  const cached = new Map(previous.map((pr) => [pr.key, pr]));

  function unavailable(reference, error) {
    const saved = cached.get(reference.key);
    return {
      ...reference,
      title: saved?.title ?? reference.title ?? "",
      review: saved?.review ?? null,
      checks: saved?.checks ?? null,
      error: githubError(error),
      stale: Boolean(saved?.review),
    };
  }

  let login;
  try {
    login = await signedInUser(signal, run);
  } catch (error) {
    if (signal?.aborted) throw error;
    return references.map((reference) => unavailable(reference, error));
  }

  async function read(reference) {
    try {
      const pr = await readPullRequest(reference, login, signal, run);
      let checks;
      try {
        checks = pr.statusCheckRollup?.length === 0 ? checkStatus([]) :
          await readChecks(reference, signal, run);
      } catch (error) {
        if (signal?.aborted) throw error;
        checks = { label: "checks unavailable", tone: "warning" };
      }
      return { ...reference, title: pr.title, review: reviewStatus(pr), checks, error: null, stale: false };
    } catch (error) {
      if (signal?.aborted) throw error;
      return unavailable(reference, error);
    }
  }

  const rows = [];
  for (let index = 0; index < references.length; index += 3) {
    rows.push(...await Promise.all(references.slice(index, index + 3).map(read)));
  }
  return rows;
}
