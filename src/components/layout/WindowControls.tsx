import { useState, useEffect, useCallback } from "react";
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { Icon } from "../common/Icon";

// 最小窗口尺寸常量
const MIN_WINDOW_WIDTH = 960;
const MIN_WINDOW_HEIGHT = 600;

export function WindowControls() {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(true);

  // 检查窗口是否处于最大化状态
  const checkMaximized = useCallback(async () => {
    try {
      const maximized = await getCurrentWindow().isMaximized();
      setIsMaximized(maximized);
    } catch {
      // 非 Tauri 环境忽略错误
    }
  }, []);

  useEffect(() => {
    // 初始化时检查窗口状态
    checkMaximized();

    // 监听窗口尺寸变化事件，同步最大化状态
    // 当用户拖动标题栏取消最大化、双击标题栏切换最大化等操作时，
    // 窗口会触发 resize 事件，此时需要重新检查 isMaximized 状态
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await getCurrentWindow().onResized(() => {
          checkMaximized();
        });
      } catch {
        // 非 Tauri 环境忽略错误
      }
    };
    setupListener();

    return () => {
      unlisten?.();
    };
  }, [checkMaximized]);

  // 最小化窗口
  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch {
      // 非 Tauri 环境忽略错误
    }
  };

  // 切换最大化/还原
  const handleToggleMaximize = async () => {
    try {
      const win = getCurrentWindow();
      if (isMaximized) {
        await win.unmaximize();
        await win.setSize(new LogicalSize(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT));
        await win.center();
        setIsMaximized(false);
      } else {
        await win.maximize();
        setIsMaximized(true);
      }
    } catch {
      // 非 Tauri 环境忽略错误
    }
  };

  // 关闭窗口
  const handleClose = async () => {
    try {
      await getCurrentWindow().close();
    } catch {
      // 非 Tauri 环境忽略错误
    }
  };

  return (
    <div className="h-full flex items-stretch flex-shrink-0">
      <button
        className="w-12 h-full flex items-center justify-center hover:bg-border transition-colors"
        title={t('windowControls.minimize')}
        onPointerUp={handleMinimize}
      >
        <Icon name="minimize" size={16} className="text-text-secondary" />
      </button>

      {/* 最大化/还原按钮 */}
      <button
        className="w-12 h-full flex items-center justify-center hover:bg-border transition-colors"
        title={isMaximized ? t('windowControls.restore') : t('windowControls.maximize')}
        onPointerUp={handleToggleMaximize}
      >
        <Icon
          name={isMaximized ? "unmaximize" : "maximize"}
          size={16}
          className="text-text-secondary"
        />
      </button>

      {/* 关闭按钮 */}
      <button
        className="w-12 h-full flex items-center justify-center hover:bg-error hover:text-white transition-colors group"
        title={t('windowControls.close')}
        onPointerUp={handleClose}
      >
        <Icon name="close" size={16} className="text-text-secondary group-hover:text-white" />
      </button>
    </div>
  );
}
