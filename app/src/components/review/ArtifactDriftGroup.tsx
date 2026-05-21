import React from "react";
import { ArchiveRestore, FileInput, Trash2 } from "lucide-react";
import type { ProjectAction } from "../../types/domain";
import {
  paginateArtifactBlockingTargets,
  type ArtifactPreviewTarget,
} from "../../utils/review-workbench";

export function ArtifactDriftGroup({
  targets,
  disabled = false,
  pendingAction = "",
  onAction,
}: {
  targets: ArtifactPreviewTarget[];
  disabled?: boolean;
  pendingAction?: string;
  onAction?: ProjectAction;
}) {
  const [page, setPage] = React.useState(0);
  if (targets.length === 0) return null;

  const targetPage = paginateArtifactBlockingTargets(targets, page, 6);

  return (
    <div className="artifact-drift-group">
      <div className="artifact-drift-group-head">
        <strong>阻塞写入的 Drift 文件 · {targets.length}</strong>
        {targetPage.totalPages > 1 ? (
          <div>
            <button
              type="button"
              disabled={!targetPage.hasPrevious}
              onClick={() => setPage((current) => Math.max(0, current - 1))}
            >
              上一页
            </button>
            <span>
              {targetPage.page + 1}/{targetPage.totalPages}
            </span>
            <button
              type="button"
              disabled={!targetPage.hasNext}
              onClick={() => setPage((current) => current + 1)}
            >
              下一页
            </button>
          </div>
        ) : null}
      </div>
      {targetPage.items.map((target) => {
        const importKey = driftFileActionKey(target.path, "import");
        const keepKey = driftFileActionKey(target.path, "keep");
        const discardKey = driftFileActionKey(target.path, "discard");
        return (
          <span key={`blocking:${target.path}`} title={target.diffPreview?.join("\n") || target.path}>
            {target.status} · {target.path}
            {target.diffTruncated ? "（diff 已截断）" : ""}
            {target.diffPreview?.[0] ? <small>{target.diffPreview[0]}</small> : null}
            {onAction ? (
              <span className="artifact-drift-file-actions">
                <button
                  type="button"
                  disabled={disabled || pendingAction === importKey}
                  onClick={() =>
                    void onAction(importKey, "已把该 drift 文件导入为待审草稿。", "import_artifact_drift_path", {
                      artifactPath: target.path,
                    })
                  }
                  title="只把这个 drift 文件导入为草稿"
                >
                  <FileInput size={12} />
                  导入
                </button>
                <button
                  type="button"
                  disabled={disabled || pendingAction === keepKey}
                  onClick={() =>
                    void onAction(keepKey, "已接受该 drift 文件的当前内容。", "keep_artifact_drift_path", {
                      artifactPath: target.path,
                    })
                  }
                  title="只保留这个 drift 文件的当前内容"
                >
                  <ArchiveRestore size={12} />
                  保留
                </button>
                <button
                  type="button"
                  className="danger"
                  disabled={disabled || pendingAction === discardKey}
                  onClick={() => {
                    if (!window.confirm("这会丢弃该 drift 文件里的手动改动，并恢复为 Memory Card 生成内容。确定继续吗？")) return;
                    void onAction(discardKey, "已恢复该 drift 文件的生成内容。", "discard_artifact_drift_path", {
                      artifactPath: target.path,
                    });
                  }}
                  title="只丢弃这个 drift 文件的手动改动"
                >
                  <Trash2 size={12} />
                  丢弃
                </button>
              </span>
            ) : null}
          </span>
        );
      })}
      {targetPage.totalPages > 1 ? (
        <em>正在查看第 {targetPage.page + 1} 页，每页 {targetPage.pageSize} 个 drift 文件。</em>
      ) : null}
    </div>
  );
}

export function driftFileActionKey(
  path: string,
  action: "import" | "keep" | "discard",
) {
  return `drift:${action}:${path}`;
}
