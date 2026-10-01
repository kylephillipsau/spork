import { strict as assert } from "node:assert";
import { test } from "node:test";
import { codesFrom } from "./lists.ts";

test("codes are read a line at a time, or as a spreadsheet's cells", () => {
  assert.deepEqual(codesFrom("ABC-1234\r\nXYZ-9\n\n  QRS-5  \n"), ["ABC-1234", "XYZ-9", "QRS-5"]);
  assert.deepEqual(codesFrom("ABC-1234\tXYZ-9,QRS-5; LMN-2"), ["ABC-1234", "XYZ-9", "QRS-5", "LMN-2"]);
});

test("a code with a space in it stays one code", () => {
  assert.deepEqual(codesFrom("Floor mat 2 m\nABC-1234"), ["Floor mat 2 m", "ABC-1234"]);
});
