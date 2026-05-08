import { Bell, Boxes, CloudCog, GitBranch } from "lucide-react";
import type { DraftRecord, MemoryCardRecord, EditFormData, PageDef } from "../types/domain";

export const KIND_OPTIONS = ["rule", "memory_card", "observation", "preference", "constraint", "procedure", "convention"];
export const SCOPE_OPTIONS = ["project", "global", "agent"];
export const EDITABLE_AGENTS = ["codex", "claude-code"];

export const PAGES: PageDef[] = [
  { id: "drafts", label: "审阅", icon: Bell },
  { id: "memory-cards", label: "记忆卡", icon: Boxes },
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

export function buildEditFormFromMemoryCard(memory_card: MemoryCardRecord): EditFormData {
  return {
    title: memory_card.title,
    brief: memory_card.brief ?? "",
    body: memory_card.body,
    kind: memory_card.kind,
    scope: memory_card.scope,
    tagsInput: (memory_card.tags ?? []).join("，"),
    targets: [],
  };
}

export function parseCommaTags(input: string): string[] {
  return input
    .split(/[,，]/)
    .map((t) => t.trim())
    .filter(Boolean);
}
