import assert from "node:assert/strict";
import test from "node:test";

import {
  historyFieldPatch,
  mergeEmailList,
  splitPersonName,
  suggestQueryToken,
} from "./contactSuggest.ts";

test("splits a stored display name into given and family names", () => {
  assert.deepEqual(splitPersonName("Ada Lovelace"), {
    givenName: "Ada",
    familyName: "Lovelace",
    displayName: "Ada Lovelace",
  });
  assert.deepEqual(splitPersonName("Madonna"), {
    givenName: "",
    familyName: "Madonna",
    displayName: "Madonna",
  });
  assert.deepEqual(splitPersonName("  Mary Ann  Smith "), {
    givenName: "Mary Ann",
    familyName: "Smith",
    displayName: "Mary Ann Smith",
  });
});

test("adds an email without duplicating one already typed", () => {
  assert.equal(mergeEmailList("", "ada@example.com"), "ada@example.com");
  assert.equal(
    mergeEmailList("ada@example.com", "Ada@Example.com"),
    "ada@example.com",
  );
  assert.equal(
    mergeEmailList("ada@example.com", "other@example.com"),
    "ada@example.com, other@example.com",
  );
});

test("suggestion query uses the address currently being typed", () => {
  assert.equal(suggestQueryToken("ada@example.com"), "ada@example.com");
  assert.equal(suggestQueryToken("ada@example.com, love"), "love");
});

test("history patch carries the saved name and address", () => {
  assert.deepEqual(
    historyFieldPatch({ name: "Ada Lovelace", email: "ada@example.com" }),
    {
      givenName: "Ada",
      familyName: "Lovelace",
      displayName: "Ada Lovelace",
      email: "ada@example.com",
    },
  );
});
