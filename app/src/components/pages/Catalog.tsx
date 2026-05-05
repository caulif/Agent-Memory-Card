import { Layers3, Store } from "lucide-react";
import { ActionButton, EmptyState, Panel } from "../common";
import { PACKAGE_MANAGER_EXPLANATION, type ProjectAction, type ProjectSkillletLibrary, type ProjectSnapshot, type PanelPageProps } from "../../ui-helpers";

export function Catalog({
  snapshot,
  library,
  pendingAction,
  disabled,
  onAction,
}: PanelPageProps & { library: ProjectSkillletLibrary | null }) {
  const items = library?.catalog_status.items ?? snapshot?.catalog_status.items ?? [];

  return (
    <div className="stack">
      <Panel title="包管理器" subtitle={PACKAGE_MANAGER_EXPLANATION} icon={Store}>
        <div className="package-help">
          <strong>它管理的不是 npm 依赖。</strong>
          <span>这里的“包”是一组可复用 Skilllet：例如 UI 设计规范、代码风格、审阅流程或 Agent 工作法。安装后可以继续编辑、融合，并分配给当前项目的 Claude Code / Codex。</span>
        </div>
      </Panel>

      <div className="package-grid">
        {items.length === 0 ? (
          <EmptyState title="暂无可安装包" description="当前项目还没有返回 Skilllet 包目录。请先选择项目或刷新索引。" />
        ) : (
          items.map((item) => (
            <article className="package-tile" key={item.package.id}>
              <div>
                <span className={`tag ${item.installed ? "installed" : ""}`}>{item.installed ? "已安装" : `版本 ${item.package.version}`}</span>
                <h3>{item.package.title}</h3>
                <p>{item.package.description}</p>
              </div>
              <div className="package-footer">
                <div className="tag-row">
                  {item.package.tags.slice(0, 3).map((tag) => (
                    <span key={tag}>{tag}</span>
                  ))}
                </div>
                <ActionButton
                  icon={Layers3}
                  label={item.installed ? "已安装" : "安装"}
                  busyLabel="安装中"
                  busy={pendingAction === `安装-${item.package.id}`}
                  disabled={disabled || item.installed}
                  onClick={() =>
                    onAction(`安装-${item.package.id}`, "包已安装", "install_catalog_package", {
                      packageId: item.package.id,
                      targets: ["codex", "claude-code"],
                    })
                  }
                />
              </div>
            </article>
          ))
        )}
      </div>
    </div>
  );
}
