use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ImeError {
    #[error("词典加载失败: {0}")]
    DictLoadFailed(#[from] std::io::Error),

    #[error("词典解析失败: {0}")]
    DictParseFailed(String),

    #[error("配置解析失败: {0}")]
    ConfigParseFailed(String),

    #[error("配置保存失败: {0}")]
    ConfigSaveFailed(String),

    #[error("搜索引擎未就绪")]
    EngineNotReady,

    #[error("搜索引擎错误: {0}")]
    SearchError(String),

    #[error("平台初始化失败: {0}")]
    PlatformInitFailed(String),

    #[error("用户数据操作失败: {0}")]
    UserDataError(String),

    #[error("UI 初始化失败: {0}")]
    UiInitFailed(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, ImeError>;

#[allow(dead_code)]
impl ImeError {
    pub fn search_error(msg: impl Into<String>) -> Self {
        ImeError::SearchError(msg.into())
    }

    pub fn platform_error(msg: impl Into<String>) -> Self {
        ImeError::PlatformInitFailed(msg.into())
    }

    pub fn user_data_error(msg: impl Into<String>) -> Self {
        ImeError::UserDataError(msg.into())
    }

    pub fn dict_parse_error(msg: impl Into<String>) -> Self {
        ImeError::DictParseFailed(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ImeError::DictLoadFailed(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(err.to_string().contains("词典加载失败"));
    }

    #[test]
    fn test_error_from_io() {
        let result: Result<()> =
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied").into());
        match result {
            Err(ImeError::DictLoadFailed(_)) => {}
            _ => panic!("Expected DictLoadFailed"),
        }
    }

    #[test]
    fn test_config_save_error() {
        let err = ImeError::ConfigSaveFailed("failed to write config".to_string());
        assert!(err.to_string().contains("配置保存失败"));
    }
}
