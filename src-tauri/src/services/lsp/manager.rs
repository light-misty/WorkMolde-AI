//! LSP 服务器管理器:管理多个语言的 LSP 服务器
//! 按需启动、自动停止、健康检查

use crate::errors::CommandError;
use crate::models::lsp::*;
use crate::services::lsp::client::LspClient;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// LSP 服务器管理器
pub struct LspServerManager {
    /// 已启动的 LSP 客户端(按语言名称索引)
    clients: RwLock<HashMap<String, Arc<LspClient>>>,
    /// LSP 服务器配置(按语言名称索引)
    configs: RwLock<HashMap<String, LspServerConfig>>,
    /// 工作区根目录
    workspace_root: RwLock<PathBuf>,
    /// 请求超时时间(从 LspConfig.request_timeout_seconds 读取)
    request_timeout: Duration,
}

impl LspServerManager {
    /// 创建 LSP 服务器管理器
    pub fn new(workspace_root: PathBuf, request_timeout: Duration) -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
            workspace_root: RwLock::new(workspace_root),
            request_timeout,
        }
    }

    /// 注册 LSP 服务器配置
    pub async fn register_config(&self, config: LspServerConfig) {
        let mut configs = self.configs.write().await;
        configs.insert(config.language.clone(), config);
    }

    /// 获取或启动指定语言的 LSP 服务器
    /// 若服务器已就绪则直接返回,否则按配置启动新实例
    pub async fn get_or_start(&self, language: &str) -> Result<Arc<LspClient>, CommandError> {
        // 检查是否已启动且就绪
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(language) {
                if client.get_status().await == LspServerStatus::Ready {
                    return Ok(client.clone());
                }
            }
        }

        // 获取该语言的 LSP 配置
        let config = {
            let configs = self.configs.read().await;
            configs.get(language).cloned().ok_or_else(|| {
                CommandError::config(
                    crate::errors::CONFIG_MISSING_FIELD,
                    format!("语言 {} 未配置 LSP 服务器", language),
                )
            })?
        };

        // 创建客户端并启动
        let workspace_root = self.workspace_root.read().await.clone();
        let client = Arc::new(LspClient::new(
            language.to_string(),
            workspace_root,
            self.request_timeout,
        ));
        client.start(&config.command).await?;

        // 存入 clients 映射
        {
            let mut clients = self.clients.write().await;
            clients.insert(language.to_string(), client.clone());
        }

        Ok(client)
    }

    /// 停止指定语言的服务器
    pub async fn stop(&self, language: &str) -> Result<(), CommandError> {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.remove(language) {
            client.shutdown().await?;
        }
        Ok(())
    }

    /// 停止所有服务器
    pub async fn stop_all(&self) -> Result<(), CommandError> {
        let mut clients = self.clients.write().await;
        for (_, client) in clients.drain() {
            // 停止单个服务器的错误不中断其余服务器的停止
            let _ = client.shutdown().await;
        }
        Ok(())
    }

    /// 获取所有服务器状态
    /// 先遍历 configs 生成 Stopped 占位,再用 clients 真实状态覆盖
    /// 三个 RwLock 独立获取读锁,不嵌套持锁,避免死锁
    pub async fn get_all_status(&self) -> Vec<LspServerInfo> {
        let workspace_root = self.workspace_root.read().await.clone();
        let mut status_map: HashMap<String, LspServerInfo> = HashMap::new();

        // 遍历已注册配置,生成占位信息(状态为 Stopped)
        {
            let configs = self.configs.read().await;
            for (language, _config) in configs.iter() {
                status_map.insert(
                    language.clone(),
                    LspServerInfo {
                        language: language.clone(),
                        server_name: None,
                        server_version: None,
                        workspace_root: workspace_root.clone(),
                        status: LspServerStatus::Stopped,
                        capabilities: None,
                        started_at: 0,
                        last_activity_at: 0,
                        error: None,
                    },
                );
            }
        }

        // 遍历已启动客户端,用真实状态覆盖同语言占位信息
        {
            let clients = self.clients.read().await;
            for (_, client) in clients.iter() {
                let language = client.language().to_string();
                if let Some(info) = client.get_server_info().await {
                    status_map.insert(language, info);
                } else {
                    // 服务器尚未初始化,用 get_status 状态构造信息覆盖
                    status_map.insert(
                        language.clone(),
                        LspServerInfo {
                            language,
                            server_name: None,
                            server_version: None,
                            workspace_root: workspace_root.clone(),
                            status: client.get_status().await,
                            capabilities: None,
                            started_at: 0,
                            last_activity_at: 0,
                            error: Some("服务器信息尚未初始化".to_string()),
                        },
                    );
                }
            }
        }

        status_map.into_values().collect()
    }

    /// 搜索工作区符号(遍历所有已启动且就绪的服务器)
    pub async fn workspace_symbol(&self, query: &str) -> Result<Vec<LspSymbol>, CommandError> {
        let clients = self.clients.read().await;
        let mut all_symbols = Vec::new();

        for (_, client) in clients.iter() {
            // 仅查询就绪状态的服务器
            if client.get_status().await != LspServerStatus::Ready {
                continue;
            }
            match client.workspace_symbol(query).await {
                Ok(symbols) => all_symbols.extend(symbols),
                Err(e) => {
                    log::warn!(
                        "workspace_symbol 查询失败(language={}): {}",
                        client.language(),
                        e
                    );
                }
            }
        }

        Ok(all_symbols)
    }

    /// 更新工作区根目录
    /// 需先停止所有现有服务器(它们持有旧根目录),再更新根目录
    pub async fn update_workspace_root(&self, new_root: PathBuf) -> Result<(), CommandError> {
        self.stop_all().await?;
        let mut root = self.workspace_root.write().await;
        *root = new_root;
        Ok(())
    }

    /// 执行健康检查(遍历所有已启动客户端检查状态)
    /// 由后台任务定期调用,记录异常状态
    pub async fn health_check(&self) -> Result<(), CommandError> {
        let clients = self.clients.read().await;
        for (language, client) in clients.iter() {
            let status = client.get_status().await;
            if status != LspServerStatus::Ready {
                log::warn!("LSP 服务器 {} 状态异常: {:?}", language, status);
            }
        }
        Ok(())
    }

    /// 预热常用语言的 LSP 服务器(默认不调用,供未来扩展)
    /// 预热可减少首次调用延迟,但增加启动资源占用
    pub async fn warmup(&self, languages: &[&str]) {
        for lang in languages {
            if let Err(e) = self.get_or_start(lang).await {
                log::debug!("预热 LSP {} 失败: {}", lang, e.message);
            }
        }
    }
}
