import { useState, useMemo } from "react";
import type { WorkflowNode, ToolNodeData } from "../../types";
import { useTranslation } from 'react-i18next';
import * as Diff from "diff";

interface ToolNodeProps {
  node: WorkflowNode<"tool">;
}

/** 无路径工具 → i18n key 映射 */
const toolDescriptions: Record<string, string> = {
  bash: 'toolBrief.runCommand',
  task: 'toolBrief.runTask',
  scratchpad: 'toolBrief.scratchpad',
  todowrite: 'toolBrief.todoWrite',
  question: 'toolBrief.question',
  web_search: 'toolBrief.webSearch',
  read_web: 'toolBrief.readWeb',
  webfetch: 'toolBrief.readWeb',
  search: 'toolBrief.searchFiles',
  glob: 'toolBrief.globFiles',
  grep: 'toolBrief.grepFiles',
};

/** 工具输入参数预览：用实际输入内容替代静态 i18n 描述 */
const toolInputPreview: Record<string, string> = {
  grep: 'pattern',
  glob: 'pattern',
  search: 'query',
  web_search: 'query',
  webfetch: 'url',
  read_web: 'url',
};

export function ToolNode({ node }: ToolNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ToolNodeData;
  const toolBriefInputKey = toolInputPreview[data.toolName];
  const toolBriefPreview = typeof toolBriefInputKey === 'string' && data.input?.[toolBriefInputKey] !== undefined
    ? String(data.input[toolBriefInputKey])
    : null;
  const hasError = data.success === false;
  // 判断工具是否正在执行中
  const isRunning = node.status === "running";
  const [errorExpanded, setErrorExpanded] = useState(false);
  // 控制命令输出详情的展开/收起
  const [outputExpanded, setOutputExpanded] = useState(false);

  // 错误文本：截断显示
  const errorText = data.error || "";
  const shouldTruncateError = errorText.length > 150;
  const displayError = shouldTruncateError && !errorExpanded
    ? errorText.slice(0, 150) + "..."
    : errorText;

  // bash 工具的命令和结果展示
  const isRunCommand = data.toolName === "bash";
  const command = isRunCommand ? String(data.input?.command ?? "") : "";
  const workingDir = isRunCommand ? String(data.input?.working_dir ?? "") : "";
  // 执行结果：stdout/stderr/exit_code
  const result = data.result as { stdout?: string; stderr?: string; exit_code?: number } | undefined;
  const stdout = result?.stdout ?? "";
  const stderr = result?.stderr ?? "";
  const exitCode = result?.exit_code;
  const hasOutput = stdout.length > 0 || stderr.length > 0;
  // 截断长输出（默认显示前 500 字符，展开后显示全部）
  const OUTPUT_TRUNCATE_LEN = 500;
  const shouldTruncateOutput = (stdout.length + stderr.length) > OUTPUT_TRUNCATE_LEN;
  const truncateOutput = (text: string) => {
    if (!shouldTruncateOutput || outputExpanded) return text;
    return text.length > OUTPUT_TRUNCATE_LEN
      ? text.slice(0, OUTPUT_TRUNCATE_LEN) + "..."
      : text;
  };

  return (
    <div className={`wf-node${isRunning ? " wf-tool-running" : ""}`}>
      <div className="wf-tool-content">
        {/* 工具名称和文件路径 */}
        <div className="wf-tool-brief">
          <span className="font-mono">{data.toolName}</span>
          {data.toolName === 'skill' && data.input?.name ? (
            <><span> · </span><span>{String(data.input.name)}</span></>
          ) : data.toolName === 'write_script' && data.input?.filename ? (
            <><span> · </span><span>{t('toolBrief.writeScript')} {String(data.input.filename)}</span></>
          ) : data.filePath ? (
            <><span> · </span><span>{data.filePath}</span></>
          ) : data.toolName === 'grep' && data.input?.pattern ? (
            <><span> · </span><span>{t('toolBrief.searchFor')} "{String(data.input.pattern)}"</span></>
          ) : toolBriefPreview ? (
            <><span> · </span><span>{toolBriefPreview}</span></>
          ) : toolDescriptions[data.toolName] ? (
            <><span> · </span><span>{t(toolDescriptions[data.toolName])}</span></>
          ) : null}
          {isRunning && (
            <span className="wf-tool-status-running">{t('toolNode.executing')}</span>
          )}
          {hasError && data.error && (
            <span className="wf-tool-error">
              {" — "}
              {displayError}
              {shouldTruncateError && (
                <button
                  className="wf-error-expand-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    setErrorExpanded(!errorExpanded);
                  }}
                >
                  {errorExpanded ? t('toolNode.collapseError') : t('toolNode.expandError')}
                </button>
              )}
            </span>
          )}
        </div>

        {/* bash 工具：合并卡片展示 Bash 标签、命令和执行结果 */}
        {isRunCommand && command && (
          <div className="wf-run-command-detail">
            {/* 卡片头部：Bash 标签 + 工作区 */}
            <div className="wf-run-command-header">
              <span className="wf-run-command-bash-label">Bash</span>
              {/* 工作目录（非默认时展示） */}
              {workingDir && (
                <span className="wf-run-command-cwd" title={workingDir}>
                  {t('toolBrief.document')}: {workingDir}
                </span>
              )}
            </div>
            {/* 命令展示行 */}
            <code className="wf-run-command-code">
              <span className="wf-run-command-prompt">$ </span>
              {command}
            </code>
            {/* 执行结果（完成后展示） */}
            {!isRunning && hasOutput && (
              <div className="wf-run-command-output-area">
                {/* 退出码标签 */}
                {exitCode !== undefined && (
                  <span
                    className={`wf-run-command-exit${exitCode === 0 ? " wf-exit-ok" : " wf-exit-err"}`}
                  >
                    {t('toolBrief.exitCode')}: {exitCode}
                  </span>
                )}
                {/* stdout 输出 */}
                {stdout && (
                  <pre className="wf-run-command-stdout">
                    <span className="wf-run-command-label">{t('toolBrief.output')}:</span>
                    {"\n"}
                    {truncateOutput(stdout)}
                  </pre>
                )}
                {/* stderr 错误输出 */}
                {stderr && (
                  <pre className="wf-run-command-stderr">
                    <span className="wf-run-command-label">{t('toolBrief.errorOutput')}:</span>
                    {"\n"}
                    {truncateOutput(stderr)}
                  </pre>
                )}
                {/* 长输出展开/收起按钮 */}
                {shouldTruncateOutput && (
                  <button
                    className="wf-error-expand-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      setOutputExpanded(!outputExpanded);
                    }}
                  >
                    {outputExpanded ? t('toolNode.collapseError') : t('toolNode.expandError')}
                  </button>
                )}
              </div>
            )}
            {/* 执行成功但无输出 */}
            {!isRunning && !hasOutput && data.success && (
              <div className="wf-run-command-output-area">
                <span className="wf-run-command-exit wf-exit-ok">
                  {t('toolBrief.exitCode')}: {exitCode ?? 0}
                </span>
                <span className="wf-run-command-noop">{t('toolBrief.noOutput')}</span>
              </div>
            )}
          </div>
        )}

        {/* edit 工具：内联差异对比卡片 */}
        {data.toolName === "edit" && data.input?.old_string !== undefined && data.input?.new_string !== undefined && data.success !== false && (
          <EditDiffCard
            oldContent={String(data.input.old_string)}
            newContent={String(data.input.new_string)}
          />
        )}
      </div>
    </div>
  );
}

function EditDiffCard({ oldContent, newContent }: { oldContent: string; newContent: string }) {
  const diffLines = useMemo(() => {
    const changes = Diff.diffLines(oldContent, newContent);
    const result: Array<{ type: "added" | "removed" | "unchanged"; content: string }> = [];
    for (const change of changes) {
      if (!change.value) continue;
      const lines = change.value.replace(/\n$/, "").split("\n");
      if (!change.added && !change.removed) {
        for (const line of lines) {
          result.push({ type: "unchanged", content: line });
        }
      } else if (change.removed) {
        for (const line of lines) {
          result.push({ type: "removed", content: line });
        }
      } else if (change.added) {
        for (const line of lines) {
          result.push({ type: "added", content: line });
        }
      }
    }
    return result;
  }, [oldContent, newContent]);

  const stats = useMemo(() => {
    let added = 0;
    let removed = 0;
    for (const line of diffLines) {
      if (line.type === "added") added++;
      if (line.type === "removed") removed++;
    }
    return { added, removed };
  }, [diffLines]);

  if (diffLines.length === 0) return null;

  return (
    <div className="wf-edit-diff-card">
      <div className="wf-edit-diff-header">
        <span className="wf-edit-diff-label">diff</span>
        <span className="wf-edit-diff-stats">
          {stats.added > 0 && <span className="wf-edit-diff-stat-added">+{stats.added}</span>}
          {stats.removed > 0 && <span className="wf-edit-diff-stat-removed">-{stats.removed}</span>}
        </span>
      </div>
      <div className="wf-edit-diff-content">
        {diffLines.map((line, i) => {
          const isAdded = line.type === "added";
          const isRemoved = line.type === "removed";
          return (
            <div
              key={i}
              className={
                isAdded ? "wf-edit-diff-line-added" :
                isRemoved ? "wf-edit-diff-line-removed" :
                "wf-edit-diff-line-unchanged"
              }
            >
              <span className="wf-edit-diff-marker">
                {isAdded ? "+" : isRemoved ? "-" : " "}
              </span>
              <span className="wf-edit-diff-text">{line.content}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
