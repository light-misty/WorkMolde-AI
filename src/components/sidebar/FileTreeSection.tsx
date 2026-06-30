import { useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from 'react-i18next';
import { useFileTreeStore } from "../../stores/useFileTreeStore";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { Icon } from "../common/Icon";
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
          role="treeitem"
          aria-expanded={isExpanded}
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
<span className="ft-chevron" style={{ transform: isExpanded ? "rotate(0deg)" : "rotate(-90deg)" }}>
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
      role="treeitem"
      aria-selected={isSelected}
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
  const { t } = useTranslation();
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
            {type === "file" ? t('fileTree.newFileTitle') : t('fileTree.newFolderTitle')}
          </span>
        </div>
        <div className="ni-body">
          <input
            className="ni-input"
            placeholder={type === "file" ? t('fileTree.fileNamePlaceholder') : t('fileTree.folderNamePlaceholder')}
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
            {t('fileTree.cancel')}
          </button>
          <button
            className="ni-btn ni-btn-confirm"
            onClick={handleSubmit}
            disabled={!value.trim() || creating}
          >
            {creating ? t('fileTree.creating') : t('fileTree.create')}
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
          background: var(--color-overlay);
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
          box-shadow: var(--shadow-lg);
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
          background: var(--color-accent-bg);
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
          color: var(--color-error);
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
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .ni-btn-cancel:hover {
          background: var(--color-bg-sub);
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
export function FileTreeSection({ onOpenPreview, onOpenVersionHistory }: { onOpenPreview?: (filePath: string, fileName: string) => void; onOpenVersionHistory?: (filePath: string, fileName: string) => void }) {
  const { t } = useTranslation();
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
          label: t('fileTree.newFile'),
          icon: "file-plus",
          onClick: () => setNewItemState({ type: "file", parentPath: node.path }),
        },
        {
          label: t('fileTree.newFolder'),
          icon: "folder-plus",
          onClick: () => setNewItemState({ type: "directory", parentPath: node.path }),
        },
        { label: "", separator: true, onClick: () => {} },
        {
          label: t('fileTree.rename'),
          icon: "edit",
          onClick: () => setRenamingPath(node.path),
        },
        {
          label: t('fileTree.delete'),
          icon: "trash",
          danger: true,
          onClick: () => setDeleteTarget({ name: node.name, path: node.path, isDir: true }),
        },
        { label: "", separator: true, onClick: () => {} },
        {
          label: t('fileTree.copyPath'),
          icon: "copy",
          onClick: () => handleCopyPath(node.path),
        },
        {
          label: t('fileTree.showInExplorer'),
          icon: "external-link",
          onClick: () => tauriCmd.showInFileManager(activeWorkspaceId, node.path),
        },
      ];
    }

    /* 文件菜单 */
    return [
      {
        label: t('fileTree.openPreview'),
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
      {
        label: t('fileTree.versionHistory'),
        icon: "clock",
        onClick: () => onOpenVersionHistory?.(node.path, node.name),
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: t('fileTree.rename'),
        icon: "edit",
        onClick: () => setRenamingPath(node.path),
      },
      {
        label: t('fileTree.delete'),
        icon: "trash",
        danger: true,
        onClick: () => setDeleteTarget({ name: node.name, path: node.path, isDir: false }),
      },
      { label: "", separator: true, onClick: () => {} },
      {
        label: t('fileTree.copyPath'),
        icon: "copy",
        onClick: () => handleCopyPath(node.path),
      },
      {
        label: t('fileTree.showInExplorer'),
        icon: "external-link",
        onClick: () => tauriCmd.showInFileManager(activeWorkspaceId, node.path),
      },
    ];
  }, [contextMenu, activeWorkspaceId, handleCopyPath, onOpenPreview, onOpenVersionHistory]);

  return (
    <div className="ft-section">
      {/* 搜索栏 */}
      <div className="ft-search">
        <Icon name="search" size={14} className="ft-search-icon" />
        <input
          type="text"
          className="ft-search-input"
          placeholder={t('fileTree.searchPlaceholder')}
          aria-label={t('fileTree.searchFile')}
          value={searchKeyword}
          onChange={(e) => setSearchKeyword(e.target.value)}
        />
        <button
          className={`ft-refresh ${isLoading ? "ft-refreshing" : ""}`}
          onClick={handleRefresh}
          title={t('fileTree.refreshTree')}
          aria-label={t('fileTree.refreshTree')}
          disabled={isLoading}
        >
          <Icon name="refresh" size={13} />
        </button>
      </div>

      {/* 文件树内容 */}
      {isLoading ? (
        <div className="ft-skeleton" role="status" aria-label={t('fileTree.loading')}>
          {[1, 2, 3, 4, 5].map((i) => (
            <div key={i} className="ft-skeleton-item">
              <div className="skeleton skeleton-circle" style={{ width: 16, height: 16, flexShrink: 0 }} />
              <div className={`skeleton skeleton-text ${i === 3 ? "skeleton-text-short" : ""}`} />
            </div>
          ))}
        </div>
      ) : filteredTree.length === 0 ? (
        <div className="ft-empty" role="status">
          <Icon name="file" size={20} className="ft-empty-icon" />
          <span className="ft-empty-text">
            {searchKeyword ? t('fileTree.noMatch') : t('fileTree.noFiles')}
          </span>
        </div>
      ) : (
        <div className="ft-tree" role="tree" aria-label={t('fileTree.treeLabel')}>
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
      {contextMenu && createPortal(
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenuItems()}
          onClose={() => setContextMenu(null)}
        />,
        document.body
      )}

      {/* 删除确认对话框 */}
      {deleteTarget && createPortal(
        <DeleteConfirmDialog
          name={deleteTarget.name}
          isDir={deleteTarget.isDir}
          onConfirm={handleDeleteConfirm}
          onCancel={() => setDeleteTarget(null)}
        />,
        document.body
      )}

      {/* 新建文件/文件夹输入弹窗 */}
      {newItemState && activeWorkspaceId && createPortal(
        <NewItemInput
          type={newItemState.type}
          parentPath={newItemState.parentPath}
          workspaceId={activeWorkspaceId}
          onCreated={handleNewItemCreated}
          onCancel={() => setNewItemState(null)}
        />,
        document.body
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
          font-size: 13px;
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
          opacity: 0.4;
        }
        .ft-empty-text {
          font-size: 13px;
          color: var(--color-text-quaternary);
        }
        .ft-tree {
          font-size: 13px;
        }
        .ft-item {
          display: flex;
          align-items: center;
          gap: 7px;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: all 0.15s;
          color: var(--color-text-primary);
          position: relative;
        }
        .ft-item:hover {
          background: var(--color-accent-bg);
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
          background: var(--color-accent-bg);
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
        .ft-ext-docx { color: var(--color-ext-docx, #2b579a); }
        .ft-ext-xlsx { color: var(--color-ext-xlsx, #217346); }
        .ft-ext-pptx { color: var(--color-ext-pptx, #b7472a); }
        .ft-ext-pdf { color: var(--color-ext-pdf, #ea4335); }
        .ft-ext-default { color: var(--color-text-tertiary); }
        .ft-name {
          flex: 1;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          font-size: 13px;
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
          font-size: 13px;
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
        .ft-skeleton {
          display: flex;
          flex-direction: column;
          gap: 4px;
          padding: 4px 0;
        }
        .ft-skeleton-item {
          display: flex;
          align-items: center;
          gap: 7px;
          padding: 4px 8px;
        }
        .ft-section {
          display: flex;
          flex-direction: column;
          flex: 1;
          min-height: 0;
          padding: 0 12px 12px;
        }
      `}</style>
    </div>
  );
}
