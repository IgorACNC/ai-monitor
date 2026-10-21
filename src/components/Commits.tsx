import { useState } from "react";
import { elapsed, fmt } from "../format";
import type { Commit, GithubCalendar } from "../types";
import { activityStats, contributionWeeks, countsByDay, level } from "./contributions";

const monthFmt = new Intl.DateTimeFormat("pt-BR", { month: "short" });
const dayFmt = new Intl.DateTimeFormat("pt-BR", { weekday: "long", day: "numeric", month: "long" });
const WEEKDAY_LABELS = ["", "seg", "", "qua", "", "sex", ""];
const WEEKS = 13;

/** Grade no estilo do GitHub. `unit` dá nome ao que é contado ("contribuição", "commit"). */
function ContributionGrid({
  counts,
  now,
  unit,
}: {
  counts: Map<string, number>;
  now: number;
  unit: [string, string];
}) {
  const columns = contributionWeeks(counts, now, WEEKS);
  const max = Math.max(0, ...columns.flat().map((d) => d.count));
  const total = columns.flat().reduce((sum, d) => sum + d.count, 0);
  const label = (n: number) => `${n} ${n === 1 ? unit[0] : unit[1]}`;
  return (
    <div className="contrib" role="img" aria-label={`${label(total)} nas últimas ${WEEKS} semanas`}>
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
                  title={d.future ? undefined : `${label(d.count)} em ${dayFmt.format(d.date)}`}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export type GithubState = {
  calendar: GithubCalendar | null;
  error: string | null;
  /** Quando o gráfico foi baixado pela última vez. */
  fetchedAt: number | null;
  refresh: () => Promise<void>;
};

function GithubSource({ github, now }: { github: GithubState; now: number }) {
  const [busy, setBusy] = useState(false);
  async function refresh() {
    setBusy(true);
    await github.refresh();
    setBusy(false);
  }
  return (
    <div className="activity-source">
      {github.calendar ? (
        <span>
          Gráfico do GitHub de <strong>{github.calendar.login}</strong>
          {github.fetchedAt && <>, atualizado {elapsed(github.fetchedAt, now)}</>}
        </span>
      ) : (
        <span>
          Mostrando os commits deste computador.
          {github.error && <> Gráfico do GitHub indisponível: {github.error}</>}
        </span>
      )}
      <button className="link-button" onClick={refresh} disabled={busy}>
        {busy ? "Atualizando…" : "Atualizar"}
      </button>
    </div>
  );
}

/**
 * Grade e números de atividade. Usa o gráfico do GitHub (igual ao do perfil) quando ele
 * está disponível; senão, os commits lidos dos repositórios do computador.
 */
export function Activity({ commits, github, now }: { commits: Commit[]; github: GithubState; now: number }) {
  const fromGithub = github.calendar !== null;
  const counts = fromGithub
    ? new Map(github.calendar!.days.map((d) => [d.date, d.count]))
    : countsByDay(commits.map((c) => c.ts));
  const unit: [string, string] = fromGithub ? ["contribuição", "contribuições"] : ["commit", "commits"];
  const { last7, streak } = activityStats(counts, now);
  const unpushed = commits.filter((c) => c.pushed === false).length;

  return (
    <div className="activity">
      <GithubSource github={github} now={now} />
      <div className="commits-overview">
        <ContributionGrid counts={counts} now={now} unit={unit} />
        <dl className="commit-stats">
          <div>
            <dt>Últimos 7 dias</dt>
            <dd>{fmt.format(last7)}</dd>
          </div>
          <div>
            <dt>Dias seguidos</dt>
            <dd>{fmt.format(streak)}</dd>
          </div>
          <div>
            <dt>Não enviados</dt>
            <dd className={unpushed > 0 ? "stat-attention" : undefined}>{fmt.format(unpushed)}</dd>
          </div>
        </dl>
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
