//! Build script: generate the `worldcup26` man page and shell completions.
//!
//! The artifacts are written to the build-script `OUT_DIR` and shipped in the
//! Debian package (see `[package.metadata.deb]` in `Cargo.toml`). The CLI
//! definition is `include!`d from `src/cli.rs` so the command stays a single
//! source of truth rather than being duplicated here.
//!
//! Each artifact is written under its canonical install name so that the deb's
//! glob-sourced assets (which preserve the file's basename) land at the right
//! path — notably the bash completion is `worldcup26`, not `worldcup26.bash`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs::File;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::{Shell, generate};
use clap_mangen::Man;

// Bring the `Cli` parser into scope without depending on the binary crate.
#[path = "src/cli.rs"]
mod cli;

fn main() {
    // Only the CLI surface affects the generated artifacts.
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));

    let mut command = cli::Cli::command();
    command.set_bin_name("worldcup26");

    let man = Man::new(command.clone());
    let mut man_page = Vec::new();
    man.render(&mut man_page)
        .expect("render worldcup26 man page");
    std::fs::write(out_dir.join("worldcup26.1"), man_page).expect("write worldcup26.1");

    for (shell, file_name) in [
        (Shell::Bash, "worldcup26"),
        (Shell::Zsh, "_worldcup26"),
        (Shell::Fish, "worldcup26.fish"),
    ] {
        let mut file = File::create(out_dir.join(file_name)).expect("create completion file");
        generate(shell, &mut command, "worldcup26", &mut file);
    }
}
