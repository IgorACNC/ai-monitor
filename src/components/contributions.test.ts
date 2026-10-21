import { describe, expect, it } from "vitest";
import { activityStats, contributionWeeks, countsByDay, level } from "./contributions";

const at = (d: number, h = 12) => new Date(2026, 9, d, h).getTime();

describe("contributionWeeks", () => {
  // Quarta-feira, 21/10/2026.
  const now = new Date(2026, 9, 21, 15, 30).getTime();

  it("termina na semana de hoje, começando no domingo", () => {
    const weeks = contributionWeeks(new Map(), now, 2);
    expect(weeks).toHaveLength(2);
    expect(weeks[0][0].key).toBe("2026-10-11");
    expect(weeks[1][3].key).toBe("2026-10-21");
    expect(weeks[1][3].future).toBe(false);
    expect(weeks[1][4].future).toBe(true);
  });

  it("usa as contagens por dia, venham do GitHub ou dos commits locais", () => {
    const local = countsByDay([at(20, 9), at(20, 23), at(14, 1)]);
    const github = new Map([["2026-10-21", 6]]);
    const byKey = (m: Map<string, number>) =>
      Object.fromEntries(contributionWeeks(m, now, 2).flat().map((d) => [d.key, d.count]));
    expect(byKey(local)["2026-10-20"]).toBe(2);
    expect(byKey(local)["2026-10-14"]).toBe(1);
    expect(byKey(github)["2026-10-21"]).toBe(6);
  });
});

describe("level", () => {
  it("vai de 0 a 4 relativo ao máximo", () => {
    expect([0, 1, 2, 3, 8].map((c) => level(c, 8))).toEqual([0, 1, 1, 2, 4]);
    expect(level(3, 0)).toBe(0);
  });
});

describe("activityStats", () => {
  const now = at(21, 18);

  it("soma os últimos 7 dias, hoje incluído", () => {
    const counts = new Map([["2026-10-21", 6], ["2026-10-15", 2], ["2026-10-14", 5]]);
    expect(activityStats(counts, now).last7).toBe(8);
  });

  it("sequência inclui hoje quando já houve atividade", () => {
    expect(activityStats(countsByDay([at(21), at(20), at(19), at(17)]), now).streak).toBe(3);
  });

  it("sequência começa ontem quando hoje ainda não teve atividade", () => {
    expect(activityStats(countsByDay([at(20), at(19)]), now).streak).toBe(2);
    expect(activityStats(countsByDay([at(18)]), now).streak).toBe(0);
  });
});
