import { Bell, Boxes, CloudCog, GitBranch } from "lucide-react";
import type { DraftRecord, SkillletRecord, EditFormData, PageDef } from "../types/domain";

export const KIND_OPTIONS = ["rule", "skilllet", "observation", "preference", "constraint", "procedure", "convention"];
export const SCOPE_OPTIONS = ["project", "global", "agent"];
export const EDITABLE_AGENTS = ["codex", "claude-code"];

export const PAGES: PageDef[] = [
  { id: "drafts", label: "审阅", icon: Bell },
  { id: "skilllets", label: "技能片段", icon: Boxes },
  { id: "agents", label: "分配", icon: GitBranch },
  { id: "settings", label: "设置", icon: CloudCog },
];

export function buildEditFormFromDraft(draft: DraftRecord): EditFormData {
  return {
    title: draft.title,
    brief: draft.brief ?? "",
    body: draft.body,
    kind: draft.kind,
    scope: draft.scope,
    tagsInput: (draft.tags ?? []).join("，"),
    targets: [...(draft.targets ?? [])],
  };
}

export function buildEditFormFromSkilllet(skilllet: SkillletRecord): EditFormData {
  return {
    title: skilllet.title,
    brief: skilllet.brief ?? "",
    body: skilllet.body,
    kind: skilllet.kind,
    scope: skilllet.scope,
    tagsInput: (skilllet.tags ?? []).join("，"),
    targets: [],
  };
}

export function parseCommaTags(input: string): string[] {
  return input
    .split(/[,，]/)
    .map((t) => t.trim())
    .filter(Boolean);
}
