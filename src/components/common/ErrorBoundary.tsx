import { Component, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: React.ErrorInfo) => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

/**
 * React 错误边界组件
 * 捕获子组件树中的渲染错误，防止整个应用白屏
 * 可通过 fallback 属性自定义错误展示，或使用默认的错误页面
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    console.error("[ErrorBoundary] 捕获到渲染错误:", error, errorInfo);
    this.props.onError?.(error, errorInfo);
  }

  handleReload = (): void => {
    this.setState({ hasError: false, error: null });
  };

  handleRestart = (): void => {
    window.location.reload();
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          height: "100vh",
          padding: "40px",
          background: "var(--color-bg, #f8f9fa)",
          color: "var(--color-text-primary, #1a1a1a)",
          fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        }}>
          <div style={{
            maxWidth: "480px",
            textAlign: "center",
          }}>
            {/* 错误图标 */}
            <div style={{
              width: "64px",
              height: "64px",
              borderRadius: "16px",
              background: "var(--color-error-bg, #fef2f2)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              margin: "0 auto 24px",
              fontSize: "28px",
            }}>
              !
            </div>

            <h2 style={{
              fontSize: "18px",
              fontWeight: 600,
              marginBottom: "8px",
            }}>
              页面渲染出错
            </h2>

            <p style={{
              fontSize: "14px",
              color: "var(--color-text-secondary, #666)",
              lineHeight: 1.6,
              marginBottom: "24px",
            }}>
              应用遇到了一个意外错误，部分功能可能无法正常使用。
              你可以尝试恢复页面或重启应用。
            </p>

            {/* 错误详情（可折叠） */}
            {this.state.error && (
              <details style={{
                marginBottom: "24px",
                textAlign: "left",
                background: "var(--color-bg-sub, #f1f1f1)",
                borderRadius: "8px",
                padding: "12px 16px",
                fontSize: "12px",
                fontFamily: "monospace",
                color: "var(--color-error, #dc2626)",
                maxHeight: "160px",
                overflow: "auto",
              }}>
                <summary style={{
                  cursor: "pointer",
                  fontWeight: 500,
                  marginBottom: "8px",
                  color: "var(--color-text-secondary, #666)",
                }}>
                  错误详情
                </summary>
                <pre style={{ margin: 0, whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
                  {this.state.error.message}
                  {this.state.error.stack && `\n\n${this.state.error.stack}`}
                </pre>
              </details>
            )}

            {/* 操作按钮 */}
            <div style={{ display: "flex", gap: "12px", justifyContent: "center" }}>
              <button
                onClick={this.handleReload}
                style={{
                  padding: "8px 20px",
                  borderRadius: "6px",
                  border: "1px solid var(--color-border, #e5e5e5)",
                  background: "var(--color-bg, #fff)",
                  color: "var(--color-text-primary, #1a1a1a)",
                  fontSize: "13px",
                  fontWeight: 500,
                  cursor: "pointer",
                  transition: "all 0.15s",
                }}
              >
                恢复页面
              </button>
              <button
                onClick={this.handleRestart}
                style={{
                  padding: "8px 20px",
                  borderRadius: "6px",
                  border: "none",
                  background: "var(--color-accent, #4f46e5)",
                  color: "#fff",
                  fontSize: "13px",
                  fontWeight: 500,
                  cursor: "pointer",
                  transition: "all 0.15s",
                }}
              >
                重启应用
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
