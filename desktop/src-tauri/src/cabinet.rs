use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::SettingsStore;

#[derive(Clone)]
pub struct CabinetManager {
    settings: SettingsStore,
}

impl CabinetManager {
    pub fn new(settings: SettingsStore) -> Self {
        Self { settings }
    }

    pub fn preview(
        &self,
        root: impl AsRef<Path>,
        sections: &[String],
    ) -> Result<CabinetPreview, CabinetError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(CabinetError::NotDirectory);
        }
        let mut seen = HashSet::new();
        for section in sections {
            validate_section_name(section)?;
            if !seen.insert(section.to_lowercase()) {
                return Err(CabinetError::DuplicateSectionName(section.to_owned()));
            }
        }
        Ok(CabinetPreview {
            root: root.to_owned(),
            sections: sections.to_vec(),
        })
    }

    pub fn create(
        &self,
        household_id: &str,
        storage: CabinetStorage,
        preview: CabinetPreview,
    ) -> Result<CabinetConfiguration, CabinetError> {
        let preview = self.preview(&preview.root, &preview.sections)?;
        let mut created = Vec::new();
        for section in &preview.sections {
            let section_path = preview.root.join(section);
            if section_path.is_dir() {
                continue;
            }
            if let Err(error) = std::fs::create_dir(&section_path) {
                rollback_created_sections(&mut created);
                return Err(error.into());
            }
            created.push(section_path);
        }
        let configuration = CabinetConfiguration {
            root: preview.root,
            sections: preview.sections,
            storage,
        };
        let stored = serde_json::to_string(&configuration)?;
        if let Err(error) = self.settings.set(&configuration_key(household_id), &stored) {
            rollback_created_sections(&mut created);
            return Err(error.into());
        }
        Ok(configuration)
    }

    pub fn load(&self, household_id: &str) -> Result<Option<CabinetConfiguration>, CabinetError> {
        self.settings
            .get(&configuration_key(household_id))?
            .map(|stored| serde_json::from_str(&stored).map_err(CabinetError::from))
            .transpose()
    }

    pub fn validate(&self, household_id: &str) -> Result<Option<CabinetValidation>, CabinetError> {
        Ok(self.load(household_id)?.map(|configuration| {
            let available = configuration.root.is_dir()
                && configuration
                    .sections
                    .iter()
                    .all(|section| configuration.root.join(section).is_dir());
            CabinetValidation {
                configuration,
                availability: if available {
                    CabinetAvailability::Ready
                } else {
                    CabinetAvailability::Unavailable
                },
            }
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CabinetPreview {
    pub root: PathBuf,
    pub sections: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CabinetStorage {
    CloudSynchronized,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CabinetConfiguration {
    pub root: PathBuf,
    pub sections: Vec<String>,
    pub storage: CabinetStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CabinetAvailability {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CabinetValidation {
    pub configuration: CabinetConfiguration,
    pub availability: CabinetAvailability,
}

#[derive(Debug, Error)]
pub enum CabinetError {
    #[error("the selected cabinet location is not a folder")]
    NotDirectory,
    #[error("'{0}' is not a safe cabinet section name")]
    InvalidSectionName(String),
    #[error("'{0}' duplicates another cabinet section")]
    DuplicateSectionName(String),
    #[error("the cabinet filesystem operation failed")]
    Filesystem(#[from] std::io::Error),
    #[error("the cabinet setting could not be stored")]
    Settings(#[from] rusqlite::Error),
    #[error("the stored cabinet configuration is invalid")]
    InvalidConfiguration(#[from] serde_json::Error),
}

fn configuration_key(household_id: &str) -> String {
    format!("cabinet:{household_id}")
}

fn rollback_created_sections(created: &mut Vec<PathBuf>) {
    for path in created.drain(..).rev() {
        let _ = std::fs::remove_dir(path);
    }
}

fn validate_section_name(section: &str) -> Result<(), CabinetError> {
    let trimmed = section.trim();
    let file_stem = trimmed.split('.').next().unwrap_or_default();
    let reserved_windows_name = matches!(
        file_stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if trimmed.is_empty()
        || trimmed != section
        || trimmed == "."
        || trimmed == ".."
        || trimmed.ends_with(['.', ' '])
        || trimmed
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
        || reserved_windows_name
    {
        return Err(CabinetError::InvalidSectionName(section.to_owned()));
    }
    Ok(())
}
