use std::{env, fs, path::PathBuf};

use serde::Deserialize;

pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub process_substring: String,
    pub check_interval_seconds: u64,
    pub excluded_processes: Vec<String>,
}

impl Settings {
    pub fn load() -> Result<Self, String> {
        let path = config_path()?;
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("Не удалось прочитать {}: {error}", path.display()))?;
        let mut settings: Self = toml::from_str(&contents)
            .map_err(|error| format!("Некорректный формат {}: {error}", path.display()))?;

        settings.normalize_and_validate()?;
        Ok(settings)
    }

    fn normalize_and_validate(&mut self) -> Result<(), String> {
        self.process_substring = self.process_substring.trim().to_lowercase();

        if self.process_substring.is_empty() {
            return Err("Параметр process_substring не должен быть пустым".to_owned());
        }

        if self.check_interval_seconds == 0 {
            return Err("Параметр check_interval_seconds должен быть больше нуля".to_owned());
        }

        self.excluded_processes = self
            .excluded_processes
            .iter()
            .map(|process| process.trim().to_lowercase())
            .filter(|process| !process.is_empty())
            .collect();

        Ok(())
    }
}

fn config_path() -> Result<PathBuf, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("Не удалось определить путь к приложению: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "Не удалось определить папку приложения".to_owned())?;

    Ok(directory.join(CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn normalizes_settings() {
        let mut settings = Settings {
            process_substring: " Agent ".to_owned(),
            check_interval_seconds: 1,
            excluded_processes: vec![" SSH-Agent.exe ".to_owned(), "  ".to_owned()],
        };

        settings.normalize_and_validate().unwrap();

        assert_eq!(settings.process_substring, "agent");
        assert_eq!(settings.excluded_processes, vec!["ssh-agent.exe"]);
    }

    #[test]
    fn rejects_empty_process_substring() {
        let mut settings = Settings {
            process_substring: "   ".to_owned(),
            check_interval_seconds: 1,
            excluded_processes: Vec::new(),
        };

        assert!(settings.normalize_and_validate().is_err());
    }

    #[test]
    fn rejects_zero_interval() {
        let mut settings = Settings {
            process_substring: "agent".to_owned(),
            check_interval_seconds: 0,
            excluded_processes: Vec::new(),
        };

        assert!(settings.normalize_and_validate().is_err());
    }
}
