import { describe, expect, it } from "vitest";
import type { Commit } from "../types";
import { commitStats } from "./Commits";

const at = (d: number, h = 12) => new Date(2026, 9, d, h).getTime();
const commit = (ts: number, pushed: boolean | null = true): Commit => ({
  sha: String(ts),
  repo: "r",
  branch: "main",
  ts,
  subject: "x",
  pushed,
});

describe("commitStats", () => {
  const now = at(21, 18);

  it("conta a semana e os não enviados", () => {
    const s = commitStats([commit(at(21)), commit(at(20), false), commit(at(10))], now);
    expect(s.thisWeek).toBe(2);
    expect(s.unpushed).toBe(1);
  });

  it("sequência inclui hoje quando já houve commit", () => {
    expect(commitStats([commit(at(21)), commit(at(20)), commit(at(19)), commit(at(17))], now).streak).toBe(3);
  });

  it("sequência começa ontem quando hoje ainda não teve commit", () => {
    expect(commitStats([commit(at(20)), commit(at(19))], now).streak).toBe(2);
    expect(commitStats([commit(at(18))], now).streak).toBe(0);
  });
});
