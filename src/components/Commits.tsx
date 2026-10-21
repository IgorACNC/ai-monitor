import { dayKey, elapsed, fmt } from "../format";
import type { Commit } from "../types";
import { contributionWeeks, level } from "./contributions";

const monthFmt = new Intl.DateTimeFormat("pt-BR", { month: "short" });
const dayFmt = new Intl.DateTimeFormat("pt-BR", { weekday: "long", day: "numeric", month: "long" });
const WEEKDAY_LABELS = ["", "seg", "", "qua", "", "sex", ""];

/** Grade de contribuições no estilo do GitHub. */
export function ContributionGrid({ commits, weeks, now }: { commits: Commit[]; weeks: number; now: number }) {
  const columns = contributionWeeks(
    commits.map((c) => c.ts),
    now,
    weeks,
  );
  const max = Math.max(0, ...columns.flat().map((d) => d.count));
  return (
    <div className="contrib" role="img" aria-label={`${commits.length} commits nas últimas ${weeks} semanas`}>
      <div className="contrib-weekdays" aria-hidden="true">
        {WEEKDAY_LABELS.map((l, i) => (
          <span key={i}>{l}</span>
        ))}
      </div>
      <div className="contrib-body">
        <div className="contrib-months" aria-hidden="true">
          {columns.map((col, i) => {
            const first = col[0].date;
            const show = i === 0 ? first.getDate() <= 7 : first.getMonth() !== columns[i - 1][0].date.getMonth();
            return <span key={i}>{show ? monthFmt.format(first).replace(".", "") : ""}</span>;
          })}
        </div>
        <div className="contrib-grid">
          {columns.map((col, i) => (
            <div key={i} className="contrib-col">
              {col.map((d) => (
                <span
                  key={d.key}
                  className={`contrib-day level-${d.future ? "none" : level(d.count, max)}`}
                  title={d.future ? undefined : `${d.count} ${d.count === 1 ? "commit" : "commits"} em ${dayFmt.format(d.date)}`}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function PushState({ pushed }: { pushed: boolean | null }) {
  if (pushed === null) return <span className="push push-local">só local</span>;
  return pushed ? <span className="push push-done">enviado</span> : <span className="push push-pending">não enviado</span>;
}

export function CommitRow({ commit, now }: { commit: Commit; now: number }) {
  return (
    <li className="commit">
      <span className="commit-subject">{commit.subject}</span>
      <span className="commit-meta">
        <span className="commit-repo">{commit.repo}</span>
        {commit.branch && <span className="commit-branch">{commit.branch}</span>}
        <span>{elapsed(commit.ts, now)}</span>
        <PushState pushed={commit.pushed} />
      </span>
    </li>
  );
}

/** Números rápidos: commits na semana, dias seguidos com commit e quantos faltam enviar. */
export function commitStats(commits: Commit[], now: number) {
  const weekAgo = now - 7 * 24 * 60 * 60 * 1000;
  const days = new Set(commits.map((c) => dayKey(c.ts)));
  let streak = 0;
  const d = new Date(now);
  // Se hoje ainda não teve commit, a sequência conta a partir de ontem.
  if (!days.has(dayKey(d.getTime()))) d.setDate(d.getDate() - 1);
  while (days.has(dayKey(d.getTime()))) {
    streak++;
    d.setDate(d.getDate() - 1);
  }
  return {
    thisWeek: commits.filter((c) => c.ts >= weekAgo).length,
    streak,
    unpushed: commits.filter((c) => c.pushed === false).length,
  };
}

export function CommitStats({ commits, now }: { commits: Commit[]; now: number }) {
  const s = commitStats(commits, now);
  return (
    <dl className="commit-stats">
      <div>
        <dt>Últimos 7 dias</dt>
        <dd>{fmt.format(s.thisWeek)}</dd>
      </div>
      <div>
        <dt>Dias seguidos</dt>
        <dd>{fmt.format(s.streak)}</dd>
      </div>
      <div>
        <dt>Não enviados</dt>
        <dd className={s.unpushed > 0 ? "stat-attention" : undefined}>{fmt.format(s.unpushed)}</dd>
      </div>
    </dl>
  );
}
