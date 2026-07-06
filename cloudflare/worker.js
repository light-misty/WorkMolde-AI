/**
 * DocAgent CDN 代理 - Cloudflare Worker
 *
 * 作用：代理 GitHub Release 下载，加速国内用户访问
 * - GET /latest.json                                代理 GitHub Release 的 latest.json
 * - GET /releases/latest/download/latest.json       同上（兼容 Tauri 默认路径）
 * - GET /releases/download/:tag/:asset              代理 GitHub Release 产物下载
 *
 * 部署说明：
 *   1. 注册 Cloudflare 账号 (https://dash.cloudflare.com/sign-up)
 *   2. 进入 Workers & Pages → Create application → Create Worker
 *   3. 将本文件内容粘贴到编辑器中，保存并部署
 *   4. 记下 Worker 地址（如 https://docagent-proxy.your-name.workers.dev）
 *   5. 在 GitHub 仓库 Settings → Secrets → Actions 添加 CDN_BASE_URL = Worker 地址
 *   6. （可选）绑定自定义域名以获得更好的国内访问速度
 *
 * 注意：GitHub 仓库地址在下方 GITHUB_OWNER / GITHUB_REPO 常量中配置
 */

// GitHub 仓库所有者（大小写敏感）
const GITHUB_OWNER = 'XuMingKe-06';
// GitHub 仓库名（大小写敏感）
const GITHUB_REPO = 'DocAgent';

export default {
  async fetch(request) {
    const url = new URL(request.url);
    const path = url.pathname;

    // 健康检查端点
    if (path === '/' || path === '/health') {
      return new Response('DocAgent CDN Proxy OK', {
        status: 200,
        headers: { 'Content-Type': 'text/plain' }
      });
    }

    // 只允许 GET / HEAD 请求
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return new Response('Method Not Allowed', { status: 405 });
    }

    // 构造目标 GitHub URL
    let githubUrl;
    if (path === '/latest.json' || path === '/releases/latest/download/latest.json') {
      // 代理最新 Release 的 latest.json
      githubUrl = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest/download/latest.json`;
    } else if (path.startsWith('/releases/download/')) {
      // 代理指定 tag 的产物下载
      // 路径格式: /releases/download/:tag/:asset
      githubUrl = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}${path}`;
    } else {
      return new Response('Not Found', { status: 404 });
    }

    try {
      // 从 GitHub 下载（自动跟随 302 重定向到 objects.githubusercontent.com）
      const response = await fetch(githubUrl, {
        method: request.method,
        redirect: 'follow'
      });

      // 构造响应头：添加 CORS 和缓存
      const newHeaders = new Headers(response.headers);
      newHeaders.set('Access-Control-Allow-Origin', '*');
      newHeaders.set('Access-Control-Allow-Methods', 'GET, HEAD, OPTIONS');
      // 小文件（latest.json）缓存 5 分钟，大文件（exe）缓存 1 小时
      const contentLength = Number(response.headers.get('Content-Length') || 0);
      const cacheMaxAge = contentLength > 1024 * 1024 ? 3600 : 300;
      newHeaders.set('Cache-Control', `public, max-age=${cacheMaxAge}`);

      // 流式返回响应体（避免内存爆炸，支持大文件）
      return new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers: newHeaders
      });
    } catch (err) {
      return new Response(`Proxy Error: ${err.message}`, { status: 502 });
    }
  }
};
