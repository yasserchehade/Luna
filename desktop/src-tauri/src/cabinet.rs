use std::{
    collections::HashSet,
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::SettingsStore;

const MAX_CABINET_SECTION_BYTES: usize = 120;
static WRITE_PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const INCOMING_FOLDER: &str = "Incoming";

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
            if section.eq_ignore_ascii_case(INCOMING_FOLDER) {
                return Err(CabinetError::InvalidSectionName(section.to_owned()));
            }
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
        preview: CabinetPreview,
    ) -> Result<CabinetConfiguration, CabinetError> {
        let preview = self.preview(&preview.root, &preview.sections)?;
        verify_writable(&preview.root)?;
        let mut folders = preview.sections.clone();
        if !folders.iter().any(|folder| folder == INCOMING_FOLDER) {
            folders.push(INCOMING_FOLDER.to_owned());
        }
        let mut created = Vec::new();
        for folder in &folders {
            let folder_path = preview.root.join(folder);
            if folder_path.is_dir() {
                continue;
            }
            if let Err(error) = std::fs::create_dir(&folder_path) {
                rollback_created_sections(&mut created);
                return Err(error.into());
            }
            created.push(folder_path);
        }
        for section in &preview.sections {
            let section_path = preview.root.join(section);
            if !directory_stays_within(&preview.root, &section_path) {
                rollback_created_sections(&mut created);
                return Err(CabinetError::UnsafeSectionTarget(section.to_owned()));
            }
            if let Err(error) = verify_writable(&section_path) {
                rollback_created_sections(&mut created);
                return Err(error);
            }
        }
        if let Err(error) = ensure_incoming_folder(&preview.root) {
            rollback_created_sections(&mut created);
            return Err(error);
        }
        let configuration = CabinetConfiguration {
            root: preview.root,
            sections: preview.sections,
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
            let available = configuration
                .sections
                .iter()
                .all(|section| validate_section_name(section).is_ok())
                && configuration.root.is_dir()
                && configuration.sections.iter().all(|section| {
                    let section = configuration.root.join(section);
                    directory_stays_within(&configuration.root, &section)
                        && verify_writable(&section).is_ok()
                })
                && verify_writable(&configuration.root).is_ok()
                && ensure_incoming_folder(&configuration.root).is_ok();
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CabinetConfiguration {
    pub root: PathBuf,
    pub sections: Vec<String>,
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
    #[error("'{0}' points outside the selected cabinet")]
    UnsafeSectionTarget(String),
    #[error("the selected cabinet location is not writable")]
    NotWritable,
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

fn directory_stays_within(root: &Path, directory: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(directory) = directory.canonicalize() else {
        return false;
    };
    directory.is_dir() && directory.starts_with(root)
}

fn ensure_incoming_folder(root: &Path) -> Result<(), CabinetError> {
    let incoming = root.join(INCOMING_FOLDER);
    if !incoming.exists() {
        std::fs::create_dir(&incoming)?;
    }
    if !directory_stays_within(root, &incoming) {
        return Err(CabinetError::UnsafeSectionTarget(
            INCOMING_FOLDER.to_owned(),
        ));
    }
    verify_writable(&incoming)
}

fn verify_writable(root: &Path) -> Result<(), CabinetError> {
    let sequence = WRITE_PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let probe = root.join(format!(
        ".luna-write-probe-{}-{sequence}",
        std::process::id()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| CabinetError::NotWritable)?;
    drop(file);
    std::fs::remove_file(probe).map_err(|_| CabinetError::NotWritable)
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
        || section.len() > MAX_CABINET_SECTION_BYTES
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
