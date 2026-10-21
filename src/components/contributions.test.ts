import { describe, expect, it } from "vitest";
import { contributionWeeks, level } from "./contributions";

describe("contributionWeeks", () => {
  // Quarta-feira, 21/10/2026.
  const now = new Date(2026, 9, 21, 15, 30).getTime();

  it("termina na semana de hoje, começando no domingo", () => {
    const weeks = contributionWeeks([], now, 2);
    expect(weeks).toHaveLength(2);
    expect(weeks[0][0].key).toBe("2026-10-11");
    expect(weeks[1][3].key).toBe("2026-10-21");
    expect(weeks[1][3].future).toBe(false);
    expect(weeks[1][4].future).toBe(true);
  });

  it("conta commits por dia local", () => {
    const at = (d: number, h: number) => new Date(2026, 9, d, h).getTime();
    const weeks = contributionWeeks([at(20, 9), at(20, 23), at(14, 1)], now, 2);
    const byKey = Object.fromEntries(weeks.flat().map((d) => [d.key, d.count]));
    expect(byKey["2026-10-20"]).toBe(2);
    expect(byKey["2026-10-14"]).toBe(1);
    expect(byKey["2026-10-21"]).toBe(0);
  });
});

describe("level", () => {
  it("vai de 0 a 4 relativo ao máximo", () => {
    expect([0, 1, 2, 3, 8].map((c) => level(c, 8))).toEqual([0, 1, 1, 2, 4]);
    expect(level(3, 0)).toBe(0);
  });
});
