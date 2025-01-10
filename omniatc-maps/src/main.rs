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
