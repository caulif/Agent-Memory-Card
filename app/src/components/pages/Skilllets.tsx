import React from "react";
import { ArrowUp, PackagePlus, Pencil } from "lucide-react";
import { ActionButton, EmptyState } from "../common";
import {
  buildEditFormFromSkilllet,
  filterRecordsByTag,
  translateKind,
  translateScope,
  type ProjectAction,
  type ProjectSkillletLibrary,
  type ProjectSnapshot,
  type PanelPageProps,
} from "../../ui-helpers";
import { RecordEditor } from "./RecordEditor";

export function Skilllets({
  snapshot,
  library,
  pendingAction,
  disabled,
  onAction,
  previewMode,
  projectPath,
  onRefresh,
  onFusionStarted,
}: PanelPageProps & {
  library: ProjectSkillletLibrary | null;
  previewMode: boolean;
  projectPath: string;
  onRefresh: () => void;
  onFusionStarted: () => void;
}) {
  const [activeTag, setActiveTag] = React.useState("all");
  const [selected, setSelected] = React.useState<string[]>([]);
  const [editingId, setEditingId] = React.useState<string | null>(null);

  function startEdit(skilllet: import("../../ui-helpers").SkillletRecord) {
    setEditingId(skilllet.id);
  }

  function cancelEdit() {
    setEditingId(null);
  }

  async function handleSaved() {
    setEditingId(null);
    onRefresh();
  }

  const projectSkilllets = library?.skilllets ?? snapshot?.skilllets ?? [];
  const allSkilllets = projectSkilllets;

  /** 从所有技能片段中收集唯一标签 */
  const allTags = React.useMemo(() => {
    const tagSet = new Set<string>();
    for (const s of allSkilllets) {
      for (const t of s.tags ?? []) {
        tagSet.add(t);
      }
    }
    return Array.from(tagSet).sort();
  }, [allSkilllets]);

  const filtered = React.useMemo(
    () => filterRecordsByTag(allSkilllets, activeTag),
    [allSkilllets, activeTag],
  );
  const filteredProject = React.useMemo(
    () => filterRecordsByTag(projectSkilllets, activeTag),
    [projectSkilllets, activeTag],
  );
  const canFuse = selected.length >= 2;

  function toggleSelected(id: string) {
    setSelected((current) => (current.includes(id) ? current.filter((item) => item !== id) : [id, ...current]));
  }

  function fuseSelected() {
    if (!canFuse) return;
    const selectedSkilllets = projectSkilllets.filter((skilllet) => selected.includes(skilllet.id));
    const title = selectedSkilllets.length > 0 ? `${selectedSkilllets[0]!.title} Fusion` : "Skilllet Fusion";
    const slug = title
      .toLowerCase()
      .replace(/[^a-z0-9\u4e00-\u9fff]+/gi, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 48);
    onAction("融合Skilllet", "已生成融合草稿", "fuse_skilllets_to_draft", {
      input: {
        id: `project:${slug || "skilllet-fusion"}`,
        title,
        sources: selected,
        targets: ["codex", "claude-code"],
      },
    }).then(() => {
      setSelected([]);
      onFusionStarted();
    });
  }

  return (
    <div className="list">
      {allSkilllets.length === 0 ? (
        <EmptyState title="暂无技能片段" description="批准草稿后，当前项目内可复用的技能片段会显示在这里。" />
      ) : (
        <>
          {/* 标签快速筛选 */}
          {allTags.length > 0 ? (
            <nav className="tag-filter" aria-label="按标签筛选技能片段">
              <button
                className={activeTag === "all" ? "active" : ""}
                onClick={() => setActiveTag("all")}
              >
                全部
              </button>
              {allTags.map((tag) => (
                <button
                  key={tag}
                  className={activeTag === tag ? "active" : ""}
                  onClick={() => setActiveTag(tag)}
                >
                  {tag}
                </button>
              ))}
            </nav>
          ) : null}

          <div className="fusion-toolbar">
            <div>
              <strong>Skilllet 融合</strong>
              <span>{selected.length >= 2 ? `已选择 ${selected.length} 个片段，可生成待审融合草稿。` : "选择至少 2 个项目 Skilllet 后生成融合草稿。"}</span>
            </div>
            <ActionButton
              className="secondary-action"
              icon={PackagePlus}
              label="生成融合草稿"
              busyLabel="融合中"
              busy={pendingAction === "融合Skilllet"}
              disabled={disabled || !canFuse}
              onClick={fuseSelected}
            />
          </div>

          {filtered.length === 0 ? (
            <p className="filter-note">当前标签筛选条件下暂无匹配的技能片段。</p>
          ) : null}

          {/* 项目 Skilllets */}
          {filteredProject.length > 0 ? (
            <section className="skilllet-section">
              <div className="section-label">项目技能片段</div>
              {filteredProject.map((skilllet) => {
                const promoteKey = `提升-${skilllet.id}`;
                return (
                <React.Fragment key={skilllet.id}>
                <article className={`record compact selectable-record ${selected.includes(skilllet.id) ? "selected" : ""}`}>
                  <button
                    className="select-chip"
                    onClick={() => toggleSelected(skilllet.id)}
                    aria-pressed={selected.includes(skilllet.id)}
                  >
                    {selected.includes(skilllet.id) ? "已选" : "选择"}
                  </button>
                  <div className="record-main">
                    <span className="tag">{translateKind(skilllet.kind)} · {translateScope(skilllet.scope)}</span>
                    {skilllet.brief ? <p className="draft-brief">{skilllet.brief}</p> : null}
                    <h3>{skilllet.title}</h3>
                    <p>{skilllet.body}</p>
                    {skilllet.tags && skilllet.tags.length > 0 ? (
                      <div className="tag-row">
                        {skilllet.tags.map((t) => (
                          <span key={t}>{t}</span>
                        ))}
                      </div>
                    ) : null}
                  </div>
                  <div className="record-actions">
                    <button
                      className="secondary-action"
                      disabled={disabled}
                      onClick={() => startEdit(skilllet)}
                    >
                      <Pencil size={15} />
                      编辑
                    </button>
                    <ActionButton
                      icon={ArrowUp}
                      label="提升到全局"
                      busyLabel="提升中"
                      busy={pendingAction === promoteKey}
                      disabled={disabled}
                      onClick={() => onAction(promoteKey, `已将"${skilllet.title}"提升到全局 Skilllet Library`, "promote_skilllet_to_global", { id: skilllet.id })}
                    />
                  </div>
                </article>
                {editingId === skilllet.id ? (
                  <RecordEditor
                    recordType="skilllet"
                    initialForm={buildEditFormFromSkilllet(skilllet)}
                    previewMode={previewMode}
                    projectPath={projectPath}
                    recordId={skilllet.id}
                    onSaved={handleSaved}
                    onCancel={cancelEdit}
                  />
                ) : null}
                </React.Fragment>
                );
              })}
            </section>
          ) : null}

        </>
      )}
    </div>
  );
}
