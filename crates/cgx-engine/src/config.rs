use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// cgx configuration — loaded from `.cgx/config.toml` in the repo root
/// or `~/.cgx/config.toml` for global defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CgxConfig {
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub analyze: AnalyzeConfig,
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub watch: WatchConfig,
    #[serde(default)]
    pub chat: ChatConfig,
    #[serde(default)]
    pub serve: ServeConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub skill: SkillConfig,
    #[serde(default)]
    pub export: ExportConfig,
}

impl CgxConfig {
    /// Load config from `.cgx/config.toml` in the given directory,
    /// falling back to `~/.cgx/config.toml` for missing fields.
    pub fn load(repo_path: &Path) -> anyhow::Result<Self> {
        let repo_config = repo_path.join(".cgx").join("config.toml");
        let global_config = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cgx")
            .join("config.toml");

        // Start with defaults
        let mut config = Self::default();

        // Load global defaults first (if no repo config exists, this is the primary config)
        if global_config.exists() {
            let content = std::fs::read_to_string(&global_config)?;
            config = toml::from_str(&content)?;
        }

        // Repo-local config fully overrides global (no partial merge to avoid
        // default-value clobbering)
        if repo_config.exists() {
            let content = std::fs::read_to_string(&repo_config)?;
            config = toml::from_str(&content)?;
        }

        Ok(config)
    }

    /// Save config to `.cgx/config.toml` in the given directory.
    pub fn save(&self, repo_path: &Path) -> anyhow::Result<()> {
        let config_dir = repo_path.join(".cgx");
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default = "default_project_name")]
    pub name: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: default_project_name(),
        }
    }
}

fn default_project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "cgx-project".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeConfig {
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
    #[serde(default = "default_churn_window")]
    pub churn_window_days: u32,
    #[serde(default = "default_co_change_threshold")]
    pub co_change_threshold: u32,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
}

impl Default for AnalyzeConfig {
    fn default() -> Self {
        Self {
            languages: default_languages(),
            exclude: default_exclude(),
            churn_window_days: default_churn_window(),
            co_change_threshold: default_co_change_threshold(),
            max_file_size: default_max_file_size(),
        }
    }
}

fn default_languages() -> Vec<String> {
    vec![
        "typescript".to_string(),
        "javascript".to_string(),
        "python".to_string(),
        "rust".to_string(),
        "go".to_string(),
        "java".to_string(),
        "php".to_string(),
    ]
}

fn default_exclude() -> Vec<String> {
    vec![
        "vendor/".to_string(),
        "generated/".to_string(),
        "*.pb.go".to_string(),
        "third_party/".to_string(),
        "build/".to_string(),
        "out/".to_string(),
    ]
}

fn default_churn_window() -> u32 {
    90
}

fn default_co_change_threshold() -> u32 {
    2
}

fn default_max_file_size() -> u64 {
    2 * 1024 * 1024
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    #[serde(default = "default_true")]
    pub incremental: bool,
    #[serde(default = "default_true")]
    pub store_hashes: bool,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            incremental: true,
            store_hashes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    #[serde(default = "default_watch_include")]
    pub include: Vec<String>,
    #[serde(default = "default_watch_ignore")]
    pub ignore: Vec<String>,
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            include: default_watch_include(),
            ignore: default_watch_ignore(),
            debounce_ms: default_debounce(),
        }
    }
}

fn default_watch_include() -> Vec<String> {
    vec![
        "*.ts".to_string(),
        "*.js".to_string(),
        "*.py".to_string(),
        "*.rs".to_string(),
        "*.go".to_string(),
        "*.java".to_string(),
        "*.php".to_string(),
    ]
}

fn default_watch_ignore() -> Vec<String> {
    vec![
        ".git".to_string(),
        "node_modules".to_string(),
        "target".to_string(),
        "dist".to_string(),
        "__pycache__".to_string(),
        ".cgx".to_string(),
    ]
}

fn default_debounce() -> u64 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    #[serde(default = "default_chat_provider")]
    pub provider: String,
    #[serde(default = "default_chat_model")]
    pub model: String,
    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,
    #[serde(default = "default_chat_timeout")]
    pub timeout: u32,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            provider: default_chat_provider(),
            model: default_chat_model(),
            ollama_host: default_ollama_host(),
            timeout: default_chat_timeout(),
        }
    }
}

fn default_chat_provider() -> String {
    "ollama".to_string()
}

fn default_chat_model() -> String {
    "codellama".to_string()
}

fn default_ollama_host() -> String {
    "http://localhost:11434".to_string()
}

fn default_chat_timeout() -> u32 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServeConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub auto_open: bool,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            auto_open: true,
        }
    }
}

fn default_port() -> u16 {
    7373
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_mcp_timeout")]
    pub timeout: u32,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: default_mcp_timeout(),
        }
    }
}

fn default_mcp_timeout() -> u32 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    #[serde(default = "default_true")]
    pub auto_generate: bool,
    #[serde(default = "default_true")]
    pub include_token_budget: bool,
    #[serde(default = "default_true")]
    pub include_architecture: bool,
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            auto_generate: true,
            include_token_budget: true,
            include_architecture: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    #[serde(default = "default_export_format")]
    pub default_format: String,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            default_format: default_export_format(),
            max_nodes: default_max_nodes(),
        }
    }
}

fn default_export_format() -> String {
    "json".to_string()
}

fn default_max_nodes() -> usize {
    80
}

fn default_true() -> bool {
    true
}
