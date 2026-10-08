import assert from "node:assert/strict";
import test from "node:test";
import { createPRCommand } from "./commands.js";

const url = "https://github.com/example/project/pull/12";

function harness(validate) {
  const registry = { sessions: {} };
  const toasts = [];
  const alerts = [];
  let route = { type: "session", sessionID: "ses_first" };
  let validations = 0;
  const context = { ui: {
    router: { current: () => route },
    toast: { show: (toast) => toasts.push(toast) },
    dialog: {
      alert: async (alert) => { alerts.push(alert); },
      prompt: async () => url,
      select: async ({ options }) => options[0].value,
    },
  } };
  const command = createPRCommand(context, registry, async (mutate) => { mutate(registry); }, {
    validate: async (reference, options) => {
      validations++;
      return validate ? validate(reference, options) : { ...reference, title: "Title" };
    },
  });
  return { registry, context, command, toasts, alerts, selectSession: (sessionID) => { route = { type: "session", sessionID }; }, home: () => { route = { type: "home" }; }, validations: () => validations };
}

test("adds a validated link only to the selected session and prevents duplicates", async () => {
  const state = harness();
  await state.command.run(`add ${url}`);
  await state.command.run(`add ${url}`);
  assert.equal(state.registry.sessions.ses_first.length, 1);
  assert.equal(state.validations(), 1);
  assert.equal(state.toasts.at(-1).variant, "info");
  state.selectSession("ses_second");
  await state.command.run("list");
  assert.equal(state.alerts.at(-1).message, "No PRs are linked to this session");
});

test("removes a registered link without a GitHub request", async () => {
  const state = harness();
  await state.command.run(`add ${url}`);
  await state.command.run(`remove ${url}`);
  assert.equal(state.registry.sessions.ses_first, undefined);
  assert.equal(state.validations(), 1);
  assert.equal(state.toasts.at(-1).variant, "success");
});

test("offers dialogs when a URL is omitted", async () => {
  const state = harness();
  await state.command.run("add");
  await state.command.run("remove");
  assert.equal(state.registry.sessions.ses_first, undefined);
  assert.equal(state.validations(), 1);
});

test("lists only the current session's saved links", async () => {
  const state = harness();
  await state.command.run(`add ${url}`);
  await state.command.run("list");
  assert.match(state.alerts.at(-1).message, /example\/project#12/);
  assert.match(state.alerts.at(-1).message, /Title/);
  assert.match(state.alerts.at(-1).message, /https:\/\/github.com/);
});

test("keeps registration tied to the session where the command started", async () => {
  let release;
  const state = harness(async (reference) => {
    await new Promise((resolve) => { release = resolve; });
    return { ...reference, title: "Title" };
  });
  const pending = state.command.run(`add ${url}`);
  state.selectSession("ses_second");
  release();
  await pending;
  assert.equal(state.registry.sessions.ses_first.length, 1);
  assert.equal(state.registry.sessions.ses_second, undefined);
});

test("reports failed validation without saving a link", async () => {
  const state = harness(async () => { throw new Error("PR not found or access denied"); });
  await state.command.run(`add ${url}`);
  assert.deepEqual(state.registry.sessions, {});
  assert.equal(state.toasts.at(-1).message, "PR not found or access denied");
});

test("rejects unknown commands and invalid URLs without a GitHub request", async () => {
  const state = harness();
  await state.command.run("create");
  await state.command.run("add invalid");
  assert.equal(state.validations(), 0);
  assert.deepEqual(state.registry.sessions, {});
  assert.equal(state.toasts.length, 2);
});

test("requires an active session", async () => {
  const state = harness();
  state.home();
  await state.command.run(`add ${url}`);
  assert.equal(state.validations(), 0);
  assert.equal(state.toasts.at(-1).message, "Open a session to manage its PRs");
});

test("shows help without changing the registry", async () => {
  const state = harness();
  await state.command.run();
  assert.equal(state.alerts.at(-1).title, "Session PR commands");
  assert.deepEqual(state.registry.sessions, {});
});

test("does not save a link when the plugin is disposed during validation", async () => {
  let release;
  const state = harness(async (reference) => {
    await new Promise((resolve) => { release = resolve; });
    return { ...reference, title: "Title" };
  });
  const pending = state.command.run(`add ${url}`);
  state.command.dispose();
  release();
  await pending;
  assert.deepEqual(state.registry.sessions, {});
  assert.deepEqual(state.toasts, []);
});
