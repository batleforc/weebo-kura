import { beforeEach, describe, expect, it } from "vitest";
import {
  clampWidth,
  loadWidth,
  saveWidth,
  TREE_DEFAULT,
  TREE_MAX,
  TREE_MIN,
} from "./width";

beforeEach(() => localStorage.clear());

describe("tree width", () => {
  it("keeps within bounds, to a quarter rem", () => {
    expect(clampWidth(3)).toBe(TREE_MIN);
    expect(clampWidth(99)).toBe(TREE_MAX);
    expect(clampWidth(20.13)).toBe(20.25);
    expect(clampWidth(Number.NaN)).toBe(TREE_DEFAULT);
  });
  it("remembers the width, and reads a broken one as the default", () => {
    expect(loadWidth()).toBe(TREE_DEFAULT);
    saveWidth(22);
    expect(loadWidth()).toBe(22);
    localStorage.setItem("kura:treeWidth", "wide");
    expect(loadWidth()).toBe(TREE_DEFAULT);
  });
});
