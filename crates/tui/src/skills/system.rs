//! System-skill installer: bundles skill-creator and delegate, auto-installs
//! them on first launch.

use std::fs;
use std::path::Path;

const BUNDLED_SKILL_VERSION: &str = "2";
const SKILL_CREATOR_BODY: &str = include_str!("../../assets/skills/skill-creator/SKILL.md");
const DELEGATE_BODY: &str = include_str!("../../assets/skills/delegate/SKILL.md");

/// Attempt to install a single bundled skill into `skills_dir`.
///
/// Returns `true` if installation occurred (fresh install or version bump).
fn install_one(
    skills_dir: &Path,
    name: &str,
    body: &str,
    installed_version: Option<&str>,
) -> std::io::Result<bool> {
    let target_dir = skills_dir.join(name);
    let target_file = target_dir.join("SKILL.md");
    let dir_exists = target_dir.exists();

    // Re-install only when BOTH conditions hold:
    //   (a) bundled version is newer than what is recorded in the marker, AND
    //   (b) the skill directory still exists (user hasn't intentionally deleted it).
    // Fresh install (no marker AND no dir) is also handled.
    let should_install = match (installed_version, dir_exists) {
        // Fresh install: neither marker nor directory.
        (None, false) => true,
        // Version bump: marker is outdated but directory still present.
        (Some(v), _) if v != BUNDLED_SKILL_VERSION => true,
        // Every other case: already installed at current version, or user deleted
        // the dir (respect that choice).
        _ => false,
    };

    if should_install {
        fs::create_dir_all(&target_dir)?;
        fs::write(&target_file, body)?;
    }
    Ok(should_install)
}

/// Install bundled system skills into `skills_dir`.
///
/// Behaviour:
/// - Fresh install (no marker, no dir): installs `skill-creator/SKILL.md` and
///   `delegate/SKILL.md`, then writes the version marker.
/// - Version bump (marker present with older version): re-installs any skill
///   whose directory still exists.
/// - User deleted a skill dir while marker still present at same version: leaves
///   it gone.
/// - Idempotent: calling twice with no changes is a no-op.
///
/// Errors are I/O errors from the filesystem; the caller should log them but not
/// abort startup.
pub fn install_system_skills(skills_dir: &Path) -> std::io::Result<()> {
    let marker = skills_dir.join(".system-installed-version");

    let installed_version = fs::read_to_string(&marker)
        .ok()
        .map(|s| s.trim().to_string());

    let sc_installed = install_one(
        skills_dir,
        "skill-creator",
        SKILL_CREATOR_BODY,
        installed_version.as_deref(),
    )?;
    let dg_installed = install_one(
        skills_dir,
        "delegate",
        DELEGATE_BODY,
        installed_version.as_deref(),
    )?;

    if sc_installed || dg_installed {
        fs::create_dir_all(skills_dir)?;
        fs::write(&marker, BUNDLED_SKILL_VERSION)?;
    }
    Ok(())
}

/// Remove all system skills and the version marker.
///
/// Intended for tests and `deepseek setup --clean`.  Ignores missing files.
#[allow(dead_code)]
pub fn uninstall_system_skills(skills_dir: &Path) -> std::io::Result<()> {
    let marker = skills_dir.join(".system-installed-version");

    for name in &["skill-creator", "delegate"] {
        let dir = skills_dir.join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
    }
    if marker.exists() {
        fs::remove_file(&marker)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn sc_file(tmp: &TempDir) -> std::path::PathBuf {
        tmp.path().join("skill-creator").join("SKILL.md")
    }

    fn dg_file(tmp: &TempDir) -> std::path::PathBuf {
        tmp.path().join("delegate").join("SKILL.md")
    }

    fn sc_dir(tmp: &TempDir) -> std::path::PathBuf {
        tmp.path().join("skill-creator")
    }

    fn dg_dir(tmp: &TempDir) -> std::path::PathBuf {
        tmp.path().join("delegate")
    }

    fn marker_file(tmp: &TempDir) -> std::path::PathBuf {
        tmp.path().join(".system-installed-version")
    }

    // ── fresh install ─────────────────────────────────────────────────────────

    #[test]
    fn fresh_install_creates_both_skills_and_marker() {
        let tmp = TempDir::new().unwrap();
        install_system_skills(tmp.path()).unwrap();

        assert!(sc_file(&tmp).exists(), "skill-creator SKILL.md should be created");
        assert!(dg_file(&tmp).exists(), "delegate SKILL.md should be created");
        assert!(marker_file(&tmp).exists(), "marker should be created");

        let ver = fs::read_to_string(marker_file(&tmp)).unwrap();
        assert_eq!(ver.trim(), BUNDLED_SKILL_VERSION);
    }

    // ── idempotence ───────────────────────────────────────────────────────────

    #[test]
    fn calling_twice_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        install_system_skills(tmp.path()).unwrap();

        // Overwrite both SKILL.md files with sentinels to detect undesired writes.
        fs::write(sc_file(&tmp), "sc-sentinel").unwrap();
        fs::write(dg_file(&tmp), "dg-sentinel").unwrap();

        install_system_skills(tmp.path()).unwrap();

        let sc = fs::read_to_string(sc_file(&tmp)).unwrap();
        let dg = fs::read_to_string(dg_file(&tmp)).unwrap();
        assert_eq!(sc, "sc-sentinel", "second install should not overwrite skill-creator");
        assert_eq!(dg, "dg-sentinel", "second install should not overwrite delegate");
    }

    // ── user deleted a directory ──────────────────────────────────────────────

    #[test]
    fn user_deleted_dir_is_not_recreated() {
        let tmp = TempDir::new().unwrap();
        install_system_skills(tmp.path()).unwrap();

        // Simulate user deliberately removing one skill directory.
        fs::remove_dir_all(dg_dir(&tmp)).unwrap();

        // Re-launch must NOT recreate the deleted directory.
        install_system_skills(tmp.path()).unwrap();

        assert!(
            !dg_file(&tmp).exists(),
            "delegate must not be recreated after user deleted it"
        );
        assert!(
            sc_file(&tmp).exists(),
            "skill-creator should still be present (not deleted by user)"
        );
    }

    #[test]
    fn user_deleted_both_dirs_are_not_recreated() {
        let tmp = TempDir::new().unwrap();
        install_system_skills(tmp.path()).unwrap();

        fs::remove_dir_all(sc_dir(&tmp)).unwrap();
        fs::remove_dir_all(dg_dir(&tmp)).unwrap();

        install_system_skills(tmp.path()).unwrap();

        assert!(!sc_file(&tmp).exists());
        assert!(!dg_file(&tmp).exists());
    }

    // ── version bump re-installs ──────────────────────────────────────────────

    #[test]
    fn outdated_marker_triggers_reinstall_of_existing_skills() {
        let tmp = TempDir::new().unwrap();

        // Simulate a previous install at a lower version with both skills present.
        fs::create_dir_all(sc_dir(&tmp)).unwrap();
        fs::write(sc_file(&tmp), "old-sc").unwrap();
        fs::create_dir_all(dg_dir(&tmp)).unwrap();
        fs::write(dg_file(&tmp), "old-dg").unwrap();
        fs::write(marker_file(&tmp), "0").unwrap(); // older than BUNDLED_SKILL_VERSION

        install_system_skills(tmp.path()).unwrap();

        let sc = fs::read_to_string(sc_file(&tmp)).unwrap();
        let dg = fs::read_to_string(dg_file(&tmp)).unwrap();
        assert_ne!(sc, "old-sc", "outdated skill-creator should be overwritten");
        assert_ne!(dg, "old-dg", "outdated delegate should be overwritten");
        assert_eq!(sc, SKILL_CREATOR_BODY);
        assert_eq!(dg, DELEGATE_BODY);

        let ver = fs::read_to_string(marker_file(&tmp)).unwrap();
        assert_eq!(ver.trim(), BUNDLED_SKILL_VERSION);
    }

    // ── partial previous install (only skill-creator existed) ─────────────────

    #[test]
    fn version_bump_adds_delegate_when_it_was_missing() {
        let tmp = TempDir::new().unwrap();

        // Simulate state from v1: only skill-creator present.
        fs::create_dir_all(sc_dir(&tmp)).unwrap();
        fs::write(sc_file(&tmp), "old-sc").unwrap();
        fs::write(marker_file(&tmp), "1").unwrap();

        install_system_skills(tmp.path()).unwrap();

        // skill-creator should be updated, delegate should be newly installed.
        assert_eq!(fs::read_to_string(sc_file(&tmp)).unwrap(), SKILL_CREATOR_BODY);
        assert_eq!(fs::read_to_string(dg_file(&tmp)).unwrap(), DELEGATE_BODY);
    }

    // ── uninstall ─────────────────────────────────────────────────────────────

    #[test]
    fn uninstall_removes_both_skills_and_marker() {
        let tmp = TempDir::new().unwrap();
        install_system_skills(tmp.path()).unwrap();
        uninstall_system_skills(tmp.path()).unwrap();

        assert!(!sc_file(&tmp).exists(), "skill-creator should be removed");
        assert!(!dg_file(&tmp).exists(), "delegate should be removed");
        assert!(!marker_file(&tmp).exists(), "marker should be removed");
    }

    #[test]
    fn uninstall_on_clean_dir_is_a_noop() {
        let tmp = TempDir::new().unwrap();
        // Must not panic or error.
        uninstall_system_skills(tmp.path()).unwrap();
    }
}
