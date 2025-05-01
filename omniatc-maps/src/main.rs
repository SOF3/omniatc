#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic, clippy::dbg_macro))]
#![allow(clippy::too_many_lines)] // we have enormous struct literals
#![allow(clippy::collapsible_else_if)] // this is usually intentional
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_variables, unused_imports))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future

use std::path::Path;
use std::{fs, io};

use anyhow::{Context, Result};

mod example;

fn main() -> Result<()> {
    let maps_dir = Path::new("assets/maps");

    if let Err(err) = fs::create_dir(maps_dir) {
        if err.kind() != io::ErrorKind::AlreadyExists {
            Err(err).context("mkdir maps")?;
        }
    }

    ciborium::into_writer(
        &example::file(),
        fs::File::create(maps_dir.join("example.osav")).context("create example.osav")?,
    )
    .context("write example.osav")?;

    Ok(())
}
