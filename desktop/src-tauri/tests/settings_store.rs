use luna_core::SettingsStore;

#[test]
fn a_device_setting_survives_reopening_the_local_database() {
    let directory = tempfile::tempdir().expect("temporary device directory");
    let database = directory.path().join("luna.db");

    SettingsStore::open(&database)
        .expect("open settings store")
        .set("brief.schedule", "weekly")
        .expect("save setting");

    let reopened = SettingsStore::open(&database).expect("reopen settings store");
    assert_eq!(
        reopened.get("brief.schedule").expect("load setting"),
        Some("weekly".to_owned())
    );
}
