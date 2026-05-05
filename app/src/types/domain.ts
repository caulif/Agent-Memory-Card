import type React from "react";

// ===== 页面标识 =====
export type PageId = "drafts" | "skilllets" | "agents" | "catalog" | "settings";

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

export type SkillletRecord = {
  id: string;
  title: string;
  kind: string;
  scope: string;
  body: string;
  brief?: string;
  tags?: string[];
  language?: string;
  source_project?: string;
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
  skilllets: SkillletRecord[];
  global_skilllets?: SkillletRecord[];
  observations: Array<{ id: string; agent?: string; source_path: string }>;
  catalog_status: { items: CatalogItem[] };
  target_matrix: { agents: string[]; rows: Array<{ skilllet_id: string; title: string; targets: Record<string, boolean> }> };
  rule_ci: { passed: number; failed: number };
  build_preview: { actions: string[]; warnings: string[] };
  status: { warnings: string[] };
};

// ===== Read Models =====
export type ProjectDashboard = {
  project_path: string;
  candidate_count: number;
  draft_count: number;
  skilllet_count: number;
  observation_count: number;
  global_skilllet_count: number;
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

export type ProjectSkillletLibrary = {
  project_path: string;
  skilllets: SkillletRecord[];
  global_skilllets: SkillletRecord[];
  catalog_status: { items: CatalogItem[] };
};

export type ProjectAssignmentView = {
  project_path: string;
  enabled_agents: string[];
  target_matrix: { agents: string[]; rows: Array<{ skilllet_id: string; title: string; targets: Record<string, boolean> }> };
};

export type ProjectQualityView = {
  project_path: string;
  rule_ci: { passed: number; failed: number };
  build_preview: { actions: string[]; warnings: string[] };
  status: { warnings: string[] };
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
  score_breakdown?: Record<string, number>;
  similar_record?: string | null;
  classification?: KnowledgeClassification | null;
  tags?: string[];
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
  skilllet_id: string;
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
