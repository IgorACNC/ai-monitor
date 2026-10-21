import { dayKey } from "../format";

export type Day = { key: string; date: Date; count: number; future: boolean };

/**
 * Grade no formato do GitHub: uma coluna por semana (domingo a sábado), a última
 * contendo hoje. Dias depois de hoje vêm marcados como `future`.
 */
export function contributionWeeks(timestamps: number[], nowMs: number, weeks: number): Day[][] {
  const counts = new Map<string, number>();
  for (const ts of timestamps) counts.set(dayKey(ts), (counts.get(dayKey(ts)) ?? 0) + 1);

  const today = new Date(nowMs);
  today.setHours(0, 0, 0, 0);
  const start = new Date(today);
  start.setDate(today.getDate() - today.getDay() - (weeks - 1) * 7);

  const columns: Day[][] = [];
  for (let w = 0; w < weeks; w++) {
    const column: Day[] = [];
    for (let d = 0; d < 7; d++) {
      const date = new Date(start);
      date.setDate(start.getDate() + w * 7 + d);
      const key = dayKey(date.getTime());
      column.push({ key, date, count: counts.get(key) ?? 0, future: date > today });
    }
    columns.push(column);
  }
  return columns;
}

/** Nível de 0 a 4 para a cor do quadrado, relativo ao dia mais movimentado. */
export function level(count: number, max: number): 0 | 1 | 2 | 3 | 4 {
  if (count === 0 || max === 0) return 0;
  return Math.min(4, Math.ceil((count / max) * 4)) as 1 | 2 | 3 | 4;
}
