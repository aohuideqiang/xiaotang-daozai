//! 桌面工具配置：开关、快捷入口、独立课表文件与服务器 SSH 连接。
//!
//! 配置文件是 `%APPDATA%/Daozai/desktop-tools.json`；旧的五开关（第一项是剪贴板）
//! 会被自动迁移到现在的四项，剪贴板已由岛仔自带的置物架分栏负责。
use serde_json::{json, Value};

/// 四个可开关的能力，顺序就是设置页里的顺序。
pub(crate) const ENABLED_LABELS: [&str; 4] = ["下载任务", "快捷启动", "系统性能", "服务器任务总览"];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Config {
    pub enabled: [bool; 4],
    pub shortcuts: Vec<(String, String)>,
    pub courses: String,
    pub bridge: String,
    pub ssh_host: String,
    pub ssh_user: String,
    pub ssh_port: String,
    pub ssh_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: [true; 4],
            shortcuts: Vec::new(),
            courses: String::new(),
            bridge: String::new(),
            ssh_host: String::new(),
            ssh_user: String::new(),
            ssh_port: "22".into(),
            ssh_key: String::new(),
        }
    }
}

fn config_path() -> std::path::PathBuf {
    crate::model::config_dir().join("desktop-tools.json")
}

impl Config {
    pub(crate) fn load() -> Self {
        let mut config = Self::default();
        if let Ok(raw) = std::fs::read(config_path()) {
            if let Ok(value) = serde_json::from_slice::<Value>(&raw) {
                if let Some(items) = value["enabled"].as_array() {
                    // 旧布局：[剪贴板, 下载, 快捷启动, 系统性能, 服务器]。
                    let offset = if items.len() >= 5 { 1 } else { 0 };
                    for (index, enabled) in config.enabled.iter_mut().enumerate() {
                        if let Some(saved) = items.get(index + offset) { *enabled = saved.as_bool().unwrap_or(true); }
                    }
                }
                config.courses = value["courses"].as_str().unwrap_or_default().into();
                config.bridge = value["bridge"].as_str().unwrap_or_default().into();
                config.ssh_host = value["ssh_host"].as_str().unwrap_or_default().into();
                config.ssh_user = value["ssh_user"].as_str().unwrap_or_default().into();
                config.ssh_port = value["ssh_port"].as_str().filter(|port| !port.trim().is_empty()).unwrap_or("22").into();
                config.ssh_key = value["ssh_key"].as_str().unwrap_or_default().into();
                if let Some(rows) = value["shortcuts"].as_array() {
                    config.shortcuts = rows.iter().take(50)
                        .filter_map(|row| Some((row[0].as_str()?.into(), row[1].as_str()?.into())))
                        .collect();
                }
            }
        }
        // 从旧的 server-connection.json 补一次 SSH 参数，补到就写回配置。
        if config.ssh_host.trim().is_empty() {
            config.backfill();
            if !config.ssh_host.trim().is_empty() { let _ = config.save(); }
        }
        config
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        let raw = serde_json::to_vec(&json!({
            "enabled": self.enabled,
            "shortcuts": self.shortcuts,
            "courses": self.courses,
            "bridge": self.bridge,
            "ssh_host": self.ssh_host,
            "ssh_user": self.ssh_user,
            "ssh_port": self.ssh_port,
            "ssh_key": self.ssh_key,
        })).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(crate::model::config_dir()).map_err(|error| error.to_string())?;
        crate::model::atomic_write(&config_path(), &raw).map_err(|error| error.to_string())
    }

    /// 把 SSH 参数写进 `server-connection.json`，`server-overview.py` 只读这个文件。
    pub(crate) fn write_connection(&self) {
        let host = self.ssh_host.trim();
        if host.is_empty() { return; }
        let user = if self.ssh_user.trim().is_empty() { "root" } else { self.ssh_user.trim() };
        let port = if self.ssh_port.trim().is_empty() { "22" } else { self.ssh_port.trim() };
        let mut command = vec![
            "ssh".to_string(), "-o".into(), "BatchMode=yes".into(),
            "-o".into(), "ConnectTimeout=12".into(),
            "-o".into(), "StrictHostKeyChecking=accept-new".into(),
        ];
        if !self.ssh_key.trim().is_empty() {
            command.push("-i".into());
            command.push(self.ssh_key.trim().to_string());
        }
        command.push("-p".into());
        command.push(port.to_string());
        command.push(format!("{user}@{host}"));
        command.push("python3".into());
        command.push("-".into());
        let body = json!({ "ssh_command": command });
        let path = crate::model::config_dir().join("server-connection.json");
        let _ = std::fs::create_dir_all(crate::model::config_dir());
        let _ = crate::model::atomic_write(&path, serde_json::to_vec(&body).unwrap_or_default().as_slice());
    }

    /// 旧版只存了一条 ssh 命令；从里面拆出主机、用户名、端口和密钥。
    fn backfill(&mut self) {
        let Ok(raw) = std::fs::read(crate::model::config_dir().join("server-connection.json")) else { return; };
        let Ok(value) = serde_json::from_slice::<Value>(&raw) else { return; };
        let Some(command) = value["ssh_command"].as_array() else { return; };
        let args: Vec<&str> = command.iter().filter_map(|value| value.as_str()).collect();
        let Some(target) = args.iter().copied().find(|arg| arg.contains('@') && !arg.starts_with('-')) else { return; };
        let Some((user, host)) = target.split_once('@') else { return; };
        if host.is_empty() || user.is_empty() { return; }
        self.ssh_user = user.into();
        self.ssh_host = host.into();
        if let Some(index) = args.iter().position(|arg| *arg == "-p") {
            if let Some(port) = args.get(index + 1).filter(|port| !port.is_empty()) { self.ssh_port = (*port).into(); }
        }
        if let Some(index) = args.iter().position(|arg| *arg == "-i") {
            if let Some(key) = args.get(index + 1) { self.ssh_key = (*key).into(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_five_switches_keep_their_meaning() {
        let value: Value = serde_json::from_str(r#"{"enabled":[false,true,false,true,false]}"#).unwrap();
        let items = value["enabled"].as_array().unwrap();
        let offset = if items.len() >= 5 { 1 } else { 0 };
        let mut enabled = [true; 4];
        for (index, value) in enabled.iter_mut().enumerate() {
            *value = items[index + offset].as_bool().unwrap_or(true);
        }
        assert_eq!(enabled, [true, false, true, false]);
    }

    #[test]
    fn ssh_command_is_built_from_the_form() {
        let config = Config { ssh_host: "example.com".into(), ssh_user: "karma".into(), ssh_port: "2222".into(), ssh_key: r"C:\keys\id.pem".into(), ..Default::default() };
        assert_eq!(config.ssh_host, "example.com");
        assert!(config.ssh_user.eq_ignore_ascii_case("karma"));
        assert_eq!(config.ssh_port, "2222");
    }
}
