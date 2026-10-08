import assert from "node:assert/strict";
import test from "node:test";
import { createDiscovery, createdPullRequests, readCreatedPullRequests } from "./discovery.js";
import { addSessionPR, removeSessionPR } from "./registry.js";

const url = (number) => `https://github.com/example/project/pull/${number}`;
const shellTool = (command, output, { exit = 0, status = "completed" } = {}) => ({
  type: "tool",
  name: "shell",
  state: { status, input: { command }, content: [{ type: "text", text: output }], metadata: { exit } },
});
const assistant = (...content) => ({ type: "assistant", content });
const numbers = (references) => references.map(({ number }) => number);

test("finds a PR created by a successful agent shell command", () => {
  const found = createdPullRequests([assistant(shellTool("gh pr create --draft --title test", `${url(12)}\n`))]);
  assert.deepEqual(found, [{
    key: "example/project#12", repository: "example/project", number: 12, url: url(12),
  }]);
});

test("finds a PR created by a successful user shell command", () => {
  const found = createdPullRequests([{
    type: "shell", command: "cd repo && gh pr create --fill", status: "exited", exit: 0,
    output: { output: `Creating pull request\n${url(13)}\n` },
  }]);
  assert.deepEqual(numbers(found), [13]);
});

test("ignores PR URLs that a command did not create", () => {
  const found = createdPullRequests([
    assistant(
      shellTool("gh pr list --author @me", `${url(1)}\n${url(2)}`),
      shellTool("gh pr view 3", url(3)),
      shellTool("cat status.test.js", "gh pr create\n" + url(4)),
      shellTool("gh pr create --fill", url(5), { exit: 1 }),
      shellTool("gh pr create --fill", url(6), { status: "running" }),
      shellTool("gh pr create --fill", "", {}),
      { type: "tool", name: "read", state: { status: "completed", input: { path: "notes.md" }, content: [{ type: "text", text: url(7) }] } },
      { type: "text", text: `gh pr create created ${url(8)}` },
    ),
    { type: "user", text: `gh pr create ${url(9)}` },
    { type: "shell", command: "gh pr create --fill", status: "killed", exit: 0, output: { output: url(10) } },
    { type: "shell", command: "ghx pr create", status: "exited", exit: 0, output: { output: url(11) } },
  ]);
  assert.deepEqual(found, []);
});

test("ignores PR URLs in command input and invalid PR numbers", () => {
  const found = createdPullRequests([assistant(
    shellTool(`gh pr create --body "${url(20)}"`, "a pull request already exists"),
    shellTool("gh pr create --fill", `${url(0)}\n${url(21)}abc\n${url(22)}`),
  )]);
  assert.deepEqual(numbers(found), [22]);
});

test("removes duplicate created PRs", () => {
  const found = createdPullRequests([
    assistant(shellTool("gh pr create --fill", url(30))),
    assistant(shellTool("gh pr create --fill && gh pr view --web", `${url(30)}\n${url(30)}`)),
  ]);
  assert.deepEqual(numbers(found), [30]);
});

test("reads all history pages for one session", async () => {
  const calls = [];
  const controller = new AbortController();
  const client = { message: { list: async (input, options) => {
    calls.push(input);
    assert.equal(options.signal, controller.signal);
    return input.cursor ?
      { data: [assistant(shellTool("gh pr create", url(2)))], cursor: { next: null } } :
      { data: [assistant(shellTool("gh pr create", url(1)))], cursor: { next: "older-page" } };
  } } };
  assert.deepEqual(numbers(await readCreatedPullRequests(client, "ses_active", controller.signal)), [1, 2]);
  assert.deepEqual(calls, [
    { sessionID: "ses_active", limit: "100", order: "asc" },
    { sessionID: "ses_active", limit: "100", cursor: "older-page" },
  ]);
});

test("rejects a repeated history cursor", async () => {
  const client = { message: { list: async () => ({ data: [], cursor: { next: "repeated" } }) } };
  await assert.rejects(readCreatedPullRequests(client, "ses_active"), /Repeated session message cursor/);
});

function harness(validate, now = () => 0) {
  const registry = { sessions: {} };
  let validations = 0;
  const discovery = createDiscovery(registry, async (mutate) => { mutate(registry); }, {
    now,
    validate: async (reference, options) => {
      validations++;
      return validate ? validate(reference, options) : { ...reference, title: "Created PR" };
    },
  });
  return { registry, discovery, validations: () => validations };
}

const created = (number) => createdPullRequests([assistant(shellTool("gh pr create", url(number)))]);

test("registers validated created PRs for the session that created them", async () => {
  const state = harness();
  await state.discovery.register("ses_first", created(1));
  assert.equal(state.registry.sessions.ses_first[0].source, "created");
  assert.equal(state.registry.sessions.ses_first[0].title, "Created PR");
  assert.equal(state.registry.sessions.ses_second, undefined);
});

test("does not validate a PR that is already registered", async () => {
  const state = harness();
  addSessionPR(state.registry, "ses_first", { ...created(1)[0], title: "Manual" });
  await state.discovery.register("ses_first", created(1));
  assert.equal(state.validations(), 0);
  assert.equal(state.registry.sessions.ses_first[0].source, "manual");
});

test("does not add a PR that the user removed from the session", async () => {
  const state = harness();
  await state.discovery.register("ses_first", created(1));
  removeSessionPR(state.registry, "ses_first", created(1)[0].key);
  await state.discovery.register("ses_first", created(1));
  assert.equal(state.registry.sessions.ses_first, undefined);
  assert.equal(state.validations(), 1);
});

test("respects a removal that happens during validation", async () => {
  let release;
  const state = harness(async (reference) => {
    await new Promise((resolve) => { release = resolve; });
    return { ...reference, title: "Created PR" };
  });
  const pending = state.discovery.register("ses_first", created(1));
  state.registry.dismissed = { ses_first: [created(1)[0].key] };
  release();
  await pending;
  assert.equal(state.registry.sessions.ses_first, undefined);
});

test("validates each PR once while a lookup is in progress", async () => {
  let release;
  const state = harness(async (reference) => {
    await new Promise((resolve) => { release = resolve; });
    return { ...reference, title: "Created PR" };
  });
  const first = state.discovery.register("ses_first", created(1));
  await state.discovery.register("ses_first", created(1));
  release();
  await first;
  assert.equal(state.validations(), 1);
  assert.equal(state.registry.sessions.ses_first.length, 1);
});

test("does not retry PRs that fail ownership or lookup checks", async () => {
  for (const message of ["PR author does not match the signed-in user", "PR not found or access denied"]) {
    let time = 0;
    const state = harness(async () => { throw new Error(message); }, () => time);
    await state.discovery.register("ses_first", created(1));
    time = 10 * 60_000;
    await state.discovery.register("ses_first", created(1));
    assert.equal(state.validations(), 1);
    assert.deepEqual(state.registry.sessions, {});
  }
});

test("retries transient GitHub failures after a delay", async () => {
  let time = 0;
  let fail = true;
  const state = harness(async (reference) => {
    if (fail) throw new Error("GitHub connection failed");
    return { ...reference, title: "Created PR" };
  }, () => time);
  await state.discovery.register("ses_first", created(1));
  time = 59_999;
  await state.discovery.register("ses_first", created(1));
  assert.equal(state.validations(), 1);
  fail = false;
  time = 60_000;
  await state.discovery.register("ses_first", created(1));
  assert.equal(state.validations(), 2);
  assert.equal(state.registry.sessions.ses_first.length, 1);
});

test("retries a lookup that the sidebar cancelled", async () => {
  const controller = new AbortController();
  let cancel = true;
  const state = harness(async (reference) => {
    if (cancel) {
      controller.abort();
      throw new Error("Cancelled");
    }
    return { ...reference, title: "Created PR" };
  });
  await state.discovery.register("ses_first", created(1), controller.signal);
  cancel = false;
  await state.discovery.register("ses_first", created(1));
  assert.equal(state.validations(), 2);
  assert.equal(state.registry.sessions.ses_first.length, 1);
});

test("does not save PRs after the plugin is disposed", async () => {
  let release;
  const state = harness(async (reference) => {
    await new Promise((resolve) => { release = resolve; });
    return { ...reference, title: "Created PR" };
  });
  const pending = state.discovery.register("ses_first", created(1));
  state.discovery.dispose();
  release();
  await pending;
  assert.deepEqual(state.registry.sessions, {});
});
