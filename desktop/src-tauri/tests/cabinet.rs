use std::fs;

use luna_core::{CabinetAvailability, CabinetManager, CabinetStorage, SettingsStore};

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
        .create(
            "rivera-household",
            CabinetStorage::CloudSynchronized,
            preview,
        )
        .expect("create cabinet");

    assert!(cabinet_root.join("Bills & Services").is_dir());
    assert!(cabinet_root.join("Identity").is_dir());
    let reopened =
        CabinetManager::new(SettingsStore::open(&database).expect("reopen device settings"));
    assert_eq!(
        reopened
            .load("rivera-household")
            .expect("load cabinet after restart"),
        Some(configured),
    );
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
    assert!(!device_directory.path().join("Outside").exists());
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

    assert!(manager
        .create("rivera-household", CabinetStorage::Local, preview)
        .is_err());
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
        .create("rivera-household", CabinetStorage::Local, preview)
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
