import assert from "node:assert/strict";
import test from "node:test";
import { addSessionPR, isDismissed, parsePRCommand, parsePullRequestURL, removeSessionPR } from "./registry.js";

const url = "https://github.com/Example/Project/pull/12";

test("normalizes PR URLs and comparison keys", () => {
  assert.deepEqual(parsePullRequestURL(`${url}/files?diff=split#section`), {
    key: "example/project#12", repository: "Example/Project", number: 12, url,
  });
});

for (const value of ["not a URL", "https://github.com/example/project/issues/12", "https://github.com.evil/example/project/pull/12", "http://github.com/example/project/pull/12", "https://user:password@github.com/example/project/pull/12", `${url}abc`, "https://github.com/example/project/pull/0", "https://github.com/example/project/pull/9999999999999999999"]) {
  test(`rejects invalid PR URL ${value}`, () => {
    assert.throws(() => parsePullRequestURL(value), /Use a GitHub PR URL/);
  });
}

test("parses slash input and argument-only input", () => {
  assert.deepEqual(parsePRCommand(`/pr add ${url}`), { action: "add", url });
  assert.deepEqual(parsePRCommand(`remove ${url}`), { action: "remove", url });
  assert.deepEqual(parsePRCommand("list"), { action: "list", url: undefined });
  assert.deepEqual(parsePRCommand(), { action: "help", url: undefined });
  assert.deepEqual(parsePRCommand("add"), { action: "add", url: undefined });
  assert.throws(() => parsePRCommand("create"), /Use \/pr/);
  assert.throws(() => parsePRCommand(`list ${url}`), /Use \/pr/);
  assert.throws(() => parsePRCommand(`add ${url} extra`), /Use \/pr/);
});

test("stores unique PRs with explicit provenance and keeps sessions separate", () => {
  const registry = { sessions: {} };
  const reference = { ...parsePullRequestURL(url), title: "Title" };
  assert.equal(addSessionPR(registry, "ses_first", reference, { addedAt: 123 }), true);
  assert.equal(addSessionPR(registry, "ses_first", reference, { addedAt: 456 }), false);
  assert.equal(addSessionPR(registry, "ses_second", reference, { source: "created", addedAt: 456 }), true);
  assert.equal(registry.sessions.ses_first.length, 1);
  assert.equal(registry.sessions.ses_first[0].source, "manual");
  assert.equal(registry.sessions.ses_first[0].addedAt, 123);
  assert.equal(registry.sessions.ses_second[0].source, "created");
  assert.equal(removeSessionPR(registry, "ses_first", reference.key), true);
  assert.equal(registry.sessions.ses_first, undefined);
  assert.equal(registry.sessions.ses_second.length, 1);
  assert.equal(removeSessionPR(registry, "ses_first", reference.key), false);
});

test("removal dismisses a PR for that session and a manual add restores it", () => {
  const registry = { sessions: {} };
  const reference = { ...parsePullRequestURL(url), title: "Title" };
  addSessionPR(registry, "ses_first", reference, { source: "created" });
  addSessionPR(registry, "ses_second", reference, { source: "created" });
  removeSessionPR(registry, "ses_first", reference.key);
  assert.equal(isDismissed(registry, "ses_first", reference.key), true);
  assert.equal(isDismissed(registry, "ses_second", reference.key), false);
  addSessionPR(registry, "ses_first", reference, { source: "created" });
  assert.equal(isDismissed(registry, "ses_first", reference.key), true);
  addSessionPR(registry, "ses_first", reference);
  assert.equal(isDismissed(registry, "ses_first", reference.key), false);
  assert.equal(registry.dismissed.ses_first, undefined);
});

test("registered entries survive JSON persistence", () => {
  const registry = { sessions: {} };
  addSessionPR(registry, "ses_first", { ...parsePullRequestURL(url), title: "Title" }, { addedAt: 123 });
  const restored = JSON.parse(JSON.stringify(registry));
  assert.deepEqual(restored, registry);
  assert.equal(removeSessionPR(restored, "ses_first", parsePullRequestURL(url).key), true);
  assert.deepEqual(JSON.parse(JSON.stringify(restored)).dismissed, { ses_first: ["example/project#12"] });
});
