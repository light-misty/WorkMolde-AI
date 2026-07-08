//! Token 预算管理器
//! 根据模型上下文窗口大小动态分配各部分 Token 配额，
//! 控制系统提示词、工具定义、对话历史和 LLM 响应的 Token 消耗

/// Token 预算配置
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// 系统提示词配额
    pub system_prompt: usize,
    /// 工具定义配额
    pub tool_definitions: usize,
    /// 对话历史配额
    pub conversation: usize,
    /// LLM 响应配额
    pub response: usize,
}

/// Token 预算管理器
/// 根据模型上下文窗口大小计算各部分配额
pub struct TokenBudgetManager {
    /// 模型上下文窗口大小
    context_window: usize,
    /// 计算后的预算
    budget: TokenBudget,
}

impl TokenBudgetManager {
    /// 创建新的预算管理器
    /// context_window: 模型的上下文窗口大小（Token 数）
    pub fn new(context_window: usize) -> Self {
        // 上下文窗口最小值保护
        let window = context_window.max(4096);
        let budget = Self::calculate_budget(window);
        Self {
            context_window: window,
            budget,
        }
    }

    /// 使用默认上下文窗口大小创建（128K）
    pub fn default_context() -> Self {
        Self::new(128_000)
    }

    /// 计算各部分 Token 配额
    fn calculate_budget(total: usize) -> TokenBudget {
        TokenBudget {
            // 系统提示词: 不超过总窗口的 15%
            system_prompt: (total as f64 * 0.15) as usize,
            // 工具定义: 不超过总窗口的 10%
            tool_definitions: (total as f64 * 0.10) as usize,
            // 对话历史: 不超过总窗口的 50%
            conversation: (total as f64 * 0.50) as usize,
            // LLM 响应: 预留 25%
            response: (total as f64 * 0.25) as usize,
        }
    }

    /// 获取当前预算配置
    pub fn budget(&self) -> &TokenBudget {
        &self.budget
    }

    /// 获取上下文窗口大小
    pub fn context_window(&self) -> usize {
        self.context_window
    }

    /// 根据剩余 Token 空间决定是否注入规范层
    pub fn should_inject_guides(&self, current_system_tokens: usize) -> bool {
        current_system_tokens < self.budget.system_prompt
    }

    /// 估算字符串的 Token 数
    /// 采用分语言估算策略：
    /// - 中文字符: 约 1 字符 = 1.5 Token (CJK 字符)
    /// - 英文/数字/标点: 约 4 字符 = 1 Token
    ///   混合内容时分别计算后求和，比纯字符数更准确
    pub fn estimate_tokens(text: &str) -> usize {
        let mut cjk_count: usize = 0;
        let mut ascii_count: usize = 0;

        for ch in text.chars() {
            if Self::is_cjk_char(ch) {
                cjk_count += 1;
            } else {
                ascii_count += 1;
            }
        }

        // 中文字符: 1 字符 ≈ 1.5 Token
        let cjk_tokens = (cjk_count as f64 * 1.5).ceil() as usize;
        // 英文/其他: 4 字符 ≈ 1 Token
        let ascii_tokens = (ascii_count as f64 / 4.0).ceil() as usize;

        cjk_tokens + ascii_tokens
    }

    /// 判断字符是否为 CJK（中日韩）字符
    fn is_cjk_char(ch: char) -> bool {
        let cp = ch as u32;
        // CJK Unified Ideographs: 4E00-9FFF
        // CJK Unified Ideographs Extension A: 3400-4DBF
        // CJK Unified Ideographs Extension B-H: 20000-2FA1F
        // CJK Compatibility Ideographs: F900-FAFF
        // CJK Symbols and Punctuation: 3000-303F
        // Hiragana: 3040-309F
        // Katakana: 30A0-30FF
        // Hangul Syllables: AC00-D7AF
        (0x4E00..=0x9FFF).contains(&cp)
            || (0x3400..=0x4DBF).contains(&cp)
            || (0x20000..=0x2FA1F).contains(&cp)
            || (0xF900..=0xFAFF).contains(&cp)
            || (0x3000..=0x303F).contains(&cp)
            || (0x3040..=0x309F).contains(&cp)
            || (0x30A0..=0x30FF).contains(&cp)
            || (0xAC00..=0xD7AF).contains(&cp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_budget() {
        let manager = TokenBudgetManager::default_context();
        let budget = manager.budget();

        // 128K 上下文窗口的预算分配
        assert_eq!(budget.system_prompt, 19200); // 15%
        assert_eq!(budget.tool_definitions, 12800); // 10%
        assert_eq!(budget.conversation, 64000); // 50%
        assert_eq!(budget.response, 32000); // 25%
    }

    #[test]
    fn test_small_context_window() {
        let manager = TokenBudgetManager::new(4096);
        let budget = manager.budget();

        // 最小上下文窗口的预算分配
        assert!(budget.system_prompt > 0);
        assert!(budget.conversation > 0);
    }

    #[test]
    fn test_should_inject_guides() {
        let manager = TokenBudgetManager::default_context();

        // 远低于配额，应注入
        assert!(manager.should_inject_guides(1000));

        // 超过配额，不应注入
        assert!(!manager.should_inject_guides(20000));
    }

    #[test]
    fn test_estimate_tokens() {
        // 英文文本: "Hello World" = 11 字符, 约 11/4 = 3 tokens
        let en_tokens = TokenBudgetManager::estimate_tokens("Hello World");
        assert!(en_tokens > 0);
        assert!(en_tokens <= 11); // 不超过字符数

        // 中文文本: "你好世界" = 4 CJK 字符, 约 4*1.5 = 6 tokens
        let zh_tokens = TokenBudgetManager::estimate_tokens("你好世界");
        assert!(zh_tokens > 0);
        assert!(zh_tokens >= 4); // 至少等于字符数

        // 空字符串
        assert_eq!(TokenBudgetManager::estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_mixed() {
        // 混合中英文: "Hello你好World世界"
        // CJK: 你好世界 = 4, 约 6 tokens
        // ASCII: HelloWorld = 10, 约 3 tokens
        let tokens = TokenBudgetManager::estimate_tokens("Hello你好World世界");
        assert!(tokens > 0);
        // 混合内容 token 数应介于纯英文和纯中文估算之间
        let pure_char_count = "Hello你好World世界".chars().count();
        assert!(tokens <= pure_char_count * 2); // 不超过字符数的2倍
    }

    #[test]
    fn test_is_cjk_char() {
        // 中文字符
        assert!(TokenBudgetManager::is_cjk_char('你'));
        assert!(TokenBudgetManager::is_cjk_char('世'));

        // 日文平假名/片假名
        assert!(TokenBudgetManager::is_cjk_char('あ'));
        assert!(TokenBudgetManager::is_cjk_char('ア'));

        // 韩文
        assert!(TokenBudgetManager::is_cjk_char('한'));

        // 英文字符
        assert!(!TokenBudgetManager::is_cjk_char('A'));
        assert!(!TokenBudgetManager::is_cjk_char('z'));

        // 数字和标点
        assert!(!TokenBudgetManager::is_cjk_char('0'));
        assert!(!TokenBudgetManager::is_cjk_char('!'));
    }
}
