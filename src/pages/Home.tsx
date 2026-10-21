import { ActiveSessions } from "../components/ActiveSessions";
import { Activity, CommitRow, type GithubState } from "../components/Commits";
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
  github,
  onSeeCommits,
}: {
  now: number;
  limits: LimitsData | null;
  capture: CaptureStatus | null;
  onEnableCapture: () => Promise<void>;
  sessions: LiveSession[];
  onSessionsChanged: () => void;
  commits: Commit[] | null;
  github: GithubState;
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
        ) : (
          <div className="card commits-card">
            <Activity commits={commits} github={github} now={now} />
            {commits.length === 0 ? (
              <p className="empty-line commits-empty">
                Nenhum commit seu nos últimos 90 dias nas pastas onde o Claude Code trabalhou.
              </p>
            ) : (
              <ul className="commit-list">
                {commits.slice(0, RECENT_COMMITS).map((c) => (
                  <CommitRow key={c.sha} commit={c} now={now} />
                ))}
              </ul>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
