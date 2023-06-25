use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct StreamRedirection {
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Deserialize)]
pub struct EnvironmentalVariable {
    pub name: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub cwd: String,
    pub command: Vec<String>,
    pub env: Vec<EnvironmentalVariable>,
    pub streams: StreamRedirection,
    pub poll_interval_ms: u64,
    pub run_timeout_sec: u64,
    pub grace_period_sec: u64,
}

pub fn load_json(path: &str) -> Result<Config, String> {
    let content = match fs::read_to_string(path) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!(
                "Failed to read config file. Reason - {}",
                e.to_string()
            ))
        }
    };

    let config: Config = match serde_json::from_str(&content) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!(
                "Failed to parse config file. Reason - {}",
                e.to_string()
            ))
        }
    };

    Ok(config)
}
