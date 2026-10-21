import { ActiveSessions } from "../components/ActiveSessions";
import { CommitRow, CommitStats, ContributionGrid } from "../components/Commits";
import { Limits } from "../components/Limits";
import type { CaptureStatus, Commit, Limits as LimitsData, LiveSession } from "../types";

const RECENT_COMMITS = 5;

export function Home({
  now,
  limits,
  capture,
  onEnableCapture,
  sessions,
  onSessionsChanged,
  commits,
  onSeeCommits,
}: {
  now: number;
  limits: LimitsData | null;
  capture: CaptureStatus | null;
  onEnableCapture: () => Promise<void>;
  sessions: LiveSession[];
  onSessionsChanged: () => void;
  commits: Commit[] | null;
  onSeeCommits: () => void;
}) {
  return (
    <div className="home">
      <section className="section">
        <h2 className="section-title">Limites de uso</h2>
        <Limits limits={limits} capture={capture} onEnable={onEnableCapture} now={now} />
      </section>

      <section className="section">
        <h2 className="section-title">
          Sessões ativas {sessions.length > 0 && <span className="count">{sessions.length}</span>}
        </h2>
        <ActiveSessions sessions={sessions} now={now} onChanged={onSessionsChanged} />
      </section>

      <section className="section">
        <div className="section-head">
          <h2 className="section-title">Seus commits</h2>
          {commits && commits.length > 0 && (
            <button className="link-button" onClick={onSeeCommits}>
              Ver todos
            </button>
          )}
        </div>
        {commits === null ? (
          <p className="empty-line">Lendo os repositórios…</p>
        ) : commits.length === 0 ? (
          <p className="empty-line">
            Nenhum commit seu nos últimos 90 dias nas pastas onde o Claude Code trabalhou.
          </p>
        ) : (
          <div className="card commits-card">
            <div className="commits-overview">
              <ContributionGrid commits={commits} weeks={13} now={now} />
              <CommitStats commits={commits} now={now} />
            </div>
            <ul className="commit-list">
              {commits.slice(0, RECENT_COMMITS).map((c) => (
                <CommitRow key={c.sha} commit={c} now={now} />
              ))}
            </ul>
          </div>
        )}
      </section>
    </div>
  );
}
