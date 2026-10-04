import assert from "node:assert/strict";
import test from "node:test";

import { nextIdAfterRemoval } from "./nextAfterDelete.ts";
import { orderTriageQueue, triageNewCount } from "./triageQueue.ts";

function message(id: string, unread: boolean) {
  return { id, unread };
}

test("delete selects the following message, or the previous at the end", () => {
  assert.equal(nextIdAfterRemoval(["a", "b", "c"], "a"), "b");
  assert.equal(nextIdAfterRemoval(["a", "b", "c"], "b"), "c");
  assert.equal(nextIdAfterRemoval(["a", "b", "c"], "c"), "b");
  assert.equal(nextIdAfterRemoval(["only"], "only"), null);
  assert.equal(nextIdAfterRemoval(["a", "b"], "missing"), null);
});

test("triage shows unread first and then the rest in original order", () => {
  const queue = orderTriageQueue([
    message("old-1", false),
    message("new-1", true),
    message("old-2", false),
    message("new-2", true),
    message("new-1", true),
  ]);
  assert.deepEqual(
    queue.map((item) => item.id),
    ["new-1", "new-2", "old-1", "old-2"],
  );
  assert.equal(triageNewCount(queue), 2);
  assert.equal(triageNewCount(queue.slice(2)), 0);
  assert.equal(triageNewCount([message("n", true)]), 1);
});
