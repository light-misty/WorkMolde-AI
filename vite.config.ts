import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      output: {
        // 手动 chunk 分割，将大型第三方库拆分为独立文件
        manualChunks: {
          // PDF 渲染引擎（体积大，独立加载）
          "vendor-pdf": ["pdfjs-dist"],
          // 图表库
          "vendor-recharts": ["recharts"],
          // Markdown 渲染链
          "vendor-markdown": ["react-markdown", "remark-gfm", "rehype-highlight"],
        },
      },
    },
    // chunk 大小警告阈值调整为 800KB
    chunkSizeWarningLimit: 800,
  },
});
