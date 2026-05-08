import type { CandidateRecord, ProjectSnapshot, ProjectDashboard, DesktopJobStart, CandidateFilter } from "../types/domain";

export const HIGH_CONFIDENCE_THRESHOLD = 0.72;

export function isHighConfidenceCandidate(candidate: CandidateRecord): boolean {
  return (candidate.confidence ?? 0) >= HIGH_CONFIDENCE_THRESHOLD;
}

export function sortCandidatesForInbox(candidates: CandidateRecord[]): CandidateRecord[] {
  return [...candidates].sort((a, b) => {
    const confidence = (b.confidence ?? 0) - (a.confidence ?? 0);
    if (confidence !== 0) return confidence;
    const template = Number(Boolean(b.matched_template)) - Number(Boolean(a.matched_template));
    if (template !== 0) return template;
    const bTime = Date.parse(b.updated_at ?? b.created_at ?? "") || 0;
    const aTime = Date.parse(a.updated_at ?? a.created_at ?? "") || 0;
    if (bTime !== aTime) return bTime - aTime;
    return a.id.localeCompare(b.id);
  });
}

export function filterCandidatesForInbox(candidates: CandidateRecord[], filter: CandidateFilter): CandidateRecord[] {
  const sorted = sortCandidatesForInbox(candidates);
  if (filter === "all") return sorted;
  if (filter === "high") return sorted.filter(isHighConfidenceCandidate);
  if (filter === "low") return sorted.filter((candidate) => !isHighConfidenceCandidate(candidate));
  return sorted.filter((candidate) => candidate.kind === filter);
}

export function projectOverviewMetrics(
  snapshot: ProjectSnapshot | null,
  dashboard: ProjectDashboard | null,
  installedCount: number,
) {
  return {
    candidateCount: snapshot?.candidates?.filter((candidate) => candidate.status === "candidate").length ?? dashboard?.candidate_count ?? 0,
    draftCount: snapshot?.drafts.length ?? dashboard?.draft_count ?? 0,
    memory_cardCount: snapshot?.memory_cards.length ?? dashboard?.memory_card_count ?? 0,
    observationCount: snapshot?.observations.length ?? dashboard?.observation_count ?? 0,
    installedCount,
  };
}

export function isDesktopJobStart(value: unknown): value is DesktopJobStart {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  return typeof record.job_id === "string" && typeof record.key === "string" && typeof record.message === "string";
}
