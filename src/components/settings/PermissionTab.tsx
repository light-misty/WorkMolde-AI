import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../common/Icon";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import {
  listPermissionRules,
  addPermissionRule,
  updatePermissionRule,
  deletePermissionRule,
} from "../../services/tauri";
import type {
  PermissionRule,
  PermissionScope,
  PermissionType,
  PermissionAction,
} from "../../types";

// 所有权限类型选项
const PERMISSION_TYPES: PermissionType[] = [
  'wildcard', 'read', 'edit', 'glob', 'grep', 'list',
  'bash', 'write_script', 'task', 'skill', 'lsp',
  'web_fetch', 'web_search', 'external_directory', 'doom_loop',
  'document', 'question',
];

const SCOPES: PermissionScope[] = ['global', 'project', 'session'];
const ACTIONS: PermissionAction[] = ['allow', 'deny', 'ask'];

/** 权限规则编辑器（内联弹窗） */
function PermissionRuleEditor({
  open,
  rule,
  onSave,
  onClose,
}: {
  open: boolean;
  rule: PermissionRule | null;
  onSave: (params: {
    scope: PermissionScope;
    permissionType: PermissionType;
    pattern: string;
    action: PermissionAction;
    description?: string;
    enabled?: boolean;
  }) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const isEdit = !!rule;

  const [scope, setScope] = useState<PermissionScope>('global');
  const [permissionType, setPermissionType] = useState<PermissionType>('bash');
  const [pattern, setPattern] = useState('');
  const [action, setAction] = useState<PermissionAction>('ask');
  const [description, setDescription] = useState('');
  const [enabled, setEnabled] = useState(true);

  // 打开时初始化字段
  useEffect(() => {
    if (!open) return;
    if (rule) {
      setScope(rule.scope);
      setPermissionType(rule.permissionType);
      setPattern(rule.pattern);
      setAction(rule.action);
      setDescription(rule.description);
      setEnabled(rule.enabled);
    } else {
      setScope('global');
      setPermissionType('bash');
      setPattern('');
      setAction('ask');
      setDescription('');
      setEnabled(true);
    }
  }, [open, rule]);

  if (!open) return null;

  const handleSave = () => {
    onSave({
      scope,
      permissionType,
      pattern: pattern.trim(),
      action,
      description: description.trim() || undefined,
      enabled: isEdit ? enabled : undefined,
    });
  };

  const canSave = pattern.trim().length > 0;

  return (
    <div className="perm-editor-overlay">
      <div className="perm-editor-dialog">
        <div className="perm-editor-header">
          <span className="perm-editor-title">
            {isEdit ? t('permission.editRule') : t('permission.addRule')}
          </span>
          <button className="perm-editor-close" onClick={onClose}>
            <Icon name="close" size={16} />
          </button>
        </div>
        <div className="perm-editor-body">
          <div className="perm-form-row">
            <label className="perm-form-label">{t('permission.scope')}</label>
            <select
              className="perm-form-select"
              value={scope}
              onChange={(e) => setScope(e.target.value as PermissionScope)}
            >
              {SCOPES.map((s) => (
                <option key={s} value={s}>{t(`permission.scopeOptions.${s}`)}</option>
              ))}
            </select>
          </div>
          <div className="perm-form-row">
            <label className="perm-form-label">{t('permission.type')}</label>
            <select
              className="perm-form-select"
              value={permissionType}
              onChange={(e) => setPermissionType(e.target.value as PermissionType)}
            >
              {PERMISSION_TYPES.map((tp) => (
                <option key={tp} value={tp}>{t(`permission.typeOptions.${tp}`)}</option>
              ))}
            </select>
          </div>
          <div className="perm-form-row">
            <label className="perm-form-label">{t('permission.pattern')}</label>
            <input
              className="perm-form-input"
              placeholder={t('permission.patternPlaceholder')}
              value={pattern}
              onChange={(e) => setPattern(e.target.value)}
            />
          </div>
          <div className="perm-form-row">
            <label className="perm-form-label">{t('permission.action')}</label>
            <select
              className="perm-form-select"
              value={action}
              onChange={(e) => setAction(e.target.value as PermissionAction)}
            >
              {ACTIONS.map((a) => (
                <option key={a} value={a}>{t(`permission.actionOptions.${a}`)}</option>
              ))}
            </select>
          </div>
          <div className="perm-form-row">
            <label className="perm-form-label">{t('permission.description')}</label>
            <textarea
              className="perm-form-textarea"
              placeholder={t('permission.descriptionPlaceholder')}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={2}
            />
          </div>
          {isEdit && (
            <div className="perm-form-row">
              <label className="perm-form-label">{t('permission.enabled')}</label>
              <label className="perm-toggle">
                <input
                  type="checkbox"
                  checked={enabled}
                  onChange={(e) => setEnabled(e.target.checked)}
                />
                <span className="perm-toggle-slider" />
              </label>
            </div>
          )}
        </div>
        <div className="perm-editor-footer">
          <button className="perm-btn perm-btn-cancel" onClick={onClose}>
            {t('permission.cancel')}
          </button>
          <button
            className="perm-btn perm-btn-primary"
            onClick={handleSave}
            disabled={!canSave}
          >
            {t('permission.save')}
          </button>
        </div>
      </div>
      <style>{`
        .perm-editor-overlay {
          position: fixed;
          inset: 0;
          z-index: 10002;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--color-overlay);
          animation: perm-fade-in 0.15s ease-out;
        }
        @keyframes perm-fade-in {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        .perm-editor-dialog {
          min-width: 420px;
          max-width: 520px;
          background: var(--color-bg-elevated, #fff);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-lg, 12px);
          box-shadow: var(--shadow-lg);
          animation: perm-dialog-in 0.2s ease-out;
        }
        @keyframes perm-dialog-in {
          from { opacity: 0; transform: scale(0.95) translateY(-8px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }
        .perm-editor-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 16px 20px;
          border-bottom: 1px solid var(--color-border-light);
        }
        .perm-editor-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .perm-editor-close {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-secondary);
          transition: all 0.15s;
        }
        .perm-editor-close:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .perm-editor-body {
          padding: 16px 20px;
          display: flex;
          flex-direction: column;
          gap: 12px;
        }
        .perm-form-row {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .perm-form-label {
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
        }
        .perm-form-select,
        .perm-form-input,
        .perm-form-textarea {
          width: 100%;
          padding: 6px 10px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-sm);
          font-size: 13px;
          color: var(--color-text-primary);
          background: var(--color-bg-elevated);
          outline: none;
          transition: border-color 0.15s;
          font-family: inherit;
        }
        .perm-form-select:focus,
        .perm-form-input:focus,
        .perm-form-textarea:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
        }
        .perm-form-textarea {
          resize: vertical;
          min-height: 40px;
        }
        .perm-toggle {
          position: relative;
          display: inline-block;
          width: 36px;
          height: 20px;
          cursor: pointer;
        }
        .perm-toggle input {
          opacity: 0;
          width: 0;
          height: 0;
        }
        .perm-toggle-slider {
          position: absolute;
          inset: 0;
          background: var(--color-bg-hover);
          border-radius: 10px;
          transition: 0.2s;
        }
        .perm-toggle-slider::before {
          content: "";
          position: absolute;
          width: 14px;
          height: 14px;
          left: 3px;
          top: 3px;
          background: white;
          border-radius: 50%;
          transition: 0.2s;
        }
        .perm-toggle input:checked + .perm-toggle-slider {
          background: var(--color-accent);
        }
        .perm-toggle input:checked + .perm-toggle-slider::before {
          transform: translateX(16px);
        }
        .perm-editor-footer {
          display: flex;
          justify-content: flex-end;
          gap: 8px;
          padding: 12px 20px;
          border-top: 1px solid var(--color-border-light);
        }
        .perm-btn {
          padding: 6px 16px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm);
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .perm-btn-cancel {
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .perm-btn-cancel:hover {
          background: var(--color-bg-sub);
        }
        .perm-btn-primary {
          background: var(--color-accent);
          color: white;
        }
        .perm-btn-primary:hover {
          background: var(--color-accent-hover);
        }
        .perm-btn-primary:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}

/** 动作标签颜色映射 */
function getActionClass(action: PermissionAction): string {
  switch (action) {
    case 'allow': return 'perm-action-allow';
    case 'deny': return 'perm-action-deny';
    case 'ask': return 'perm-action-ask';
  }
}

export function PermissionTab() {
  const { t } = useTranslation();
  const [allRules, setAllRules] = useState<PermissionRule[]>([]);
  const [userRules, setUserRules] = useState<PermissionRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<PermissionRule | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PermissionRule | null>(null);

  // 加载规则列表（并行获取全部规则和用户规则）
  const loadRules = async () => {
    setLoading(true);
    try {
      const [all, user] = await Promise.all([
        listPermissionRules(undefined, undefined, undefined, undefined, undefined, true),
        listPermissionRules(undefined, undefined, undefined, undefined, undefined, false),
      ]);
      setAllRules(all);
      setUserRules(user);
    } catch (e) {
      console.error('加载权限规则失败:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadRules();
  }, []);

  // 默认规则 = 全部规则前段（API 返回顺序为 [defaults..., user_rules...]）
  const defaultRules = allRules.slice(0, allRules.length - userRules.length);

  const handleAdd = () => {
    setEditingRule(null);
    setEditorOpen(true);
  };

  const handleEdit = (rule: PermissionRule) => {
    setEditingRule(rule);
    setEditorOpen(true);
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deletePermissionRule(deleteTarget.id);
      await loadRules();
    } catch (e) {
      console.error('删除权限规则失败:', e);
    }
    setDeleteTarget(null);
  };

  const handleSave = async (params: {
    scope: PermissionScope;
    permissionType: PermissionType;
    pattern: string;
    action: PermissionAction;
    description?: string;
    enabled?: boolean;
  }) => {
    try {
      if (editingRule) {
        await updatePermissionRule(editingRule.id, {
          scope: params.scope,
          permissionType: params.permissionType,
          pattern: params.pattern,
          action: params.action,
          description: params.description,
          enabled: params.enabled,
        });
      } else {
        await addPermissionRule({
          scope: params.scope,
          permissionType: params.permissionType,
          pattern: params.pattern,
          action: params.action,
          description: params.description,
        });
      }
      setEditorOpen(false);
      await loadRules();
    } catch (e) {
      console.error('保存权限规则失败:', e);
    }
  };

  // 切换启用状态
  const handleToggleEnabled = async (rule: PermissionRule) => {
    try {
      await updatePermissionRule(rule.id, { enabled: !rule.enabled });
      await loadRules();
    } catch (e) {
      console.error('更新权限规则失败:', e);
    }
  };

  return (
    <div>
      {/* 顶部工具栏 */}
      <div className="perm-toolbar">
        <h3 className="perm-title">{t('permission.title')}</h3>
        <div className="perm-toolbar-actions">
          <button className="perm-refresh-btn" onClick={loadRules} title={t('permission.refresh')}>
            <Icon name="refresh" size={14} />
          </button>
          <button className="perm-add-btn" onClick={handleAdd}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            {t('permission.addRule')}
          </button>
        </div>
      </div>

      {loading ? (
        <div className="perm-empty-state">{t('common.loading') || '加载中...'}</div>
      ) : (
        <>
          {/* 默认规则区 */}
          <div className="perm-section">
            <div className="section-header">
              <span className="section-title">{t('permission.defaultRules')}</span>
              <span className="section-badge">{defaultRules.length}</span>
            </div>
            {defaultRules.length > 0 ? (
              <div className="perm-table">
                <div className="perm-table-header">
                  <span className="perm-col-scope">{t('permission.scope')}</span>
                  <span className="perm-col-type">{t('permission.type')}</span>
                  <span className="perm-col-pattern">{t('permission.pattern')}</span>
                  <span className="perm-col-action">{t('permission.action')}</span>
                  <span className="perm-col-desc">{t('permission.description')}</span>
                </div>
                {defaultRules.map((rule) => (
                  <div key={rule.id} className="perm-table-row perm-row-default">
                    <span className="perm-col-scope">
                      <span className="perm-default-badge">{t('permission.defaultBadge')}</span>
                      {t(`permission.scopeOptions.${rule.scope}`)}
                    </span>
                    <span className="perm-col-type">{t(`permission.typeOptions.${rule.permissionType}`)}</span>
                    <span className="perm-col-pattern perm-mono">{rule.pattern}</span>
                    <span className="perm-col-action">
                      <span className={`perm-action-tag ${getActionClass(rule.action)}`}>
                        {t(`permission.actionOptions.${rule.action}`)}
                      </span>
                    </span>
                    <span className="perm-col-desc">{rule.description || '-'}</span>
                  </div>
                ))}
              </div>
            ) : (
              <div className="perm-empty-state">{t('permission.noDefaults')}</div>
            )}
          </div>

          {/* 用户规则区 */}
          <div className="perm-section">
            <div className="section-header">
              <span className="section-title">{t('permission.userRules')}</span>
              <span className="section-badge">{userRules.length}</span>
            </div>
            {userRules.length > 0 ? (
              <div className="perm-table">
                <div className="perm-table-header">
                  <span className="perm-col-scope">{t('permission.scope')}</span>
                  <span className="perm-col-type">{t('permission.type')}</span>
                  <span className="perm-col-pattern">{t('permission.pattern')}</span>
                  <span className="perm-col-action">{t('permission.action')}</span>
                  <span className="perm-col-desc">{t('permission.description')}</span>
                  <span className="perm-col-ops">{t('permission.enabled')}</span>
                  <span className="perm-col-actions"></span>
                </div>
                {userRules.map((rule) => (
                  <div key={rule.id} className="perm-table-row">
                    <span className="perm-col-scope">
                      {t(`permission.scopeOptions.${rule.scope}`)}
                    </span>
                    <span className="perm-col-type">{t(`permission.typeOptions.${rule.permissionType}`)}</span>
                    <span className="perm-col-pattern perm-mono">{rule.pattern}</span>
                    <span className="perm-col-action">
                      <span className={`perm-action-tag ${getActionClass(rule.action)}`}>
                        {t(`permission.actionOptions.${rule.action}`)}
                      </span>
                    </span>
                    <span className="perm-col-desc">{rule.description || '-'}</span>
                    <span className="perm-col-ops">
                      <label className="perm-toggle perm-toggle-sm">
                        <input
                          type="checkbox"
                          checked={rule.enabled}
                          onChange={() => handleToggleEnabled(rule)}
                        />
                        <span className="perm-toggle-slider" />
                      </label>
                    </span>
                    <span className="perm-col-actions">
                      <button
                        className="perm-action-btn"
                        title={t('common.edit')}
                        onClick={() => handleEdit(rule)}
                      >
                        <Icon name="edit" size={14} />
                      </button>
                      <button
                        className="perm-action-btn perm-action-btn-danger"
                        title={t('common.delete')}
                        onClick={() => setDeleteTarget(rule)}
                      >
                        <Icon name="trash" size={14} />
                      </button>
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <div className="perm-empty-state">{t('permission.noRules')}</div>
            )}
          </div>
        </>
      )}

      {/* 编辑/添加弹窗 */}
      <PermissionRuleEditor
        open={editorOpen}
        rule={editingRule}
        onSave={handleSave}
        onClose={() => setEditorOpen(false)}
      />

      {/* 删除确认 */}
      {deleteTarget && (
        <DeleteConfirmDialog
          name={deleteTarget.pattern}
          type="permission"
          onConfirm={handleDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}

      <style>{`
        .perm-toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 20px;
        }
        .perm-title {
          font-size: 14px;
          font-weight: 700;
          color: var(--color-text-primary);
          margin: 0;
        }
        .perm-toolbar-actions {
          display: flex;
          gap: 8px;
        }
        .perm-refresh-btn {
          width: 30px;
          height: 30px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-secondary);
          border: 1px solid var(--color-border-light);
          transition: all 0.15s;
        }
        .perm-refresh-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .perm-add-btn {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          padding: 6px 14px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          background: var(--color-accent);
          color: white;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .perm-add-btn:hover {
          background: var(--color-accent-hover);
        }
        .perm-section {
          margin-bottom: 24px;
        }
        .perm-section:last-child {
          margin-bottom: 0;
        }
        .perm-table {
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          overflow: hidden;
        }
        .perm-table-header {
          display: grid;
          grid-template-columns: 120px 100px 1fr 80px 1fr 60px 70px;
          gap: 0;
          padding: 8px 12px;
          background: var(--color-bg-sub);
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
          text-transform: uppercase;
          letter-spacing: 0.3px;
          border-bottom: 1px solid var(--color-border-light);
        }
        .perm-table-row {
          display: grid;
          grid-template-columns: 120px 100px 1fr 80px 1fr 60px 70px;
          gap: 0;
          padding: 8px 12px;
          font-size: 12px;
          color: var(--color-text-primary);
          border-bottom: 1px solid var(--color-border-light);
          transition: background 0.15s;
          align-items: center;
        }
        .perm-table-row:last-child {
          border-bottom: none;
        }
        .perm-table-row:hover {
          background: var(--color-bg-sub);
        }
        .perm-row-default {
          background: var(--color-bg-sub);
        }
        .perm-row-default:hover {
          background: var(--color-bg-hover);
        }
        .perm-mono {
          font-family: 'Consolas', 'Monaco', monospace;
          font-size: 11px;
          word-break: break-all;
        }
        .perm-default-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 3px;
          background: var(--color-accent-light);
          color: var(--color-accent);
          margin-right: 4px;
        }
        .perm-action-tag {
          display: inline-block;
          font-size: 10px;
          font-weight: 500;
          padding: 2px 8px;
          border-radius: 10px;
        }
        .perm-action-allow {
          background: rgba(34, 197, 94, 0.12);
          color: #22c55e;
        }
        .perm-action-deny {
          background: rgba(239, 68, 68, 0.12);
          color: #ef4444;
        }
        .perm-action-ask {
          background: rgba(245, 158, 11, 0.12);
          color: #f59e0b;
        }
        .perm-col-desc {
          color: var(--color-text-quaternary);
          font-size: 11px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .perm-col-actions {
          display: flex;
          gap: 4px;
          justify-content: flex-end;
        }
        .perm-action-btn {
          width: 26px;
          height: 26px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-xs);
          color: var(--color-text-quaternary);
          transition: all 0.15s;
        }
        .perm-action-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .perm-action-btn-danger:hover {
          background: rgba(239,68,68,0.1);
          color: #ef4444;
        }
        .perm-toggle-sm {
          width: 32px;
          height: 18px;
        }
        .perm-toggle-sm .perm-toggle-slider::before {
          width: 12px;
          height: 12px;
        }
        .perm-toggle-sm input:checked + .perm-toggle-slider::before {
          transform: translateX(14px);
        }
        .perm-empty-state {
          font-size: 13px;
          color: var(--color-text-quaternary);
          text-align: center;
          padding: 20px 16px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
        }
      `}</style>
    </div>
  );
}
