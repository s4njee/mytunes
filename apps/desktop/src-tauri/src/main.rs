// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    if let Err(error) = prepare_wayland() {
        eprintln!("could not restart MyTunes with compatible GTK modules: {error}");
        std::process::exit(1);
    }
    mytunes_desktop_lib::run();
}

#[cfg(target_os = "linux")]
fn prepare_wayland() -> std::io::Result<()> {
    use std::{env, os::unix::process::CommandExt, process::Command};

    let wayland = match env::var("GDK_BACKEND").ok().as_deref() {
        Some("wayland") => true,
        Some("x11") => false,
        _ => env::var_os("WAYLAND_DISPLAY").is_some(),
    };
    if !wayland {
        return Ok(());
    }
    // GTK 3 can recurse while mapping a menu with this module on Wayland.
    // Re-exec before GTK starts to change only this process's module list.
    // See https://bugs.debian.org/1107671.
    let mut overrides = Vec::new();
    for variable in ["GTK_MODULES", "GTK3_MODULES"] {
        if let Ok(value) = env::var(variable) {
            let filtered = value
                .split(':')
                .filter(|module| !matches!(*module, "appmenu-gtk-module" | "appmenu-gtk3-module"))
                .collect::<Vec<_>>()
                .join(":");
            if filtered != value {
                overrides.push((variable, filtered));
            }
        }
    }
    if overrides.is_empty() {
        return Ok(());
    }
    Err(Command::new(env::current_exe()?)
        .args(env::args_os().skip(1))
        .envs(overrides)
        .exec())
}
