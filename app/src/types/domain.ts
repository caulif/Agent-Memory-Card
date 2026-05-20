import type React from "react";

// ===== 页面标识 =====
export type PageId = "drafts" | "memory-cards" | "skills" | "agents" | "settings";

// ===== 项目注册 =====
export type RegisteredProject = {
  name: string;
  path: string;
  markers: string[];
  agents: string[];
  last_seen: string;
};

export type DesktopAppState = {
  home: string;
  scan_roots: string[];
  registry: {
    version: number;
    projects: RegisteredProject[];
  };
};

// ===== 领域记录 =====
export type DraftRecord = {
  id: string;
  title: string;
  kind: string;
  scope: string;
  body: string;
  brief?: string;
  tags?: string[];
  language?: string;
  targets: string[];
  evidence: string;
  confidence?: number;
  reason?: string;
  matched_template?: string;
  extraction?: ExtractionMetadata;
  status: string;
};

export type CandidateRecord = {
  id: string;
  title: string;
  kind: string;
  scope: string;
  body: string;
  brief?: string;
  tags?: string[];
  language?: string;
  targets: string[];
  evidence: string;
  confidence?: number;
  reason?: string;
  matched_template?: string;
  source_observations?: string[];
  status: "candidate" | "hidden" | "rejected" | "promoted";
  extraction?: ExtractionMetadata;
  rejected_reason?: string;
  created_at?: string;
  updated_at?: string;
};

export type MemoryCardRecord = {
  id: string;
  title: string;
  kind: string;
  scope: string;
  body: string;
  brief?: string;
  tags?: string[];
  language?: string;
  activation?: string;
  source_project?: string;
  extraction?: ExtractionMetadata | null;
  approved_from?: string | null;
  evidence?: string | null;
  merge_history?: Array<{ source_id: string; merged_at: string; action: string }>;
};

export type CatalogItem = {
  installed: boolean;
  package: {
    id: string;
    title: string;
    description: string;
    version: string;
    tags: string[];
  };
};

// ===== 项目快照 =====
export type ProjectSnapshot = {
  project_path: string;
  candidates?: CandidateRecord[];
  drafts: DraftRecord[];
  memory_cards: MemoryCardRecord[];
  global_memory_cards?: MemoryCardRecord[];
  observations: Array<{ id: string; agent?: string; source_path: string }>;
  catalog_status: { items: CatalogItem[] };
  target_matrix: { agents: string[]; rows: Array<{ memory_card_id: string; title: string; scope?: string; targets: Record<string, boolean> }> };
  rule_ci: { passed: number; failed: number };
  build_preview: BuildPreview;
  status: { warnings: string[] };
};

// ===== Read Models =====
export type ProjectDashboard = {
  project_path: string;
  candidate_count: number;
  draft_count: number;
  memory_card_count: number;
  observation_count: number;
  global_memory_card_count: number;
  enabled_agents: string[];
  warning_count: number;
};

export type ProjectReviewInbox = {
  project_path: string;
  drafts: DraftRecord[];
};

export type ProjectCandidateInbox = {
  project_path: string;
  candidates: CandidateRecord[];
};

export type ProjectMemoryCardLibrary = {
  project_path: string;
  memory_cards: MemoryCardRecord[];
  global_memory_cards: MemoryCardRecord[];
  catalog_status: { items: CatalogItem[] };
};

export type ProjectSkillView = {
  id: string;
  name: string;
  description: string;
  source_path: string;
  source_kind: string;
  source_hash: string;
  warnings: string[];
  mirror_targets: string[];
  linked_memory_cards: MemoryCardRecord[];
  recommended_memory_cards: MemoryCardRecord[];
};

export type ProjectSkillLibrary = {
  project_path: string;
  generated_at: string;
  source_counts: Record<string, number>;
  skills: ProjectSkillView[];
};

export type ProjectAssignmentView = {
  project_path: string;
  enabled_agents: string[];
  target_matrix: { agents: string[]; rows: Array<{ memory_card_id: string; title: string; scope?: string; targets: Record<string, boolean> }> };
};

export type ProjectQualityView = {
  project_path: string;
  rule_ci: { passed: number; failed: number };
  build_preview: BuildPreview;
  status: { warnings: string[]; last_sync?: SyncCheckpoint | null };
};

export type SyncCheckpoint = {
  id: string;
  created_at: string;
  artifact_count: number;
  artifacts: Array<{ path: string; kind: string; hash: string }>;
  memory_card_ids: string[];
  rollback_instructions: string[];
};

export type ProjectEvalMetricView = {
  label: string;
  percent?: number | null;
  count: number;
  total: number;
  status: "pass" | "fail" | string;
};

export type ProjectEvalRunView = {
  project_path: string;
  status: "missing" | "passing" | "attention" | string;
  provider?: string | null;
  pipeline_version?: number | null;
  timestamp?: string | null;
  recall?: ProjectEvalMetricView | null;
  precision?: ProjectEvalMetricView | null;
  one_off_false_positive?: ProjectEvalMetricView | null;
  duplicate_cluster_risk?: ProjectEvalMetricView | null;
  evidence_validity?: ProjectEvalMetricView | null;
  provider_evidence_validity?: ProjectEvalMetricView | null;
  recommendations: string[];
};

export type BuildPreview = {
  actions: string[];
  warnings: string[];
  artifact_previews?: ArtifactPreviewRow[];
  verification?: SyncVerificationReport | null;
};

export type SyncVerificationReport = {
  rule_ci: { passed: number; failed: number; rows: Array<{ name: string; status: string; details: string[] }> };
  status: "pass" | "fail" | string;
  next_actions: string[];
  reload_prompt: string;
};

export type ArtifactPreviewRow = {
  agent: string;
  kind: string;
  path: string;
  status: "create" | "update" | "unchanged" | "drifted" | string;
  current_hash?: string | null;
  expected_hash: string;
  diff_preview: string[];
  diff_lines?: string[];
  diff_truncated?: boolean;
};

export type CustomProviderConfig = {
  enabled: boolean;
  protocol: "openai-compatible" | "anthropic-compatible";
  base_url: string;
  model: string;
  api_key_env: string;
  api_key?: string;
};

export type SetupChecklistItem = {
  label: string;
  status: "pass" | "warn" | "fail" | string;
  detail: string;
  next_action?: string | null;
};

export type SetupChecklistReport = {
  runtime_mode: "installer" | "dev" | string;
  items: SetupChecklistItem[];
};

export type ProviderStatusReport = {
  status: "pass" | "warn" | "fail" | string;
  provider: string;
  protocol: string;
  base_url: string;
  model: string;
  api_key_env: string;
  proxy?: string | null;
  checks: SetupChecklistItem[];
  next_actions: string[];
};

// ===== 任务相关 =====
export type DesktopJobStart = {
  job_id: string;
  key: string;
  stage: string;
  message: string;
  accepted: boolean;
};

export type TaskProgress = {
  label: string;
  percent: number;
  description: string;
};

export type DesktopTaskDetail = {
  label: string;
  description: string;
  percent: number;
};

export type DesktopTaskLogEntry = {
  timestamp: string;
  stage: string;
  description: string;
  percent: number;
};

export type DesktopJobReplay = {
  command: string;
  args: Record<string, unknown>;
};

export type DesktopTaskStatus = {
  job_id?: string;
  key: string;
  stage?: string;
  lifecycle?: string;
  label: string;
  description: string;
  percent: number;
  running: boolean;
  cancel_requested?: boolean;
  message: string;
  details?: DesktopTaskDetail[];
  logs?: DesktopTaskLogEntry[];
  started_at?: string | null;
  finished_at?: string | null;
  result_summary?: string | null;
  replay?: DesktopJobReplay | null;
};

// ===== Kernel Policy =====
export type KernelPolicyPayload = {
  mode: "manual" | "assisted" | "guarded-auto" | "agent-managed";
  auto_approve_min_confidence: number;
  allow_file_writes: boolean;
  require_review_for_high_risk: boolean;
};

export type KernelPlanRequest = {
  command: Record<string, unknown>;
  payload: Record<string, unknown>;
};

// ===== 提取分类 =====
/** 分类硬度元数据：描述提取结果的信号类型、激活方式、硬度等级等 */
export type KnowledgeClassification = {
  signal?: string;
  artifact_kind?: string;
  activation?: string;
  hardness?: string;
  control?: string;
  rationale?: string;
  tags?: string[];
};

// ===== 提取元数据 =====
export type ExtractionMetadata = {
  origin?: string;
  matched_signal?: string;
  reason?: string;
  source_observations?: string[];
  evidence_bundle?: EvidenceBundle | null;
  score_breakdown?: Record<string, number>;
  similar_record?: string | null;
  classification?: KnowledgeClassification | null;
  tags?: string[];
  suggested_action?: ExtractionAction | null;
  card_function?: "library" | "skill_targeted" | "workflow" | "merge" | string | null;
  value_claim?: string | null;
  value_delta?: ValueDelta | null;
  target_context?: TargetContext | null;
  synthesis_trace?: SynthesisTraceEntry[];
};

export type ValueDelta = {
  existing_behavior?: string;
  missing_part?: string;
  new_behavior?: string;
  why_not_duplicate?: string;
};

export type TargetContext = {
  target_type?: string;
  target_id?: string | null;
  why_this_target?: string;
};

export type SynthesisTraceEntry = {
  step: string;
  summary: string;
};

export type EvidenceBundle = {
  source_observation_ids?: string[];
  quotes?: Array<{ observation_id: string; text: string; role?: string; created_at?: string }>;
  context?: Array<{ observation_id: string; before?: string[]; after?: string[] }>;
  source_trust?: "user_direct" | "user_feedback" | "assistant_summary" | "tool_output" | "artifact" | "unknown";
  validity?: "valid" | "weak" | "invalid";
};

export type ExtractionAction = {
  action: string;
  route?: string;
  target_record?: string | null;
  compile_enabled?: boolean | null;
  record_id?: string | null;
  similarity?: number | null;
  reason?: string | null;
  rationale?: string | null;
};

// ===== 编辑表单 =====
export type EditFormData = {
  title: string;
  brief: string;
  body: string;
  kind: string;
  scope: string;
  tagsInput: string;
  targets: string[];
};

export type PlanReviewResult = {
  risk: string;
  disposition: string;
  reason: string;
  reviewRequired: boolean;
  requires_human_review?: boolean;
  decisionToken?: string;
};

// ===== 进化视图 =====
export type EvolutionInsight = {
  memory_card_id: string;
  title: string;
  confidence: number;
  stability: number;
  timeline: Array<{ date: string; event: string }>;
  activity: "active" | "dormant";
  conflicts: string[];
  promotion_candidate: boolean;
  evolution_tree: Array<{ from: string; to: string; label: string }>;
};

// ===== 共享组件类型 =====
/** 项目操作回调：各页面组件通过此类型调用 useProjectActions 返回的函数 */
export type ProjectAction = (
  actionKey: string,
  label: string,
  command: string,
  args?: Record<string, unknown>,
) => Promise<void>;

/** 页面面板通用 Props */
export type PanelPageProps = {
  snapshot: ProjectSnapshot | null;
  pendingAction: string;
  disabled: boolean;
  onAction: ProjectAction;
};

// ===== 候选筛选 =====
export type CandidateFilter = "high" | "all" | "low" | "rule" | "preference" | "procedure" | "constraint";

// ===== 导航页面定义 =====
export type PageDef = {
  id: PageId;
  label: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
};
