import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { elapsed, fmt } from "../format";
import type { Analysis, Analyzer, CheckOutcome, MergeMethod, PullRequest, PullRequests as PullRequestsData } from "../types";
import { AnalyzerPicker, analyzerLabel, useAnalyzer } from "./Analyzer";

export type PullRequestsState = {
  data: PullRequestsData | null;
  error: string | null;
  fetchedAt: number | null;
  refresh: () => Promise<void>;
  analyses: PrAnalyses;
};

const prKey = (pr: { repo: string; number: number }) => `${pr.repo}#${pr.number}`;

type Running = { analyzer: Analyzer; startedAt: number };

export type PrAnalyses = {
  saved: Record<string, Analysis>;
  running: Record<string, Running>;
  errors: Record<string, string>;
  analyze: (pr: PullRequest, analyzer: Analyzer) => void;
};

/**
 * Pareceres dos PRs. Fica no App (e não na aba) para uma análise de minutos continuar
 * aparecendo como "analisando" mesmo se a pessoa trocar de aba no meio.
 */
export function usePrAnalyses(): PrAnalyses {
  const [saved, setSaved] = useState<Record<string, Analysis>>({});
  const [running, setRunning] = useState<Record<string, Running>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    invoke<Analysis[]>("get_pr_analyses")
      .then((list) => setSaved(Object.fromEntries(list.map((a) => [prKey(a), a]))))
      .catch(() => {});
  }, []);

  const analyze = useCallback((pr: PullRequest, analyzer: Analyzer) => {
    const key = prKey(pr);
    const without = <T,>(rec: Record<string, T>) => Object.fromEntries(Object.entries(rec).filter(([k]) => k !== key));
    setRunning((r) => ({ ...r, [key]: { analyzer, startedAt: Date.now() } }));
    setErrors(without);
    invoke<Analysis>("analyze_pull_request", { repo: pr.repo, number: pr.number, analyzer })
      .then((a) => setSaved((s) => ({ ...s, [key]: a })))
      .catch((e) => setErrors((er) => ({ ...er, [key]: String(e) })))
      .finally(() => setRunning(without));
  }, []);

  return { saved, running, errors, analyze };
}

const CHECK_LABEL: Record<CheckOutcome, string> = {
  success: "checks ok",
  failure: "checks falharam",
  pending: "checks rodando",
  neutral: "checks neutros",
};

const REVIEW_LABEL: Record<string, string> = {
  APPROVED: "aprovado",
  CHANGES_REQUESTED: "mudanças pedidas",
  REVIEW_REQUIRED: "aguardando revisão",
};

function Checks({ outcome }: { outcome: CheckOutcome | null }) {
  if (!outcome) return null;
  return <span className={`check check-${outcome}`}>{CHECK_LABEL[outcome]}</span>;
}

function PrRow({
  pr,
  showAuthor,
  selected,
  onSelect,
  now,
}: {
  pr: PullRequest;
  showAuthor: boolean;
  selected: boolean;
  onSelect: () => void;
  now: number;
}) {
  return (
    <li>
      <button className={`pr-row${selected ? " selected" : ""}`} onClick={onSelect} aria-pressed={selected}>
        <span className="pr-title">
          {pr.draft && <span className="pr-badge">rascunho</span>}
          {pr.title}
        </span>
        <span className="pr-meta">
          <span className="commit-repo">
            {pr.repo}#{pr.number}
          </span>
          {showAuthor && <span>{pr.author}</span>}
          <span>atualizado {elapsed(pr.updatedAt, now)}</span>
          <Checks outcome={pr.checks} />
        </span>
      </button>
    </li>
  );
}

function PrGroup({
  title,
  empty,
  prs,
  showAuthor,
  selected,
  onSelect,
  now,
}: {
  title: string;
  empty: string;
  prs: PullRequest[];
  showAuthor: boolean;
  selected: string | null;
  onSelect: (key: string) => void;
  now: number;
}) {
  return (
    <section>
      <h2 className="commit-day-title">
        {title} <span className="count">{prs.length}</span>
      </h2>
      {prs.length === 0 ? (
        <p className="empty-line pr-empty">{empty}</p>
      ) : (
        <ul className="pr-list card">
          {prs.map((pr) => (
            <PrRow key={prKey(pr)} pr={pr} showAuthor={showAuthor} selected={selected === prKey(pr)} onSelect={() => onSelect(prKey(pr))} now={now} />
          ))}
        </ul>
      )}
    </section>
  );
}

const BODY_PREVIEW = 400;

const MERGE_LABEL: Record<MergeMethod, string> = {
  merge: "com um commit de merge",
  squash: "juntando tudo num commit só (squash)",
  rebase: "reaplicando os commits (rebase)",
};

/** Por que o merge não pode ser feito agora, ou `null` se pode. */
function mergeBlocker(pr: PullRequest): string | null {
  if (pr.draft) return "É um rascunho: marque como pronto no GitHub antes do merge.";
  if (pr.mergeable === "CONFLICTING") return "Tem conflitos com a branch de destino; resolva antes do merge.";
  if (!pr.mergeMethod) return "O repositório não permite merge por aqui.";
  return null;
}

type Pending = "approve" | "requestChanges" | "merge";

/**
 * Ações no PR. Tudo aparece para outras pessoas no GitHub e não dá para desfazer pelo app,
 * então cada ação abre uma confirmação antes.
 */
function PrActions({ pr, mine, onDone }: { pr: PullRequest; mine: boolean; onDone: (message: string) => void }) {
  const [pending, setPending] = useState<Pending | null>(null);
  const [comment, setComment] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const blocker = mine ? mergeBlocker(pr) : null;

  function ask(p: Pending) {
    setPending(p);
    setError(null);
  }

  async function confirm() {
    setBusy(true);
    setError(null);
    try {
      if (pending === "merge") {
        await invoke("merge_pull_request", { repo: pr.repo, number: pr.number, method: pr.mergeMethod, headSha: pr.headSha });
        onDone(`Merge feito em ${pr.repo}#${pr.number}.`);
      } else {
        await invoke("review_pull_request", { repo: pr.repo, number: pr.number, action: pending, body: comment });
        onDone(pending === "approve" ? `${pr.repo}#${pr.number} aprovado.` : `Mudanças pedidas em ${pr.repo}#${pr.number}.`);
      }
      setPending(null);
      setComment("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (pending) {
    const needsComment = pending === "requestChanges";
    return (
      <div className="pr-confirm" role="group" aria-label="Confirmar ação no PR">
        {pending === "merge" ? (
          <p>
            Juntar <span className="commit-branch">{pr.head}</span> em <span className="commit-branch">{pr.base}</span>{" "}
            {MERGE_LABEL[pr.mergeMethod!]}? Não dá para desfazer pelo app.
          </p>
        ) : (
          <>
            <p>
              {pending === "approve" ? "Aprovar" : "Pedir mudanças em"} {pr.repo}#{pr.number}? {pr.author} vê isso no GitHub.
            </p>
            <textarea
              className="pr-comment"
              value={comment}
              onChange={(e) => setComment(e.target.value)}
              placeholder={needsComment ? "O que precisa mudar (obrigatório)" : "Comentário (opcional)"}
              rows={3}
              autoFocus
            />
          </>
        )}
        {error && <p className="inline-error">{error}</p>}
        <div className="pr-confirm-buttons">
          <button
            className={`button ${pending === "requestChanges" ? "button-danger" : "button-primary"}`}
            onClick={confirm}
            disabled={busy || (needsComment && !comment.trim())}
          >
            {busy ? "Enviando…" : pending === "merge" ? "Fazer merge" : pending === "approve" ? "Aprovar" : "Pedir mudanças"}
          </button>
          <button className="button button-quiet" onClick={() => setPending(null)} disabled={busy}>
            Cancelar
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="pr-actions">
      {mine ? (
        <button className="button button-primary" onClick={() => ask("merge")} disabled={!!blocker} title={blocker ?? undefined}>
          Fazer merge
        </button>
      ) : (
        <>
          <button className="button button-primary" onClick={() => ask("approve")}>
            Aprovar
          </button>
          <button className="button button-quiet" onClick={() => ask("requestChanges")}>
            Pedir mudanças
          </button>
        </>
      )}
      <button className="button button-quiet" onClick={() => openUrl(pr.url)}>
        Abrir no GitHub
      </button>
      {blocker && <p className="muted pr-blocker">{blocker}</p>}
    </div>
  );
}

const VERDICT: Record<string, { label: string; className: string }> = {
  pronto: { label: "Pronto", className: "verdict-ok" },
  ajustes: { label: "Precisa de ajustes", className: "verdict-warn" },
  arriscado: { label: "Arriscado", className: "verdict-danger" },
};

function PrAnalysis({ pr, analyses, now }: { pr: PullRequest; analyses: PrAnalyses; now: number }) {
  const [analyzer, setAnalyzer] = useAnalyzer();
  const key = prKey(pr);
  const saved = analyses.saved[key];
  const running = analyses.running[key];
  const error = analyses.errors[key];
  const outdated = saved && pr.headSha && saved.headSha !== pr.headSha;

  return (
    <div className="pr-detail-section pr-analysis">
      <h4>Análise</h4>
      {saved && (
        <div className="analysis-result">
          <p className="pr-detail-line">
            {saved.verdict && (
              <span className={`verdict ${VERDICT[saved.verdict].className}`}>{VERDICT[saved.verdict].label}</span>
            )}
            {outdated && <span className="pr-badge">desatualizada: o PR mudou depois</span>}
            <span className="muted-inline">
              {analyzerLabel(saved.analyzer)}, {elapsed(saved.createdAt, now)}
            </span>
          </p>
          <p className="pr-body">{saved.text}</p>
          {!saved.localRepo && (
            <p className="muted">O repositório não está neste computador, então só o diff foi lido.</p>
          )}
        </div>
      )}
      {running ? (
        <p className="analysis-running" role="status">
          <span className="status-dot status-busy" aria-hidden="true" />
          Analisando com {analyzerLabel(running.analyzer)}
          {now - running.startedAt >= 60_000 && ` há ${Math.floor((now - running.startedAt) / 60_000)} min`}… Pode levar
          alguns minutos; dá para trocar de aba.
        </p>
      ) : (
        <div className="analysis-controls">
          <AnalyzerPicker value={analyzer} onChange={setAnalyzer} />
          <button className="button button-primary" onClick={() => analyses.analyze(pr, analyzer)}>
            {saved ? "Analisar de novo" : "Analisar"}
          </button>
          <p className="muted analysis-note">
            {analyzer.agent === "claude"
              ? "Gasta do seu limite do Claude. Ele só lê o código, sem alterar nada."
              : "Usa o modelo configurado no opencode, só lendo o código."}
          </p>
        </div>
      )}
      {error && <p className="inline-error">{error}</p>}
    </div>
  );
}

function PrDetail({
  pr,
  mine,
  analyses,
  now,
  onClose,
  onDone,
}: {
  pr: PullRequest;
  mine: boolean;
  analyses: PrAnalyses;
  now: number;
  onClose: () => void;
  onDone: (message: string) => void;
}) {
  const [fullBody, setFullBody] = useState(false);
  const body = pr.body.trim();
  const longBody = body.length > BODY_PREVIEW;
  return (
    <aside className="pr-detail card" aria-label={`Detalhes de ${pr.repo}#${pr.number}`}>
      <div className="pr-detail-head">
        <div>
          <p className="pr-detail-repo">
            {pr.repo}#{pr.number}
          </p>
          <h3>{pr.title}</h3>
        </div>
        <button className="link-button" onClick={onClose} aria-label="Fechar detalhes">
          Fechar
        </button>
      </div>

      <p className="pr-detail-line">
        <strong>{pr.author}</strong> quer juntar <span className="commit-branch">{pr.head}</span> em{" "}
        <span className="commit-branch">{pr.base}</span>
        <span>· aberto {elapsed(pr.createdAt, now)}</span>
      </p>
      <p className="pr-detail-line">
        {pr.draft && <span className="pr-badge">rascunho</span>}
        {pr.reviewDecision && <span className="pr-badge">{REVIEW_LABEL[pr.reviewDecision] ?? pr.reviewDecision}</span>}
        <span className="diff-add">+{fmt.format(pr.additions)}</span>
        <span className="diff-del">−{fmt.format(pr.deletions)}</span>
        <span className="muted-inline">
          em {pr.changedFiles} {pr.changedFiles === 1 ? "arquivo" : "arquivos"}
        </span>
      </p>

      <PrActions pr={pr} mine={mine} onDone={onDone} />

      <PrAnalysis pr={pr} analyses={analyses} now={now} />

      {body && (
        <div className="pr-detail-section">
          <h4>Descrição</h4>
          <p className="pr-body">{fullBody || !longBody ? body : `${body.slice(0, BODY_PREVIEW)}…`}</p>
          {longBody && (
            <button className="link-button" onClick={() => setFullBody(!fullBody)}>
              {fullBody ? "Mostrar menos" : "Mostrar tudo"}
            </button>
          )}
        </div>
      )}

      {pr.checkRuns.length > 0 && (
        <div className="pr-detail-section">
          <h4>Checks</h4>
          <ul className="pr-checks">
            {pr.checkRuns.map((c, i) => (
              <li key={`${c.name}-${i}`}>
                <span className={`check check-${c.outcome}`} />
                {c.name}
              </li>
            ))}
          </ul>
        </div>
      )}

      {pr.files.length > 0 && (
        <div className="pr-detail-section">
          <h4>Arquivos alterados</h4>
          <ul className="pr-files">
            {pr.files.map((f) => (
              <li key={f.path}>
                <span className="pr-file-path" title={f.path}>
                  {f.path}
                </span>
                <span className="diff-add">+{f.additions}</span>
                <span className="diff-del">−{f.deletions}</span>
              </li>
            ))}
          </ul>
          {pr.files.length < pr.changedFiles && (
            <p className="muted">E mais {pr.changedFiles - pr.files.length}, só no GitHub.</p>
          )}
        </div>
      )}
    </aside>
  );
}

/** Listas de PRs abertos à esquerda; o PR escolhido abre num painel à direita. */
export function PullRequests({ state, now }: { state: PullRequestsState; now: number }) {
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const { data } = state;

  async function refresh() {
    setBusy(true);
    await state.refresh();
    setBusy(false);
  }

  const all = data ? [...data.reviewRequested, ...data.mine] : [];
  const current = all.find((pr) => prKey(pr) === selected) ?? null;
  const currentIsMine = !!data && data.mine.some((pr) => prKey(pr) === selected);

  function done(message: string) {
    setNotice(message);
    // O PR some da lista depois do merge; a revisão muda a situação dele.
    state.refresh();
  }

  return (
    <div className="prs">
      <div className="activity-source prs-source">
        <span>
          {state.fetchedAt ? <>PRs abertos, atualizados {elapsed(state.fetchedAt, now)}</> : "Buscando os PRs no GitHub…"}
          {state.error && <span className="inline-error"> Última consulta falhou: {state.error}</span>}
        </span>
        <button className="link-button" onClick={refresh} disabled={busy}>
          {busy ? "Atualizando…" : "Atualizar"}
        </button>
      </div>
      {notice && (
        <p className="message pr-notice" role="status">
          {notice}
        </p>
      )}
      {data && (
        <div className={`prs-layout${current ? " with-detail" : ""}`}>
          <div className="prs-lists">
            <PrGroup
              title="Esperando sua revisão"
              empty="Ninguém pediu a sua revisão agora."
              prs={data.reviewRequested}
              showAuthor
              selected={selected}
              onSelect={(key) => {
                setSelected(key);
                setNotice(null);
              }}
              now={now}
            />
            <PrGroup
              title="Seus PRs abertos"
              empty="Você não tem PRs abertos."
              prs={data.mine}
              showAuthor={false}
              selected={selected}
              onSelect={(key) => {
                setSelected(key);
                setNotice(null);
              }}
              now={now}
            />
          </div>
          {current && (
            <PrDetail
              key={selected}
              pr={current}
              mine={currentIsMine}
              analyses={state.analyses}
              now={now}
              onClose={() => setSelected(null)}
              onDone={done}
            />
          )}
        </div>
      )}
    </div>
  );
}
