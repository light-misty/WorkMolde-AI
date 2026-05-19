import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { Icon } from "../common/Icon";

// 最小窗口尺寸常量
const MIN_WINDOW_WIDTH = 960;
const MIN_WINDOW_HEIGHT = 600;

export function WindowControls() {
  const [isMaximized, setIsMaximized] = useState(true);

  useEffect(() => {
    // 初始化时检查窗口状态
    const checkMaximized = async () => {
      try {
        const maximized = await getCurrentWindow().isMaximized();
        setIsMaximized(maximized);
      } catch {
        // 非 Tauri 环境忽略错误
      }
    };
    checkMaximized();
  }, []);

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
        // 从最大化还原时，调整为最小尺寸并居中显示
        await win.unmaximize();
        await win.setSize(new LogicalSize(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT));
        // 使用内置的 center() 方法让窗口居中
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
    <div className="flex items-center gap-0">
      {/* 最小化按钮 */}
      <button
        className="w-11 h-8 flex items-center justify-center hover:bg-bg-sub transition-colors"
        title="最小化"
        onClick={handleMinimize}
      >
        <Icon name="minimize" size={16} className="text-text-secondary" />
      </button>

      {/* 最大化/还原按钮 */}
      <button
        className="w-11 h-8 flex items-center justify-center hover:bg-bg-sub transition-colors"
        title={isMaximized ? "还原" : "最大化"}
        onClick={handleToggleMaximize}
      >
        <Icon
          name={isMaximized ? "unmaximize" : "maximize"}
          size={16}
          className="text-text-secondary"
        />
      </button>

      {/* 关闭按钮 */}
      <button
        className="w-11 h-8 flex items-center justify-center hover:bg-red-500 hover:text-white transition-colors group"
        title="关闭"
        onClick={handleClose}
      >
        <Icon name="close" size={16} className="text-text-secondary group-hover:text-white" />
      </button>
    </div>
  );
}
