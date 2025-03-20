use std::collections::HashMap;
use std::process::Command;
use std::io;

#[derive(Debug)]
pub struct PhpExecContext {
    pub bin_path: String,
    pub script_path: String,
    pub envs: HashMap<String, String>,   
}

impl PhpExecContext {
    pub fn new(bin_path: String, script_path: String) -> Self {
        let mut envs = HashMap::new();
        
        // Set required CGI environment variables
        envs.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
        envs.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
        envs.insert("SERVER_SOFTWARE".to_string(), "Kang/1.0".to_string());
        envs.insert("SCRIPT_FILENAME".to_string(), script_path.clone());
        envs.insert("REDIRECT_STATUS".to_string(), "200".to_string());

        PhpExecContext {
            bin_path,
            script_path,
            envs,
        }
    }

    pub fn add_env(&mut self, key: &str, value: &str) {
        self.envs.insert(key.to_string(), value.to_string());
    }

    pub fn exec(&self) -> io::Result<String> {
        let output = Command::new(&self.bin_path)
            .env_clear() // Clear existing environment
            .envs(&self.envs)
            .arg(&self.script_path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}