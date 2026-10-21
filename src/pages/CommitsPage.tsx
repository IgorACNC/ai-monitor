import { CommitRow, CommitStats, ContributionGrid } from "../components/Commits";
import { dayKey } from "../format";
import type { Commit } from "../types";

const dayFmt = new Intl.DateTimeFormat("pt-BR", { weekday: "long", day: "numeric", month: "long" });

function groupByDay(commits: Commit[]) {
  const groups: { key: string; label: string; commits: Commit[] }[] = [];
  for (const c of commits) {
    const key = dayKey(c.ts);
    const last = groups[groups.length - 1];
    if (last?.key === key) last.commits.push(c);
    else groups.push({ key, label: dayFmt.format(c.ts), commits: [c] });
  }
  return groups;
}

export function CommitsPage({ commits, now }: { commits: Commit[] | null; now: number }) {
  if (commits === null) return <p className="empty-line">Lendo os repositórios…</p>;
  if (commits.length === 0) {
    return (
      <p className="empty-line">
        Nenhum commit seu nos últimos 90 dias. Os commits são lidos dos repositórios onde o Claude Code já trabalhou,
        filtrados pelo seu nome e e-mail do git.
      </p>
    );
  }
  return (
    <div className="commits-page">
      <div className="card commits-overview">
        <ContributionGrid commits={commits} weeks={13} now={now} />
        <CommitStats commits={commits} now={now} />
      </div>
      {groupByDay(commits).map((g) => (
        <section key={g.key} className="commit-day">
          <h2 className="commit-day-title">{g.label}</h2>
          <ul className="commit-list card">
            {g.commits.map((c) => (
              <CommitRow key={c.sha} commit={c} now={now} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
