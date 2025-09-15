use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use crate::exception_handler::ThreadRestartPolicy;

#[derive(Debug, Clone)]
pub struct ProcessRestartPolicy {
    pub max_restarts: u32,
    pub restart_delay: Duration,
    pub backoff_multiplier: f64,
    pub max_restart_delay: Duration,
}

impl Default for ProcessRestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            restart_delay: Duration::from_secs(2),
            backoff_multiplier: 2.0,
            max_restart_delay: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessConfig {
    pub name: String,
    pub executable_path: String,
    pub working_directory: String,
    pub arguments: Vec<String>,
    pub restart_policy: ProcessRestartPolicy,
    pub environment_vars: HashMap<String, String>,
}

impl ProcessConfig {
    fn normalize_path(&self, path: &Path) -> PathBuf {
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                // Skip current directory components (.)
                std::path::Component::CurDir => continue,
                // Handle parent directory components (..)
                std::path::Component::ParentDir => {
                    if let Some(last) = components.last() {
                        if last != &std::path::Component::ParentDir {
                            components.pop(); // Remove the last component when we see ..
                            continue;
                        }
                    }
                    components.push(component);
                }
                // Keep all other components
                _ => components.push(component),
            }
        }

        // Reconstruct the path from the filtered components
        let mut normalized = PathBuf::new();
        for component in components {
            normalized.push(component);
        }
        normalized
    }
    pub fn new(name: String) -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let parent_dir = current_dir.parent().unwrap_or(&current_dir);

        // 推断可执行文件路径
        let executable_path = if cfg!(target_os = "windows") {
            format!("{}.exe", name)
        } else {
            name.clone()
        };

        // 推断工作目录（与当前目录同级）
        let working_directory = parent_dir.join(&name);

        Self {
            name: name.clone(),
            executable_path,
            working_directory: working_directory.to_string_lossy().to_string(),
            arguments: Vec::new(),
            restart_policy: ProcessRestartPolicy::default(),
            environment_vars: HashMap::new(),
        }
    }

    pub fn with_executable_path(mut self, path: String) -> Self {
        self.executable_path = path;
        self
    }

    pub fn with_working_directory(mut self, dir: String) -> Self {
        self.working_directory = dir;
        self
    }

    pub fn with_arguments(mut self, args: Vec<String>) -> Self {
        self.arguments = args;
        self
    }

    pub fn with_restart_policy(mut self, policy: ProcessRestartPolicy) -> Self {
        self.restart_policy = policy;
        self
    }

    pub fn with_environment_var(mut self, key: String, value: String) -> Self {
        self.environment_vars.insert(key, value);
        self
    }

    pub fn get_full_executable_path(&self) -> PathBuf {
        let working_dir = Path::new(&self.working_directory);
        let full_path = if Path::new(&self.executable_path).is_absolute() {
            PathBuf::from(&self.executable_path)
        } else {
            working_dir.join(&self.executable_path)
        };
        // Normalize the path to remove redundant components like ./ and ../
        self.normalize_path(&full_path)
    }

    pub fn validate(&self) -> Result<(), String> {
        // 检查进程名称
        if self.name.is_empty() {
            return Err("进程名称不能为空".to_string());
        }

        // 检查工作目录是否存在
        let working_dir = Path::new(&self.working_directory);
        if !working_dir.exists() {
            return Err(format!("工作目录不存在: {}", self.working_directory));
        }

        if !working_dir.is_dir() {
            return Err(format!("工作目录不是一个目录: {}", self.working_directory));
        }

        // 检查可执行文件是否存在
        let exe_path = self.get_full_executable_path();
        if !exe_path.exists() {
            return Err(format!("可执行文件不存在: {}", exe_path.display()));
        }

        // 在Windows上检查是否有执行权限（简单检查文件扩展名）
        if cfg!(target_os = "windows") {
            let exe_extension = exe_path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");

            if !["exe", "bat", "cmd", "com"].contains(&exe_extension.to_lowercase().as_str()) {
                crate::log_warn(&format!(
                    "警告：文件 {} 可能不是可执行文件（扩展名：{}）",
                    exe_path.display(),
                    exe_extension
                ));
            }
        }

        Ok(())
    }
}

pub struct ConfigParser {
    config_file_path: PathBuf,
    last_modified: Option<SystemTime>,
    cached_configs: HashMap<String, ProcessConfig>,
}

impl ConfigParser {
    pub fn new<P: AsRef<Path>>(config_path: P) -> Self {
        Self {
            config_file_path: config_path.as_ref().to_path_buf(),
            last_modified: None,
            cached_configs: HashMap::new(),
        }
    }

    pub fn load_config(&mut self) -> Result<HashMap<String, ProcessConfig>, String> {
        crate::log_info(&format!("加载配置文件: {}", self.config_file_path.display()));

        if !self.config_file_path.exists() {
            return Err(format!("配置文件不存在: {}", self.config_file_path.display()));
        }

        // 检查文件修改时间
        let metadata = fs::metadata(&self.config_file_path)
            .map_err(|e| format!("无法读取配置文件元数据: {}", e))?;

        let modified_time = metadata.modified()
            .map_err(|e| format!("无法获取文件修改时间: {}", e))?;

        // 如果文件没有变化，返回缓存的配置
        if let Some(last_mod) = self.last_modified {
            if modified_time <= last_mod {
                crate::log_info("配置文件未变化，使用缓存配置");
                return Ok(self.cached_configs.clone());
            }
        }

        // 读取并解析配置文件
        let file = fs::File::open(&self.config_file_path)
            .map_err(|e| format!("无法打开配置文件: {}", e))?;

        let reader = BufReader::new(file);
        let mut configs = HashMap::new();
        let mut line_number = 0;

        for line_result in reader.lines() {
            line_number += 1;
            let line = line_result
                .map_err(|e| format!("读取第{}行时出错: {}", line_number, e))?;

            // 解析单行配置
            if let Some(config) = self.parse_line(&line, line_number)? {
                if configs.contains_key(&config.name) {
                    crate::log_warn(&format!("警告：进程 '{}' 在第{}行重复定义", config.name, line_number));
                }
                configs.insert(config.name.clone(), config);
            }
        }

        crate::log_info(&format!("成功加载 {} 个进程配置", configs.len()));

        // 验证所有配置
        let mut validation_errors = Vec::new();
        for (name, config) in &configs {
            if let Err(error) = config.validate() {
                validation_errors.push(format!("进程 '{}': {}", name, error));
            }
        }

        if !validation_errors.is_empty() {
            crate::log_warn("配置验证发现以下问题:");
            for error in &validation_errors {
                crate::log_warn(&format!("  - {}", error));
            }
        }

        // 更新缓存
        self.last_modified = Some(modified_time);
        self.cached_configs = configs.clone();

        Ok(configs)
    }

    fn parse_line(&self, line: &str, line_number: usize) -> Result<Option<ProcessConfig>, String> {
        let line = line.trim();

        // 忽略空行和注释行
        if line.is_empty() || line.starts_with('#') {
            return Ok(None);
        }

        // 支持两种格式：
        // 1. 简单格式：只有进程名
        // 2. 扩展格式：进程名:可执行文件:工作目录:启动参数:最大重启次数

        if line.contains(':') {
            self.parse_extended_format(line, line_number)
        } else {
            self.parse_simple_format(line, line_number)
        }
    }

    fn parse_simple_format(&self, line: &str, _line_number: usize) -> Result<Option<ProcessConfig>, String> {
        let process_name = line.trim().to_string();

        if process_name.is_empty() {
            return Ok(None);
        }

        let config = ProcessConfig::new(process_name);
        Ok(Some(config))
    }

    fn parse_extended_format(&self, line: &str, line_number: usize) -> Result<Option<ProcessConfig>, String> {
        let parts: Vec<&str> = line.split(':').collect();

        if parts.len() < 2 {
            return Err(format!("第{}行格式错误：扩展格式至少需要进程名和可执行文件", line_number));
        }

        let process_name = parts[0].trim().to_string();
        if process_name.is_empty() {
            return Err(format!("第{}行：进程名不能为空", line_number));
        }

        let mut config = ProcessConfig::new(process_name);

        // 可执行文件路径
        if parts.len() > 1 && !parts[1].trim().is_empty() {
            config = config.with_executable_path(parts[1].trim().to_string());
        }

        // 工作目录
        if parts.len() > 2 && !parts[2].trim().is_empty() {
            let working_dir = parts[2].trim();
            // 支持相对路径解析
            let working_dir = if working_dir == "." {
                config.working_directory.clone()
            } else if working_dir.starts_with("../") || working_dir.starts_with("./") {
                let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                current_dir.join(working_dir).to_string_lossy().to_string()
            } else {
                working_dir.to_string()
            };
            config = config.with_working_directory(working_dir);
        }

        // 启动参数
        if parts.len() > 3 && !parts[3].trim().is_empty() {
            let args_str = parts[3].trim();
            let arguments: Vec<String> = args_str
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            config = config.with_arguments(arguments);
        }

        // 最大重启次数
        if parts.len() > 4 && !parts[4].trim().is_empty() {
            match parts[4].trim().parse::<u32>() {
                Ok(max_restarts) => {
                    let restart_policy = ProcessRestartPolicy {
                        max_restarts,
                        ..ProcessRestartPolicy::default()
                    };
                    config = config.with_restart_policy(restart_policy);
                }
                Err(_) => {
                    return Err(format!("第{}行：最大重启次数必须是数字", line_number));
                }
            }
        }

        Ok(Some(config))
    }

    pub fn has_config_changed(&self) -> bool {
        if let Some(last_mod) = self.last_modified {
            if let Ok(metadata) = fs::metadata(&self.config_file_path) {
                if let Ok(current_mod) = metadata.modified() {
                    return current_mod > last_mod;
                }
            }
        }
        true // 如果无法确定，假设已更改
    }

    pub fn reload_if_changed(&mut self) -> Result<Option<HashMap<String, ProcessConfig>>, String> {
        if self.has_config_changed() {
            crate::log_info("检测到配置文件变化，重新加载配置");
            self.load_config().map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn get_config_file_path(&self) -> &Path {
        &self.config_file_path
    }

    pub fn create_sample_config(&self) -> Result<(), String> {
        if self.config_file_path.exists() {
            return Err("配置文件已存在".to_string());
        }

        let sample_content = r#"# Wei守护进程配置文件
# 格式说明：
# 简单格式：进程名
# 扩展格式：进程名:可执行文件:工作目录:启动参数:最大重启次数
#
# 示例：
# my-app
# web-server:./server.exe:.:--port=8080 --config=server.conf:5
# database:../db/database.exe:../db:--data-dir=./data:3

# 在这里添加你的应用程序配置
example-app
"#;

        fs::write(&self.config_file_path, sample_content)
            .map_err(|e| format!("无法创建示例配置文件: {}", e))?;

        crate::log_info(&format!("已创建示例配置文件: {}", self.config_file_path.display()));
        Ok(())
    }
}

pub fn load_daemon_config() -> Result<HashMap<String, ProcessConfig>, String> {
    let mut parser = ConfigParser::new("daemon.dat");

    // 如果配置文件不存在，尝试创建示例配置
    if !parser.get_config_file_path().exists() {
        crate::log_warn("配置文件 daemon.dat 不存在，创建示例配置文件");
        parser.create_sample_config()?;
        return Ok(HashMap::new()); // 返回空配置，让用户填写
    }

    parser.load_config()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_simple_format_parsing() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"
# 这是注释
app1
app2

# 另一个注释
app3
"#;
        fs::write(temp_file.path(), content).unwrap();

        let mut parser = ConfigParser::new(temp_file.path());
        let configs = parser.load_config().unwrap();

        assert_eq!(configs.len(), 3);
        assert!(configs.contains_key("app1"));
        assert!(configs.contains_key("app2"));
        assert!(configs.contains_key("app3"));
    }

    #[test]
    fn test_extended_format_parsing() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "web-server:./server.exe:.:--port=8080:5\n";
        fs::write(temp_file.path(), content).unwrap();

        let mut parser = ConfigParser::new(temp_file.path());
        let configs = parser.load_config().unwrap();

        assert_eq!(configs.len(), 1);
        let config = configs.get("web-server").unwrap();
        assert_eq!(config.executable_path, "./server.exe");
        assert_eq!(config.arguments, vec!["--port=8080"]);
        assert_eq!(config.restart_policy.max_restarts, 5);
    }
}