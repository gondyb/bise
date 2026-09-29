use super::*;
use std::collections::HashMap;
use serde_json::Value;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-home-test-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn home_of(pairs: &[(&str, &str)]) -> Home {
    let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Home::from_lookup(&move |k: &str| m.get(k).cloned())
}

#[test]
fn the_old_layout_keeps_todays_paths() {
    let h = home_of(&[("HOME", "/h"), ("XDG_STATE_HOME", "/xdg")]);
    assert_eq!(h.layout(), Layout::Legacy);
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(h.root(), p("/h/.bend-harness"));
    assert_eq!(h.config_file(), p("/h/.bend-harness/config.toml"));
    assert_eq!(h.key_file(), p("/h/.bend-harness/.env"));
    assert_eq!(h.env_files(), vec![p("/h/.bend-harness/.env"), p("/h/.vibe/.env")]);
    assert_eq!(h.sessions_dir(), p("/h/.bend-harness/sessions"));
    // XDG_STATE_HOME is not read any more
    assert_eq!(h.hub_dir("ws-12345678"), p("/h/.local/state/switchboard/ws-12345678"));
    assert_eq!(h.images_dir(), p("/h/.bend-harness/images"));
    assert_eq!(h.crashes_dir(), p("/h/.bend-harness/crashes"));
    assert_eq!(h.mcp_index(), p("/h/.bend-harness/mcp-index.txt"));
    assert_eq!(h.skills_index(), p("/h/.bend-harness/skills-index.txt"));
    assert_eq!(h.plugins_state(), p("/h/.bend-harness/plugins.json"));
    assert_eq!(h.plugin_data_dir(), p("/h/.bend-harness/plugin-data"));
    assert_eq!(h.run_dir(), p("/h/.bend-harness/run"));
    assert_eq!(h.drafts_dir(), p("/h/.local/state/switchboard/drafts"));
    assert_eq!(h.versions_dir(), p("/h/.local/state/switchboard/versions"));
    assert_eq!(h.build_dir(), p("/h/.local/state/switchboard/build"));
    assert_eq!(h.pref(Pref::Theme), Slot::key("/h/.bend-harness/tui.json", "theme"));
    assert_eq!(h.pref(Pref::Voice), Slot::key("/h/.bend-harness/tui.json", "voice_mode_enabled"));
    assert_eq!(h.pref(Pref::Hints), Slot::file("/h/.local/state/switchboard/hints.json"));
    assert_eq!(h.pref(Pref::Tip), Slot::file("/h/.local/state/switchboard/tip"));
    assert_eq!(h.pref(Pref::Onboarded), Slot::file("/h/.local/state/switchboard/onboarded"));
}

#[test]
fn bise_home_moves_everything() {
    let h = home_of(&[("HOME", "/h"), ("BISE_HOME", "/b")]);
    assert_eq!(h.layout(), Layout::Bise);
    let p = |s: &str| PathBuf::from(s);
    assert_eq!(h.root(), p("/b"));
    assert_eq!(h.user_home(), p("/h"));
    assert_eq!(h.config_file(), p("/b/config.toml"));
    assert_eq!(h.auth_file(), p("/b/auth.json"));
    assert_eq!(h.env_files(), vec![p("/b/.env"), p("/h/.bend-harness/.env"), p("/h/.vibe/.env")]);
    assert_eq!(h.sessions_dir(), p("/b/sessions"));
    assert_eq!(h.hub_dir("ws-1"), p("/b/hubs/ws-1"));
    assert_eq!(h.mcp_index(), p("/b/cache/mcp-index.txt"));
    assert_eq!(h.skills_index(), p("/b/cache/skills-index.txt"));
    assert_eq!(h.run_dir(), p("/b/run"));
    assert_eq!(h.drafts_dir(), p("/b/drafts"));
    assert_eq!(h.versions_dir(), p("/b/dev/versions"));
    assert_eq!(h.build_dir(), p("/b/dev/build"));
    for pref in [Pref::Voice, Pref::Theme, Pref::Hints, Pref::Tip, Pref::Onboarded] {
        assert_eq!(h.pref(pref), Slot::key("/b/prefs.json", pref.key()));
    }
}

#[test]
fn the_migration_marker_turns_dot_bise_on() {
    let d = tmp("marker");
    let hs = d.to_string_lossy().to_string();
    assert_eq!(home_of(&[("HOME", &hs)]).layout(), Layout::Legacy);
    std::fs::create_dir_all(d.join(".bise")).unwrap();
    assert_eq!(home_of(&[("HOME", &hs)]).layout(), Layout::Legacy, "an empty ~/.bise is not enough");
    std::fs::write(d.join(".bise").join(MIGRATED), "{}").unwrap();
    let h = home_of(&[("HOME", &hs)]);
    assert_eq!((h.layout(), h.root()), (Layout::Bise, d.join(".bise").as_path()));
    assert_eq!(h.hubs_dir(), d.join(".bise/hubs"));
}

#[test]
fn no_home_falls_back_to_tmp() {
    let h = home_of(&[]);
    assert_eq!(h.config_file(), PathBuf::from("/tmp/.bend-harness/config.toml"));
    assert_eq!(home_of(&[("HOME", "")]).root(), Path::new("/tmp/.bend-harness"));
}

#[test]
fn overrides_win_and_are_exported() {
    let h = home_of(&[("HOME", "/h"), ("BEND_CONFIG", "/c.toml"), ("SB_VERSIONS_DIR", "/v")]);
    assert_eq!(h.config_file(), PathBuf::from("/c.toml"));
    assert_eq!(h.versions_dir(), PathBuf::from("/v"));
    let ex: HashMap<_, _> = h.exports().into_iter().collect();
    assert_eq!(ex["BEND_CONFIG"], "/c.toml");
    assert_eq!(ex["BEND_SESSIONS_DIR"], "/h/.bend-harness/sessions");
    for k in PATH_VARS {
        assert!(ex.contains_key(k), "{k} not exported");
    }
    assert!(!ex.contains_key("BISE_HOME"), "legacy: BISE_HOME would turn the new layout on");
    let b: HashMap<_, _> = home_of(&[("HOME", "/h"), ("BISE_HOME", "/b")]).exports().into_iter().collect();
    assert_eq!(b["BISE_HOME"], "/b");
}

#[test]
fn exported_paths_follow_the_same_home_and_only_it() {
    let parent = home_of(&[("HOME", "/h")]);
    let mut env: HashMap<String, String> = parent.exports().into_iter().map(|(k, v)| (k.into(), v)).collect();
    // a child with the same HOME: the exports are read back unchanged
    env.insert("HOME".into(), "/h".into());
    let child = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(child.exports(), parent.exports());
    // a test with a temp HOME started from that shell: the inherited paths are ignored
    env.insert("HOME".into(), "/t".into());
    let test = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(test.sessions_dir(), PathBuf::from("/t/.bend-harness/sessions"));
    assert_eq!(test.config_file(), PathBuf::from("/t/.bend-harness/config.toml"));
    // same HOME, a BISE_HOME of its own: ignored too
    env.insert("HOME".into(), "/h".into());
    env.insert("BISE_HOME".into(), "/b".into());
    let moved = Home::from_lookup(&|k: &str| env.get(k).cloned());
    assert_eq!(moved.config_file(), PathBuf::from("/b/config.toml"));
    // a hand-set override without a stamp is honoured
    let own = home_of(&[("HOME", "/t"), ("BEND_CONFIG", "/mine.toml")]);
    assert_eq!(own.config_file(), PathBuf::from("/mine.toml"));
}

#[test]
fn the_run_dir_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("run");
    let h = Home::at(&d);
    std::fs::create_dir_all(h.run_dir()).unwrap();
    std::fs::set_permissions(h.run_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let r = h.ensure_run_dir().unwrap();
    assert_eq!(std::fs::metadata(r).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn prefs_share_one_file_and_keep_the_other_keys() {
    let d = tmp("prefs");
    let h = Home::at(&d);
    h.pref(Pref::Theme).set("dark".into()).unwrap();
    h.pref(Pref::Hints).set(serde_json::json!({"first_card": true})).unwrap();
    h.pref(Pref::Tip).set(3.into()).unwrap();
    assert_eq!(h.pref(Pref::Theme).get(), Some("dark".into()));
    assert_eq!(h.pref(Pref::Tip).get(), Some(3.into()));
    assert_eq!(h.pref(Pref::Onboarded).get(), None);
    let all: Value = serde_json::from_str(&std::fs::read_to_string(h.prefs_file()).unwrap()).unwrap();
    assert_eq!(all["hints"]["first_card"], true);
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "no tmp file left");
}

#[test]
fn legacy_prefs_read_the_old_files() {
    let d = tmp("legacy-prefs");
    let hs = d.to_string_lossy().to_string();
    let st = d.join(".local/state/switchboard");
    std::fs::create_dir_all(&st).unwrap();
    std::fs::create_dir_all(d.join(".bend-harness")).unwrap();
    std::fs::write(st.join("onboarded"), "1\n").unwrap();
    std::fs::write(st.join("tip"), "4\n").unwrap();
    std::fs::write(d.join(".bend-harness/tui.json"), r#"{"voice_mode_enabled": true}"#).unwrap();
    let h = home_of(&[("HOME", &hs)]);
    assert_eq!(h.pref(Pref::Onboarded).get(), Some(1.into()));
    assert_eq!(h.pref(Pref::Tip).get(), Some(4.into()));
    assert_eq!(h.pref(Pref::Voice).get(), Some(true.into()));
    h.pref(Pref::Theme).set("light".into()).unwrap();
    let tui: Value = serde_json::from_str(&std::fs::read_to_string(d.join(".bend-harness/tui.json")).unwrap()).unwrap();
    assert_eq!((tui["voice_mode_enabled"].clone(), tui["theme"].clone()), (true.into(), "light".into()));
    // a flag file that is not JSON still reads as set
    std::fs::write(st.join("onboarded"), "yes\n").unwrap();
    assert_eq!(h.pref(Pref::Onboarded).get(), Some("yes".into()));
}
