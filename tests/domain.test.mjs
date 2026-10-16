import { test } from "node:test";
import assert from "node:assert/strict";
import {
  matchesSearch,
  mergeQueue,
  selectRange,
  formatBytes,
  containsPath,
  validRegistrationPath,
} from "../src/domain.ts";

test("macOS queue paths preserve case and component boundaries", () => {
  assert(containsPath("/Users/me/cache", "/Users/me/cache/nested"));
  assert(!containsPath("/Users/me/cache", "/Users/me/cache2"));
  assert(!containsPath("/Users/me/Cache", "/Users/me/cache"));
  assert.equal(
    mergeQueue([item("a", "/Users/me/Cache")], [item("b", "/Users/me/cache")])
      .length,
    2,
  );
});
test("directory registration validates platform paths and rejects traversal", () => {
  assert(validRegistrationPath("D:/AI/outputs", false));
  assert(validRegistrationPath("/Users/me/AI/outputs", true));
  for (const path of [
    "/",
    "relative",
    "/Users/me/../secret",
    "/Users/me/\0file",
  ])
    assert(!validRegistrationPath(path, true));
  assert(!validRegistrationPath("D:/", false));
  assert(!validRegistrationPath("D:/AI/../secret", false));
});
const item = (id, path, protection = "safe") => ({
  id,
  path,
  protection,
  name: path.split("\\").at(-1),
  source: "Codex",
  category: "cache",
  sizeBytes: 1024,
  complete: true,
});
test("search supports multiple tokens, wildcards and literal regex characters", () => {
  const entry = item("a", "C:\\test\\cache[1].log");
  assert(matchesSearch(entry, "codex cache*.log"));
  assert(matchesSearch(entry, "[1]"));
  assert(!matchesSearch(entry, "cursor cache"));
  assert(!matchesSearch(entry, "[2]"));
});
test("queue survives page selections and collapses parent child overlap", () => {
  const child = item("child", "C:\\cache\\nested");
  const parent = item("parent", "c:/cache");
  assert.deepEqual(mergeQueue([child], [parent]), [parent]);
  assert.deepEqual(mergeQueue([parent], [child]), [parent]);
  assert.equal(mergeQueue([parent], [item("other", "C:\\cache2")]).length, 2);
  assert(!containsPath("C:\\cache", "C:\\cache2"));
});
test("blocked or incomplete items can never enter queue", () => {
  assert.equal(
    mergeQueue(
      [],
      [
        item("a", "C:\\state", "blocked"),
        { ...item("b", "C:\\partial"), complete: false },
      ],
    ).length,
    0,
  );
});
test("Ctrl toggles and Shift selects the whole filtered range across pages", () => {
  const ids = Array.from({ length: 100 }, (_, i) => String(i));
  const selected = selectRange(ids, new Set(["0"]), "0", "45", true, false);
  assert.equal(selected.size, 46);
  assert.equal(selectRange(ids, selected, "0", "20", false, true).size, 45);
  assert.deepEqual(
    [...selectRange(ids, selected, null, "90", false, false)],
    ["90"],
  );
});
test("byte formatting preserves meaning of zero and GiB", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(1024 ** 3), "1.0 GiB");
  assert.equal(formatBytes(NaN), "—");
});
