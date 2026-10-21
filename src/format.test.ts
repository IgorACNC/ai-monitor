import { describe, expect, it } from "vitest";
import { baseName, dayKey, elapsed, resetsIn, severity } from "./format";

const MIN = 60_000;
const HOUR = 60 * MIN;

describe("resetsIn", () => {
  const now = new Date(2026, 9, 21, 10, 0).getTime();
  it("conta horas e minutos quando falta menos de um dia", () => {
    expect(resetsIn(now + 2 * HOUR + 10 * MIN, now)).toBe("reinicia em 2 h 10 min");
    expect(resetsIn(now + 25 * MIN, now)).toBe("reinicia em 25 min");
    expect(resetsIn(now + 3 * HOUR, now)).toBe("reinicia em 3 h");
  });
  it("mostra o dia da semana quando falta mais de um dia", () => {
    expect(resetsIn(new Date(2026, 9, 23, 14, 0).getTime(), now)).toMatch(/^reinicia sex.*14:00$/);
  });
  it("trata horário já passado", () => {
    expect(resetsIn(now - 1, now)).toBe("reiniciando agora");
  });
});

describe("outros formatos", () => {
  it("elapsed", () => {
    expect(elapsed(0, 30_000)).toBe("agora há pouco");
    expect(elapsed(0, 65 * MIN)).toBe("há 1 h 5 min");
  });
  it("baseName aceita barras do Windows", () => {
    expect(baseName("C:\\Users\\eu\\projeto")).toBe("projeto");
    expect(baseName("/home/eu/projeto/")).toBe("projeto");
  });
  it("dayKey usa o dia local", () => {
    expect(dayKey(new Date(2026, 0, 5, 23, 59).getTime())).toBe("2026-01-05");
  });
  it("severity segue os limites dos alertas", () => {
    expect([severity(74.9), severity(75), severity(90)]).toEqual(["ok", "warn", "high"]);
  });
});
