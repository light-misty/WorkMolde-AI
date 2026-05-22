import { useState, useCallback } from "react";
import { useFileTreeStore } from "../../stores/useFileTreeStore";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { Icon } from "../common/Icon";
import { SidebarSection } from "../layout/Sidebar";
import { ContextMenu, type ContextMenuItem } from "../common/ContextMenu";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import * as tauriCmd from "../../services/tauri";
import type { FileNode } from "../../types";

/* ---- 内联重命名输入框 ---- */
function InlineRenameInput({
  defaultValue,
  onConfirm,
  onCancel,
}: {
  defaultValue: string;
  onConfirm: (newName: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(defaultValue);

  /* 提交重命名 */
  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== defaultValue) {
      onConfirm(trimmed);
    } else {
      onCancel();
    }
  }, [value, defaultValue, onConfirm, onCancel]);

  /* 键盘事件 */
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSubmit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    },
    [handleSubmit, onCancel],
  );

  return (
    <input
      ref={(el) => {
        if (el) {
          /* 选中文件名（不含扩展名） */
          const dotIdx = defaultValue.lastIndexOf(".");
          el.setSelectionRange(0, dotIdx > 0 ? dotIdx : defaultValue.length);
          el.focus();
        }
      }}
      className="ft-rename-input"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={handleSubmit}
      onKeyDown={handleKeyDown}
      onClick={(e) => e.stopPropagation()}
    />
  );
}

/* ---- 文件树节点组件 ---- */
function FileTreeItem({
  node,
  depth = 0,
  renamingPath,
  onRenameConfirm,
  onRenameCancel,
  onContextMenu,
  onDoubleClickFile,
}: {
  node: FileNode;
  depth?: number;
  renamingPath: string | null;
  onRenameConfirm: (oldPath: string, newName: string) => void;
  onRenameCancel: () => void;
  onContextMenu: (e: React.MouseEvent, node: FileNode) => void;
  onDoubleClickFile?: (filePath: string, fileName: string) => void;
}) {
  const { expandedKeys, selectedKey, toggleNode, selectNode } = useFileTreeStore();
  const isExpanded = expandedKeys.has(node.path);
  const isSelected = selectedKey === node.path;
  const isRenaming = renamingPath === node.path;

  if (node.isDir) {
    return (
      <div>
        <div
          className="ft-item ft-dir group"
          onClick={() => toggleNode(node.path)}
          onContextMenu={(e) => onContextMenu(e, node)}
        >
          <span className={`ft-dir-icon ${isExpanded ? "ft-dir-open" : ""}`}>
            <Icon name="folder" size={15} />
          </span>
          {isRenaming ? (
            <InlineRenameInput
              defaultValue={node.name}
              onConfirm={(newName) => onRenameConfirm(node.path, newName)}
              onCancel={onRenameCancel}
            />
          ) : (
            <span className="ft-name">{node.name}</span>
          )}
          <span className="ft-chevron" style={{ transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)" }}>
            <Icon name="chevron-down" size={12} />
          </span>
        </div>
        {isExpanded && node.children && (
          <div className="ft-indent">
            {node.children.map((child) => (
              <FileTreeItem
                key={child.path}
                node={child}
                depth={depth + 1}
                renamingPath={renamingPath}
                onRenameConfirm={onRenameConfirm}
                onRenameCancel={onRenameCancel}
                onContextMenu={onContextMenu}
                onDoubleClickFile={onDoubleClickFile}
              />
            ))}
          </div>
        )}
      </div>
    );
  }

  /* 文件类型颜色映射 */
  const extColorClass =
    node.extension === "docx" ? "ft-ext-docx" :
    node.extension === "xlsx" ? "ft-ext-xlsx" :
    node.extension === "pptx" ? "ft-ext-pptx" :
    node.extension === "pdf" ? "ft-ext-pdf" :
    "ft-ext-default";

  const extIcon =
    node.extension === "docx" ? <Icon name="doc" size={15} /> :
    node.extension === "xlsx" ? <Icon name="xlsx" size={15} /> :
    node.extension === "pptx" ? <Icon name="ppt" size={15} /> :
    node.extension === "pdf" ? <Icon name="pdf" size={15} /> :
    <Icon name="file" size={15} />;

  return (
    <div
      className={`ft-item ft-file ${isSelected ? "ft-selected" : ""}`}
      onClick={() => selectNode(node.path)}
      onDoubleClick={() => onDoubleClickFile?.(node.path, node.name)}
      onContextMenu={(e) => onContextMenu(e, node)}
    >
      <span className={`ft-file-icon ${extColorClass}`}>
        {extIcon}
      </span>
      {isRenaming ? (
        <InlineRenameInput
          defaultValue={node.name}
          onConfirm={(newName) => onRenameConfirm(node.path, newName)}
          onCancel={onRenameCancel}
        />
      ) : (
        <span className="ft-name">{node.name}</span>
      )}
    </div>
  );
}

/* ---- 新建输入弹窗 ---- */
function NewItemInput({
  type,
  parentPath,
  workspaceId,
  onCreated,
  onCancel,
}: {
  type: "file" | "directory";
  parentPath: string;
  workspaceId: string;
  onCreated: () => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const handleSubmit = useCallback(async () => {
    const name = value.trim();
    if (!name) return;

    const newPath = parentPath ? `${parentPath}/${name}` : name;
    setCreating(true);
    setError(null);

    try {
      if (type === "file") {
        await tauriCmd.createFile(workspaceId, newPath);
      } else {
        await tauriCmd.createDirectory(workspaceId, newPath);
      }
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setCreating(false);
    }
  }, [value, parentPath, workspaceId, type, onCreated]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSubmit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    },
    [handleSubmit, onCancel],
  );

  return (
    <div className="ni-overlay" onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}>
      <div className="ni-dialog">
        <div className="ni-header">
          <span className="ni-icon">
            <Icon name={type === "file" ? "file-plus" : "folder-plus"} size={18} />
          </span>
          <span className="ni-title">
            新建{type === "file" ? "文件" : "文件夹"}
          </span>
        </div>
        <div className="ni-body">
          <input
            className="ni-input"
            placeholder={type === "file" ? "输入文件名（含扩展名）" : "输入文件夹名称"}
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              setError(null);
            }}
            onKeyDown={handleKeyDown}
            autoFocus
          />
          {error && <p className="ni-error">{error}</p>}
        </div>
        <div className="ni-footer">
          <button className="ni-btn ni-btn-cancel" onClick={onCancel}>
            取消
          </button>
          <button
            className="ni-btn ni-btn-confirm"
            onClick={handleSubmit}
            disabled={!value.trim() || creating}
          >
            {creating ? "创建中..." : "创建"}
          </button>
        </div>
      </div>

      <style>{`
        .ni-overlay {
          position: fixed;
          inset: 0;
          z-index: 10001;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(0, 0, 0, 0.3);
          animation: ni-fade-in 0.15s ease-out;
        }
        @keyframes ni-fade-in {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        .ni-dialog {
          min-width: 340px;
          max-width: 420px;
          background: var(--color-bg-elevated, #fff);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-lg, 12px);
          box-shadow: 0 12px 32px rgba(0, 0, 0, 0.12), 0 4px 12px rgba(0, 0, 0, 0.06);
          padding: 20px;
          animation: ni-dialog-in 0.2s ease-out;
        }
        @keyframes ni-dialog-in {
          from {
            opacity: 0;
            transform: scale(0.95) translateY(-8px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .ni-header {
          display: flex;
          align-items: center;
          gap: 10px;
          margin-bottom: 14px;
        }
        .ni-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 32px;
          height: 32px;
          border-radius: 50%;
          background: rgba(51, 112, 255, 0.08);
          color: var(--color-accent);
          flex-shrink: 0;
        }
        .ni-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .ni-body {
          margin-bottom: 18px;
        }
        .ni-input {
          width: 100%;
          padding: 8px 12px;
          font-size: 13px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-sm, 4px);
          background: var(--color-bg);
          color: var(--color-text-primary);
          outline: none;
          transition: border-color 0.2s, box-shadow 0.2s;
          box-sizing: border-box;
        }
        .ni-input:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 3px var(--color-accent-lighter);
        }
        .ni-input::placeholder {
          color: var(--color-text-quaternary);
        }
        .ni-error {
          margin: 6px 0 0;
          font-size: 12px;
          color: var(--color-error, #e53e3e);
        }
        .ni-footer {
          display: flex;
          justify-content: flex-end;
          gap: 8px;
        }
        .ni-btn {
          padding: 6px 14px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm, 4px);
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .ni-btn-cancel {
          background: var(--color-bg-hover, rgba(0, 0, 0, 0.04));
          color: var(--color-text-secondary);
        }
        .ni-btn-cancel:hover {
          background: var(--color-bg-hover, rgba(0, 0, 0, 0.08));
        }
        .ni-btn-confirm {
          background: var(--color-accent);
          color: #fff;
        }
        .ni-btn-confirm:hover {
          opacity: 0.9;
        }
        .ni-btn-confirm:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}

/* ---- 主组件 ---- */
export function FileTreeSection({ onOpenPreview }: { onOpenPreview?: (filePath: string, fileName: string) => void }) {
  const { searchKeyword, setSearchKeyword, getFilteredTree, loadTree, isLoading, activeWorkspaceId } = useFileTreeStore();
  const { workspaces } = useWorkspaceStore();
  const filteredTree = getFilteredTree();

  /* 右键菜单状态 */
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    node: FileNode;
  } | null>(null);

  /* 重命名状态 */
  const [renamingPath, setRenamingPath] = useState<string | null>(null);

  /* 删除确认状态 */
  const [deleteTarget, setDeleteTarget] = useState<{
    name: string;
    path: string;
    isDir: boolean;
  } | null>(null);

  /* 新建文件/文件夹状态 */
  const [newItemState, setNewItemState] = useState<{
    type: "file" | "directory";
    parentPath: string;
  } | null>(null);

  /* 获取当前活动工作区 */
  const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId);

  /* 刷新文件树 */
  const handleRefresh = useCallback(() => {
    if (activeWorkspaceId) {
      loadTree(activeWorkspaceId);
    }
  }, [activeWorkspaceId, loadTree]);

  /* 复制路径到剪贴板 */
  const handleCopyPath = useCallback(async (nodePath: string) => {
    if (!activeWorkspace) return;
    const fullPath = activeWorkspace.path + "\\" + nodePath.replace(/\//g, "\\");
    try {
      await navigator.clipboard.writeText(fullPath);
    } catch {
      /* 降级方案：使用 textarea 复制 */
      const textarea = document.createElement("textarea");
      textarea.value = fullPath;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
    }
  }, [activeWorkspace]);

  /* 执行重命名 */
  const handleRenameConfirm = useCallback(async (oldPath: string, newName: string) => {
    if (!activeWorkspaceId) return;
    /* 计算新路径：替换最后一段路径名 */
    const lastSep = oldPath.lastIndexOf("/");
    const parentPart = lastSep >= 0 ? oldPath.substring(0, lastSep + 1) : "";
    const newPath = parentPart + newName;

    try {
      await tauriCmd.renameFile(activeWorkspaceId, oldPath, newPath);
      handleRefresh();
    } catch (err) {
      console.error("[FileTree] 重命名失败:", err);
    }
    setRenamingPath(null);
  }, [activeWorkspaceId, handleRefresh]);

  /* 取消重命名 */
  const handleRenameCancel = useCallback(() => {
    setRenamingPath(null);
  }, []);

  /* 执行删除 */
  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget || !activeWorkspaceId) return;
    try {
      await tauriCmd.deleteFile(activeWorkspaceId, deleteTarget.path);
      handleRefresh();
    } catch (err) {
      console.error("[FileTree] 删除失败:", err);
    }
    setDeleteTarget(null);
  }, [deleteTarget, activeWorkspaceId, handleRefresh]);

  /* 新建完成回调 */
  const handleNewItemCreated = useCallback(() => {
    setNewItemState(null);
    handleRefresh();
  }, [handleRefresh]);

  /* 右键菜单事件 */
  const handleContextMenu = useCallback((e: React.MouseEvent, node: FileNode) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY, node });
  }, []);

  /* 构建右键菜单项 */
  const contextMenuItems = useCallback((): ContextMenuItem[] => {
    if (!contextMenu || !activeWorkspaceId) return [];
    const node = contextMenu.node;

    if (node.isDir) {
      return [
        {
          label: "新建文件",
          icon: "file-plus",
          onClick: () => setNewItemState({ type: "file", parentPath: node.path }),
        },
        {
          label: "新建文件夹",
          icon: "folder-plus",
          onClick: () => setNewItemState({ type: "directory", parentPath: node.path }),
        },
        { label: "", separator: true, onClick: () => {} },
        {
          label: "重命名",
          icon: "edit",
          onClick: () => setRenamingPath(node.path),
        },
        {
          label: "删除",
          icon: "trash",
          danger: true,
          onClick: () => setDeleteTarget({ name: node.name, path: node.path, isDir: true }),
        },
        { label: "", separator: true, onClick: () => {} },
        {
          label: "复制路径",
          icon: "copy",
          onClick: () => handleCopyPath(node.path),
        },
        {
          label: "在资源管理器中显示",
          icon: "external-link",
          onClick: () => tauriCmd.showInFileManager(activeWorkspaceId, node.path),
        },
      ];
    }

    /* 文件菜单 */
    return [
      {
        label: "打开预览",
        icon: "eye",
        onClick: () => {
          if (onOpenPreview) {
            onOpenPreview(node.path, node.name);
          } else {
            const { selectNode } = useFileTreeStore.getState();
            selectNode(node.path);
          }
        },
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: "重命名",
        icon: "edit",
        onClick: () => setRenamingPath(node.path),
      },
      {
        label: "删除",
        icon: "trash",
        danger: true,
        onClick: () => setDeleteTarget({ name: node.name, path: node.path, isDir: false }),
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: "复制路径",
        icon: "copy",
        onClick: () => handleCopyPath(node.path),
      },
      {
        label: "在资源管理器中显示",
        icon: "external-link",
        onClick: () => tauriCmd.showInFileManager(activeWorkspaceId, node.path),
      },
    ];
  }, [contextMenu, activeWorkspaceId, handleCopyPath, onOpenPreview]);

  return (
    <SidebarSection title="工作区文件">
      {/* 搜索栏 */}
      <div className="ft-search">
        <Icon name="search" size={14} className="ft-search-icon" />
        <input
          type="text"
          className="ft-search-input"
          placeholder="搜索文件..."
          value={searchKeyword}
          onChange={(e) => setSearchKeyword(e.target.value)}
        />
        <button
          className={`ft-refresh ${isLoading ? "ft-refreshing" : ""}`}
          onClick={handleRefresh}
          title="刷新文件树"
          disabled={isLoading}
        >
          <Icon name="refresh" size={13} />
        </button>
      </div>

      {/* 文件树内容 */}
      {filteredTree.length === 0 ? (
        <div className="ft-empty">
          <div className="ft-empty-icon">
            <Icon name="file" size={20} />
          </div>
          <span className="ft-empty-text">
            {searchKeyword ? "未找到匹配文件" : "暂无文件"}
          </span>
        </div>
      ) : (
        <div className="ft-tree">
          {filteredTree.map((node) => (
            <FileTreeItem
              key={node.path}
              node={node}
              renamingPath={renamingPath}
              onRenameConfirm={handleRenameConfirm}
              onRenameCancel={handleRenameCancel}
              onContextMenu={handleContextMenu}
              onDoubleClickFile={onOpenPreview}
            />
          ))}
        </div>
      )}

      {/* 右键菜单 */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenuItems()}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* 删除确认对话框 */}
      {deleteTarget && (
        <DeleteConfirmDialog
          name={deleteTarget.name}
          isDir={deleteTarget.isDir}
          onConfirm={handleDeleteConfirm}
          onCancel={() => setDeleteTarget(null)}
        />
      )}

      {/* 新建文件/文件夹输入弹窗 */}
      {newItemState && activeWorkspaceId && (
        <NewItemInput
          type={newItemState.type}
          parentPath={newItemState.parentPath}
          workspaceId={activeWorkspaceId}
          onCreated={handleNewItemCreated}
          onCancel={() => setNewItemState(null)}
        />
      )}

      <style>{`
        .ft-search {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 6px 10px;
          margin-bottom: 8px;
          background: var(--color-bg);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          transition: all 0.2s;
        }
        .ft-search:focus-within {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 3px var(--color-accent-lighter);
        }
        .ft-search-icon {
          color: var(--color-text-quaternary);
          flex-shrink: 0;
          transition: color 0.2s;
        }
        .ft-search:focus-within .ft-search-icon {
          color: var(--color-accent);
        }
        .ft-search-input {
          flex: 1;
          font-size: 12px;
          color: var(--color-text-primary);
          background: transparent;
          border: none;
          outline: none;
        }
        .ft-search-input::placeholder {
          color: var(--color-text-quaternary);
        }
        .ft-refresh {
          width: 22px;
          height: 22px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-xs);
          color: var(--color-text-quaternary);
          transition: all 0.2s;
          flex-shrink: 0;
          border: none;
          background: none;
          cursor: pointer;
        }
        .ft-refresh:hover {
          color: var(--color-text-primary);
          background: var(--color-bg-hover);
        }
        .ft-refresh:disabled {
          opacity: 0.4;
          cursor: not-allowed;
        }
        .ft-refresh.ft-refreshing svg {
          animation: spin 0.8s linear infinite;
        }
        .ft-empty {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 8px;
          padding: 24px 16px;
        }
        .ft-empty-icon {
          width: 44px;
          height: 44px;
          border-radius: 50%;
          background: var(--color-bg);
          border: 1px solid var(--color-border-light);
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--color-text-quaternary);
        }
        .ft-empty-text {
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .ft-tree {
          font-size: 12px;
        }
        .ft-item {
          display: flex;
          align-items: center;
          gap: 7px;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: all 0.15s;
          color: var(--color-text-primary);
          position: relative;
        }
        .ft-item:hover {
          background: rgba(51, 112, 255, 0.04);
        }
        .ft-dir:hover {
          color: var(--color-accent);
        }
        .ft-dir:hover .ft-dir-icon {
          color: var(--color-accent);
        }
        .ft-dir-icon {
          color: var(--color-text-tertiary);
          transition: color 0.15s;
          display: flex;
          align-items: center;
          justify-content: center;
          width: 16px;
          height: 16px;
          flex-shrink: 0;
        }
        .ft-file:hover {
          background: rgba(51, 112, 255, 0.04);
        }
        .ft-selected {
          background: var(--color-accent-light) !important;
          color: var(--color-accent);
          font-weight: 500;
        }
        .ft-file-icon {
          width: 16px;
          height: 16px;
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          transition: color 0.15s;
        }
        .ft-ext-docx { color: #2b579a; }
        .ft-ext-xlsx { color: #217346; }
        .ft-ext-pptx { color: #b7472a; }
        .ft-ext-pdf { color: #ea4335; }
        .ft-ext-default { color: var(--color-text-tertiary); }
        .ft-name {
          flex: 1;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          font-size: 12px;
          line-height: 1.5;
        }
        .ft-chevron {
          width: 16px;
          height: 16px;
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--color-text-quaternary);
          transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .ft-indent {
          padding-left: 16px;
          margin-left: 8px;
          border-left: 1.5px solid var(--color-border-light);
        }
        .ft-rename-input {
          flex: 1;
          font-size: 12px;
          line-height: 1.5;
          padding: 1px 4px;
          border: 1px solid var(--color-accent);
          border-radius: var(--radius-xs);
          background: var(--color-bg);
          color: var(--color-text-primary);
          outline: none;
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          min-width: 0;
        }
      `}</style>
    </SidebarSection>
  );
}
