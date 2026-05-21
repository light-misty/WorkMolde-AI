import { useFileTreeStore } from "../../stores/useFileTreeStore";
import { Icon } from "../common/Icon";
import { SidebarSection } from "../layout/Sidebar";
import type { FileNode } from "../../types";

function FileTreeItem({ node, depth = 0 }: { node: FileNode; depth?: number }) {
  const { expandedKeys, selectedKey, toggleNode, selectNode } = useFileTreeStore();
  const isExpanded = expandedKeys.has(node.path);
  const isSelected = selectedKey === node.path;

  if (node.isDir) {
    return (
      <div>
        <div
          className="ft-item ft-dir group"
          onClick={() => toggleNode(node.path)}
        >
          <span className={`ft-dir-icon ${isExpanded ? "ft-dir-open" : ""}`}>
            <Icon name="folder" size={15} />
          </span>
          <span className="ft-name">{node.name}</span>
          <span className="ft-chevron" style={{ transform: isExpanded ? "rotate(90deg)" : "rotate(0deg)" }}>
            <Icon name="chevron-down" size={12} />
          </span>
        </div>
        {isExpanded && node.children && (
          <div className="ft-indent">
            {node.children.map((child) => (
              <FileTreeItem key={child.path} node={child} depth={depth + 1} />
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
    >
      <span className={`ft-file-icon ${extColorClass}`}>
        {extIcon}
      </span>
      <span className="ft-name">{node.name}</span>
    </div>
  );
}

export function FileTreeSection() {
  const { searchKeyword, setSearchKeyword, getFilteredTree, loadTree, isLoading, activeWorkspaceId } = useFileTreeStore();
  const filteredTree = getFilteredTree();

  const handleRefresh = () => {
    if (activeWorkspaceId) {
      loadTree(activeWorkspaceId);
    }
  };

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
            <FileTreeItem key={node.path} node={node} />
          ))}
        </div>
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
      `}</style>
    </SidebarSection>
  );
}
