import { useState } from "react";
import { Activity, CommitRow, type GithubState } from "../components/Commits";
import { PullRequests, type PullRequestsState } from "../components/PullRequests";
import { dayKey } from "../format";
import type { Commit } from "../types";

const dayFmt = new Intl.DateTimeFormat("pt-BR", { weekday: "long", day: "numeric", month: "long" });
/** Quantos dias com commits aparecem antes do "Ver mais". */
const DAYS_SHOWN = 7;

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

function CommitDays({ commits, now }: { commits: Commit[]; now: number }) {
  const [all, setAll] = useState(false);
  if (commits.length === 0) {
    return (
      <p className="empty-line">
        Nenhum commit seu nos últimos 90 dias neste computador. Os commits são lidos dos repositórios onde o Claude
        Code já trabalhou, filtrados pelo seu nome e e-mail do git.
      </p>
    );
  }
  const groups = groupByDay(commits);
  return (
    <>
      {(all ? groups : groups.slice(0, DAYS_SHOWN)).map((g) => (
        <section key={g.key} className="commit-day">
          <h2 className="commit-day-title">{g.label}</h2>
          <ul className="commit-list card">
            {g.commits.map((c) => (
              <CommitRow key={c.sha} commit={c} now={now} />
            ))}
          </ul>
        </section>
      ))}
      {groups.length > DAYS_SHOWN && (
        <button className="button button-quiet see-more" onClick={() => setAll(!all)}>
          {all ? "Ver menos" : `Ver mais ${groups.length - DAYS_SHOWN} dias`}
        </button>
      )}
    </>
  );
}

type View = "prs" | "commits";

export function GithubPage({
  commits,
  github,
  prs,
  now,
}: {
  commits: Commit[] | null;
  github: GithubState;
  prs: PullRequestsState;
  now: number;
}) {
  const [view, setView] = useState<View>("prs");
  const waiting = prs.data?.reviewRequested.length ?? 0;
  return (
    <div className="commits-page">
      <div className="card">
        <Activity commits={commits ?? []} github={github} now={now} />
      </div>

      <div className="segmented" role="tablist" aria-label="O que mostrar">
        <button role="tab" aria-selected={view === "prs"} className={view === "prs" ? "active" : ""} onClick={() => setView("prs")}>
          Pull requests {waiting > 0 && <span className="count count-accent">{waiting}</span>}
        </button>
        <button
          role="tab"
          aria-selected={view === "commits"}
          className={view === "commits" ? "active" : ""}
          onClick={() => setView("commits")}
        >
          Commits
        </button>
      </div>

      {view === "prs" && <PullRequests state={prs} now={now} />}
      {view === "commits" &&
        (commits === null ? <p className="empty-line">Lendo os repositórios…</p> : <CommitDays commits={commits} now={now} />)}
    </div>
  );
}
