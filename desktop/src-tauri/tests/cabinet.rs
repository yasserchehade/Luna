use std::{fs, path::Path};

use luna_core::{CabinetAvailability, CabinetManager, SettingsStore};

#[test]
fn previewing_a_cabinet_does_not_change_the_selected_folder() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    let manager = CabinetManager::new(
        SettingsStore::open(device_directory.path().join("luna.db")).expect("open device settings"),
    );

    let preview = manager
        .preview(
            &cabinet_root,
            &["Bills & Services".to_owned(), "Identity".to_owned()],
        )
        .expect("preview cabinet");

    assert_eq!(preview.root, cabinet_root);
    assert_eq!(
        preview.sections,
        vec!["Bills & Services".to_owned(), "Identity".to_owned()]
    );
    assert_eq!(
        fs::read_dir(&cabinet_root)
            .expect("read selected folder")
            .count(),
        0,
        "previewing must not create cabinet folders",
    );
}

#[test]
fn confirming_a_preview_creates_readable_folders_and_remembers_the_cabinet() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let database = device_directory.path().join("luna.db");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    let manager =
        CabinetManager::new(SettingsStore::open(&database).expect("open device settings"));
    let preview = manager
        .preview(
            &cabinet_root,
            &["Bills & Services".to_owned(), "Identity".to_owned()],
        )
        .expect("preview cabinet");

    let configured = manager
        .create("rivera-household", preview)
        .expect("create cabinet");

    assert!(cabinet_root.join("Bills & Services").is_dir());
    assert!(cabinet_root.join("Identity").is_dir());
    let reopened = CabinetManager::new(
        SettingsStore::open(&database).expect("reopen device settings after restart"),
    );
    let validation = reopened
        .validate("rivera-household")
        .expect("validate cabinet after restart")
        .expect("remembered cabinet after restart");
    assert_eq!(validation.configuration, configured);
    assert_eq!(validation.availability, CabinetAvailability::Ready);
}

#[test]
fn cabinet_sections_cannot_escape_the_selected_folder() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    let manager = CabinetManager::new(
        SettingsStore::open(device_directory.path().join("luna.db")).expect("open device settings"),
    );

    assert!(manager
        .preview(&cabinet_root, &["../Outside".to_owned()])
        .is_err());
    assert!(manager
        .preview(&cabinet_root, &["Legal/Contracts".to_owned()])
        .is_err());
    assert!(manager.preview(&cabinet_root, &["CON".to_owned()]).is_err());
    assert!(manager
        .preview(&cabinet_root, &["Legal".to_owned(), "legal".to_owned()])
        .is_err());
    assert!(manager.preview(&cabinet_root, &["a".repeat(121)]).is_err());
    assert!(!device_directory.path().join("Outside").exists());
}

#[test]
fn a_remembered_cabinet_with_denied_write_access_is_unavailable() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    let manager = CabinetManager::new(
        SettingsStore::open(device_directory.path().join("luna.db")).expect("open device settings"),
    );
    let preview = manager
        .preview(&cabinet_root, &["Bills & Services".to_owned()])
        .expect("preview cabinet");
    manager
        .create("rivera-household", preview)
        .expect("create cabinet before access is denied");
    let denied_access = deny_write_access(&cabinet_root);

    let validation = manager
        .validate("rivera-household")
        .expect("validate cabinet with denied access")
        .expect("remembered cabinet");

    assert_eq!(validation.availability, CabinetAvailability::Unavailable);
    drop(denied_access);
}

#[test]
fn a_remembered_cabinet_with_a_read_only_section_is_unavailable() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    let manager = CabinetManager::new(
        SettingsStore::open(device_directory.path().join("luna.db")).expect("open device settings"),
    );
    let preview = manager
        .preview(&cabinet_root, &["Bills & Services".to_owned()])
        .expect("preview cabinet");
    manager
        .create("rivera-household", preview)
        .expect("create cabinet before section access is denied");
    let denied_access = deny_write_access(&cabinet_root.join("Bills & Services"));

    let validation = manager
        .validate("rivera-household")
        .expect("validate cabinet with a read-only section")
        .expect("remembered cabinet");

    assert_eq!(validation.availability, CabinetAvailability::Unavailable);
    drop(denied_access);
}

#[test]
fn a_failed_cabinet_creation_is_not_saved_or_left_half_created() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    fs::write(cabinet_root.join("Blocked"), b"existing file").expect("blocking file");
    let manager = CabinetManager::new(
        SettingsStore::open(device_directory.path().join("luna.db")).expect("open device settings"),
    );
    let preview = manager
        .preview(
            &cabinet_root,
            &["Created first".to_owned(), "Blocked".to_owned()],
        )
        .expect("preview cabinet");

    assert!(manager.create("rivera-household", preview).is_err());
    assert!(!cabinet_root.join("Created first").exists());
    assert_eq!(
        manager
            .load("rivera-household")
            .expect("load cabinet after failure"),
        None,
    );
}

#[test]
fn a_remembered_cabinet_is_reported_unavailable_without_being_redirected() {
    let device_directory = tempfile::tempdir().expect("temporary device directory");
    let cabinet_root = device_directory.path().join("Rivera Household Cabinet");
    fs::create_dir(&cabinet_root).expect("selected cabinet folder");
    let manager = CabinetManager::new(
        SettingsStore::open(device_directory.path().join("luna.db")).expect("open device settings"),
    );
    let preview = manager
        .preview(&cabinet_root, &["Bills & Services".to_owned()])
        .expect("preview cabinet");
    manager
        .create("rivera-household", preview)
        .expect("create cabinet");
    fs::remove_dir_all(&cabinet_root).expect("make cabinet unavailable");

    let validation = manager
        .validate("rivera-household")
        .expect("validate remembered cabinet")
        .expect("remembered cabinet");

    assert_eq!(validation.configuration.root, cabinet_root);
    assert_eq!(validation.availability, CabinetAvailability::Unavailable);
    assert!(
        !cabinet_root.exists(),
        "validation must not recreate the cabinet"
    );
}

#[cfg(unix)]
struct DeniedAccess {
    path: std::path::PathBuf,
    original: fs::Permissions,
}

#[cfg(unix)]
impl Drop for DeniedAccess {
    fn drop(&mut self) {
        fs::set_permissions(&self.path, self.original.clone())
            .expect("restore cabinet permissions");
    }
}

#[cfg(unix)]
fn deny_write_access(path: &Path) -> DeniedAccess {
    use std::os::unix::fs::PermissionsExt;

    let original = fs::metadata(path).expect("cabinet metadata").permissions();
    let mut denied = original.clone();
    denied.set_mode(0o555);
    fs::set_permissions(path, denied).expect("deny cabinet write access");
    DeniedAccess {
        path: path.to_owned(),
        original,
    }
}

#[cfg(windows)]
struct DeniedAccess {
    path: std::path::PathBuf,
    principal: String,
}

#[cfg(windows)]
impl Drop for DeniedAccess {
    fn drop(&mut self) {
        let status = std::process::Command::new("icacls")
            .arg(&self.path)
            .arg("/remove:d")
            .arg(&self.principal)
            .status()
            .expect("restore cabinet access");
        assert!(status.success(), "restore cabinet access control list");
    }
}

#[cfg(windows)]
fn deny_write_access(path: &Path) -> DeniedAccess {
    let output = std::process::Command::new("whoami")
        .output()
        .expect("read current Windows principal");
    assert!(output.status.success(), "read current Windows principal");
    let principal = String::from_utf8(output.stdout)
        .expect("Windows principal is UTF-8")
        .trim()
        .to_owned();
    let status = std::process::Command::new("icacls")
        .arg(path)
        .arg("/deny")
        .arg(format!("{principal}:(W)"))
        .status()
        .expect("deny cabinet write access");
    assert!(status.success(), "deny cabinet write access control list");
    DeniedAccess {
        path: path.to_owned(),
        principal,
    }
}
