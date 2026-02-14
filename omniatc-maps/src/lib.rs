#![allow(clippy::too_many_lines, reason = "we have enormous struct literals")]

use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::{fmt, fs, io};

use anyhow::{Context, Result};

pub mod common_types;

pub mod tutorial;

pub fn builtins()
-> impl Iterator<Item = (impl AsRef<str> + Into<String> + fmt::Display, store::File)> {
    [("tutorial", tutorial::file())].into_iter()
}

pub fn build_assets(maps_dir: &Path) -> Result<()> {
    if let Err(err) = fs::create_dir(maps_dir)
        && err.kind() != io::ErrorKind::AlreadyExists
    {
        return Err(err).context("mkdir maps");
    }

    for (name, data) in builtins() {
        ciborium::into_writer(
            &data,
            fs::File::create(maps_dir.join(format!("{name}.osav")))
                .with_context(|| format!("create {name}.osav"))?,
        )
        .with_context(|| format!("write {name}.osav"))?;
    }

    Ok(())
}

pub fn json_schema(output: &Path, gzip: bool) -> Result<()> {
    let schema = schemars::schema_for!(store::File);
    let mut writer: Box<dyn io::Write> =
        Box::new(BufWriter::new(fs::File::create(output).context("create output")?));
    if gzip {
        writer = Box::new(BufWriter::new(flate2::write::GzEncoder::new(
            writer,
            flate2::Compression::best(),
        )));
    }
    serde_json::to_writer(writer, &schema).context("write schema")?;
    Ok(())
}

pub fn from_json(input: &Path, output: &Path) -> Result<()> {
    let file: store::File =
        serde_json::from_reader(BufReader::new(fs::File::open(input).context("open input")?))
            .context("parse json")?;
    ciborium::into_writer(
        &file,
        BufWriter::new(fs::File::create(output).context("create output")?),
    )
    .context("write osav")?;
    Ok(())
}

pub fn to_json(input: &Path, output: &Path) -> Result<()> {
    let file: store::File =
        ciborium::de::from_reader(BufReader::new(fs::File::open(input).context("open input")?))
            .context("parse osav")?;
    serde_json::to_writer_pretty(
        BufWriter::new(fs::File::create(output).context("create output")?),
        &file,
    )
    .context("write json")?;
    Ok(())
}
