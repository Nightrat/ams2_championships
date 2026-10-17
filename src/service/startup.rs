//! Bringing the app up: config, the saves folder, the active career, and the threads that watch
//! the game. Shared by every executable, so the HTTP server and the native GUI start identically.

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use super::{seal_career, App};
use crate::career::{data_store, saves};
use crate::config::Config;
use crate::game::{recorder, spotter};

/// A running app. Keep `lock` alive for as long as the process runs: dropping it hands the saves
/// folder to whoever asks next.
pub struct Startup {
    pub app: App,
    /// The config as it stood at startup. Settings read only here — the address to serve on, the
    /// poll rate, which sessions to record — need a restart to change.
    pub config: Config,
    pub lock: saves::FolderLock,
}

/// Starts the app from the executable's folder, where `config.json` lives.
///
/// Refuses when another AMS2 Career program already owns the saves folder — see
/// [`saves::lock_folder`]. The lock is taken before anything is written there: the load-time
/// upgrades in [`seal_career`] write the career, and two processes doing that at once is exactly
/// what the lock is for.
pub fn startup(exe_dir: &Path) -> Result<Startup, String> {
    let config_path = exe_dir.join("config.json");

    // The saves folder is configurable, so it has to be resolved before anything under it.
    let first_look = crate::config::load_or_create(&config_path);
    let champ_dir = saves::resolve_dir(exe_dir, first_look.saves_dir.as_deref());
    std::fs::create_dir_all(&champ_dir).map_err(|e| {
        format!("Failed to create saves directory {}: {e}", champ_dir.display())
    })?;
    let lock = saves::lock_folder(&champ_dir)?;

    // The one place the config file is rewritten to pick up fields added since the last run.
    // Doing that on every read is what left the file momentarily empty for other threads.
    let cfg = crate::config::load_and_upgrade(&config_path);

    println!("Saves folder:   {}", champ_dir.display());
    // A delete cannot always finish inside the run that made the folder — see
    // `saves::sweep_empty_husks`. A fresh process can, and this is one. Swept *before*
    // `track_layouts` is made: an empty one is exactly what the sweep removes, and it used to be
    // created and then swept away in the same startup.
    match saves::sweep_empty_husks(&champ_dir) {
        0 => {}
        n => println!("Cleared {n} empty folder(s) left by a previous delete."),
    }
    let layouts_dir = champ_dir.join("track_layouts");
    std::fs::create_dir_all(&layouts_dir)
        .map_err(|e| format!("Failed to create track_layouts directory: {e}"))?;

    // `None` when the folder holds no careers: the app runs without one rather than inventing a
    // save the user never asked for, and the switcher asks them to create the first. Carried as
    // an empty path, which `App::active_path` and the recorder already yield on a poisoned lock.
    let career_path =
        saves::resolve_active(&champ_dir, cfg.active_career.as_deref()).unwrap_or_default();

    let store = data_store::load_store(&career_path);
    // A career finished before payouts were sealed still has them derived. Stamp them now,
    // while the economy in force is still the one those seasons were paid under.
    if let Err(e) = seal_career(&store, &career_path, &config_path) {
        eprintln!("Could not seal finished seasons: {e}");
    }
    if career_path.as_os_str().is_empty() {
        println!("Career data:    none yet — create a career in the app before racing");
    } else {
        let data = store.read().unwrap();
        println!(
            "Career data:    {} ({} championship(s), {} session(s))",
            career_path.display(),
            data.championships.len(),
            data.sessions.len()
        );
    }

    // Shared so that switching saves at runtime repoints the recorder thread too.
    let data_path: data_store::SavePath = Arc::new(RwLock::new(career_path));
    recorder::start(
        store.clone(),
        data_path.clone(),
        cfg.record_practice,
        cfg.record_qualify,
        cfg.record_race,
    );
    let spotter_focus: spotter::Focus = Arc::new(Mutex::new(spotter::SpotterConfig {
        enabled: cfg.spotter_enabled,
        voice: cfg.spotter_voice.clone(),
        name: cfg.spotter_name.clone(),
    }));
    spotter::start(cfg.poll_ms, spotter_focus.clone());

    Ok(Startup {
        app: App::new(
            store,
            data_path,
            champ_dir,
            layouts_dir,
            config_path,
            spotter_focus,
        ),
        config: cfg,
        lock,
    })
}

/// The folder the running executable sits in — where `config.json` and, by default, the saves
/// folder live.
pub fn exe_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}
