//! Web 服务模块入口
//! 提供 URL 验证、网页内容获取、网络搜索功能

pub mod fetcher;
pub mod searcher;
pub mod url_validator;

pub use fetcher::FetchResult;
pub use fetcher::WebFetcher;
pub use searcher::SearchResponse;
pub use searcher::SearchResultItem;
pub use searcher::WebSearcher;
pub use url_validator::UrlValidator;
pub use url_validator::ValidationResult;
