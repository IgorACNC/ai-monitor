export const fmt = new Intl.NumberFormat("pt-BR");

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

/** "há 5 min", "há 2 h 10 min", "há 3 dias". */
export function elapsed(fromMs: number, nowMs: number) {
  const min = Math.max(0, Math.floor((nowMs - fromMs) / MIN));
  if (min < 1) return "agora há pouco";
  if (min < 60) return `há ${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return min % 60 ? `há ${h} h ${min % 60} min` : `há ${h} h`;
  const d = Math.floor(h / 24);
  return d === 1 ? "há 1 dia" : `há ${d} dias`;
}

const weekdayTime = new Intl.DateTimeFormat("pt-BR", { weekday: "short", hour: "2-digit", minute: "2-digit" });

/** "reinicia em 2 h 10 min" quando falta menos de um dia; senão "reinicia sex., 14:00". */
export function resetsIn(atMs: number, nowMs: number) {
  const left = atMs - nowMs;
  if (left <= 0) return "reiniciando agora";
  if (left >= DAY) return `reinicia ${weekdayTime.format(atMs)}`;
  const h = Math.floor(left / HOUR);
  const m = Math.ceil((left % HOUR) / MIN);
  if (h === 0) return `reinicia em ${m} min`;
  if (m === 0 || m === 60) return `reinicia em ${m === 60 ? h + 1 : h} h`;
  return `reinicia em ${h} h ${m} min`;
}

/** Último trecho de um caminho do Windows ou Unix. */
export function baseName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** Chave do dia local, "2026-10-21". */
export function dayKey(ms: number) {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Faixa de cor de um percentual de limite, igual aos alertas: até 75, até 90, acima. */
export function severity(pct: number): "ok" | "warn" | "high" {
  return pct >= 90 ? "high" : pct >= 75 ? "warn" : "ok";
}
