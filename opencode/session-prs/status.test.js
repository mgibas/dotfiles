import assert from "node:assert/strict";
import test from "node:test";
import {
  checkStatus,
  readPullRequests,
  reviewStatus,
  validatePullRequest,
} from "./status.js";
import { parsePullRequestURL } from "./registry.js";

const url = (number) => `https://github.com/example/project/pull/${number}`;
const reference = (number) => parsePullRequestURL(url(number));
const pr = (overrides = {}) => ({
  author: { login: "me" },
  state: "OPEN",
  isDraft: false,
  reviewDecision: "",
  latestReviews: [],
  reviewRequests: [],
  statusCheckRollup: [{}],
  title: "Session PR",
  ...overrides,
});

test("counts failed and cancelled checks before pending checks", () => {
  assert.deepEqual(checkStatus([{ bucket: "fail" }, { bucket: "cancel" }, { bucket: "pending" }]),
    { label: "2 failing", tone: "error" });
});

test("distinguishes pending, passing, and absent checks", () => {
  assert.deepEqual(checkStatus([{ bucket: "pending" }, { bucket: "pass" }]),
    { label: "1 running", tone: "warning" });
  assert.deepEqual(checkStatus([{ bucket: "pass" }, { bucket: "skipping" }]),
    { label: "checks passed", tone: "success" });
  assert.deepEqual(checkStatus([]), { label: "no checks", tone: "muted" });
});

for (const [label, tone, overrides] of [
  ["merged", "accent", { state: "MERGED", isDraft: true }],
  ["closed", "error", { state: "CLOSED", reviewDecision: "APPROVED" }],
  ["draft", "info", { isDraft: true, reviewDecision: "APPROVED" }],
  ["approved", "success", { reviewDecision: "APPROVED" }],
  ["changes requested", "error", { reviewDecision: "CHANGES_REQUESTED" }],
  ["in review", "warning", { reviewDecision: "REVIEW_REQUIRED" }],
  ["changes requested", "error", { latestReviews: [{ state: "APPROVED" }, { state: "CHANGES_REQUESTED" }] }],
  ["approved", "success", { latestReviews: [{ state: "APPROVED" }] }],
  ["in review", "warning", { reviewRequests: [{ login: "reviewer" }] }],
  ["no review", "muted", {}],
]) {
  test(`shows review state ${label}: ${JSON.stringify(overrides)}`, () => {
    assert.deepEqual(reviewStatus(pr(overrides)), { label, tone });
  });
}

test("does not call GitHub when the session has no PR links", async () => {
  const result = await readPullRequests([], { run: () => assert.fail("Unexpected GitHub request") });
  assert.deepEqual(result, []);
});

test("reports an author mismatch on the affected registered PR", async () => {
  const calls = [];
  const result = await readPullRequests([reference(1), reference(2)], {
    run: async (args) => {
      calls.push(args);
      if (args[0] === "api") return "ME\n";
      if (args[1] === "view") return JSON.stringify(pr({ author: { login: args[2] === "1" ? "me" : "other" } }));
      return JSON.stringify([{ bucket: "pass" }]);
    },
  });
  assert.equal(result[0].error, null);
  assert.equal(result[1].error, "PR author does not match the signed-in user");
  assert.equal(calls.filter((args) => args[1] === "checks").length, 1);
  assert.deepEqual(calls[1].slice(0, 5), ["pr", "view", "1", "--repo", "example/project"]);
});

for (const [code, bucket, label] of [[1, "fail", "1 failing"], [8, "pending", "1 running"]]) {
  test(`reads check JSON when gh exits with ${code}`, async () => {
    const result = await readPullRequests([reference(1)], {
      run: async (args) => {
        if (args[0] === "api") return "me";
        if (args[1] === "view") return JSON.stringify(pr());
        throw Object.assign(new Error("Checks are not complete"), {
          code,
          stdout: JSON.stringify([{ bucket }]),
        });
      },
    });
    assert.equal(result[0].checks.label, label);
  });
}

test("shows no checks when GitHub reports an empty check rollup", async () => {
  const result = await readPullRequests([reference(1)], {
    run: async (args) => {
      if (args[0] === "api") return "me";
      if (args[1] === "view") return JSON.stringify(pr({ statusCheckRollup: [] }));
      assert.fail("Empty rollup must not call gh pr checks");
    },
  });
  assert.equal(result[0].checks.label, "no checks");
});

test("shows unavailable checks when GitHub fails or returns invalid JSON", async () => {
  for (const output of ["invalid JSON", new Error("Network request failed")]) {
    const result = await readPullRequests([reference(1)], {
      run: async (args) => {
        if (args[0] === "api") return "me";
        if (args[1] === "view") return JSON.stringify(pr());
        if (output instanceof Error) throw output;
        return output;
      },
    });
    assert.equal(result[0].checks.label, "checks unavailable");
  }
});

test("reports failed PR lookups without suppressing other PRs", async () => {
  const result = await readPullRequests([reference(1), reference(2)], {
    run: async (args) => {
      if (args[0] === "api") return "me";
      if (args[1] === "view" && args[2] === "1") throw new Error("PR is unavailable");
      if (args[1] === "view") return JSON.stringify(pr());
      return "[]";
    },
  });
  assert.deepEqual(result.map(({ number }) => number), [1, 2]);
  assert.equal(result[0].error, "GitHub request failed");
  assert.equal(result[0].stale, false);
  assert.equal(result[1].error, null);
});

test("reports sign-in errors on registered PRs", async () => {
  const result = await readPullRequests([reference(1)], {
    run: async () => { throw new Error("Not signed in"); },
  });
  assert.equal(result[0].error, "GitHub sign-in required");
});

test("propagates cancellation instead of reporting a failed PR", async () => {
  const controller = new AbortController();
  await assert.rejects(readPullRequests([reference(1)], {
    signal: controller.signal,
    run: async (args, signal) => {
      assert.equal(signal, controller.signal);
      if (args[0] === "api") return "me";
      controller.abort();
      throw new Error("Cancelled");
    },
  }), /Cancelled/);
});

test("marks saved status as stale when a lookup fails", async () => {
  const saved = { ...reference(1), title: "Saved title", review: { label: "approved", tone: "success" }, checks: { label: "checks passed", tone: "success" } };
  const result = await readPullRequests([reference(1)], {
    previous: [saved],
    run: async (args) => {
      if (args[0] === "api") return "me";
      throw Object.assign(new Error("Request failed"), { stderr: "Could not resolve to a Repository" });
    },
  });
  assert.equal(result[0].title, "Saved title");
  assert.deepEqual(result[0].review, saved.review);
  assert.equal(result[0].error, "PR not found or access denied");
  assert.equal(result[0].stale, true);
});

test("validates PR ownership and returns its title", async () => {
  const result = await validatePullRequest(reference(1), {
    run: async (args) => args[0] === "api" ? "me" : JSON.stringify(pr()),
  });
  assert.deepEqual(result, { ...reference(1), title: "Session PR" });
});

test("rejects registration of another user's PR", async () => {
  await assert.rejects(validatePullRequest(reference(1), {
    run: async (args) => args[0] === "api" ? "me" : JSON.stringify(pr({ author: { login: "other" } })),
  }), /PR author does not match/);
});
