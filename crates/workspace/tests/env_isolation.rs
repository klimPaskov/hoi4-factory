//! One test per process: it changes environment variables, which would race with other
//! tests if it shared a binary with them.

mod common;

use std::fs;

use factory_workspace::{GitRepo, InitOptions};

use common::{Fx, bom_crlf, tgit, tgit_text};

#[test]
fn wrapper_ignores_hostile_environment_and_user_level_config() {
    let fx = Fx::new();
    fx.write("descriptor.mod", &bom_crlf(&["name=\"Fixture\""]));
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    // Undo the repository's own protection so only the wrapper's flags stand in the way.
    tgit(
        &fx.src(),
        &["config", "--local", "--unset-all", "core.autocrlf"],
    );

    let home = fx.tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join(".gitconfig"),
        "[core]\n\tautocrlf = true\n[user]\n\tname = Leaked Global\n\temail = leak@example.invalid\n[filter \"evil\"]\n\tclean = touch pwned\n",
    )
    .unwrap();
    // SAFETY: this is the only test in this binary, so no other thread reads the environment.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("USERPROFILE", &home);
        std::env::set_var("XDG_CONFIG_HOME", &home);
        std::env::set_var("GIT_AUTHOR_NAME", "Leaked Author");
        std::env::set_var("GIT_COMMITTER_EMAIL", "leak@example.invalid");
        std::env::set_var("GIT_DIR", fx.tmp.path().join("nowhere"));
        std::env::set_var("GIT_WORK_TREE", fx.tmp.path().join("nowhere"));
        std::env::set_var("GIT_INDEX_FILE", fx.tmp.path().join("nowhere-index"));
    }
    let head = repo.baseline_commit("baseline").unwrap();

    // Observe with a clean environment.
    unsafe {
        for key in [
            "GIT_AUTHOR_NAME",
            "GIT_COMMITTER_EMAIL",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
        ] {
            std::env::remove_var(key);
        }
    }
    assert_eq!(
        tgit_text(&fx.src(), &["log", "-1", "--format=%an <%ae>"]),
        "HOI4 Factory <factory@hoi4-factory.invalid>"
    );
    assert_eq!(
        tgit_text(&fx.src(), &["log", "-1", "--format=%cn <%ce>"]),
        "HOI4 Factory <factory@hoi4-factory.invalid>"
    );
    let blob = tgit(
        &fx.src(),
        &["cat-file", "blob", &format!("{head}:descriptor.mod")],
    );
    assert_eq!(
        blob,
        fx.read("descriptor.mod"),
        "global autocrlf=true must not apply"
    );
    assert!(!fx.src().join("pwned").exists());
    assert!(!fx.tmp.path().join("nowhere").exists());
}
