#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic, clippy::dbg_macro))]
#![allow(clippy::too_many_lines)] // we have enormous struct literals
#![allow(clippy::collapsible_else_if)] // this is usually intentional
#![allow(clippy::missing_panics_doc)] // 5:21 PM conrad.lock().expect("luscious")[tty0] : Worst clippy lint
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_variables, unused_imports))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future

use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result};

mod example;

#[derive(clap::Parser)]
struct Options {
    #[clap(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Output the JSON schema for the map format.
    JsonSchema {
        /// Output JSON schema file.
        #[clap(default_value = "assets/schema.json.gz")]
        output: PathBuf,
        /// Gzip the output file.
        #[clap(long, default_value_t = true)]
        gzip:   std::primitive::bool,
    },
    /// Convert a JSON file to an OSAV file.
    FromJson {
        /// Input JSON file
        input:  PathBuf,
        /// Output OSAV file
        output: PathBuf,
    },
    /// Convert an OSAV file to a JSON file.
    ToJson {
        /// Input OSAV file
        input:  PathBuf,
        /// Output JSON file
        output: PathBuf,
    },
    /// Build assets/maps.
    BuildAssets {
        /// Directory to write map files to.
        #[clap(default_value = "assets/maps")]
        maps_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    match <Options as clap::Parser>::parse().command {
        Command::JsonSchema { output, gzip } => json_schema(&output, gzip),
        Command::FromJson { input, output } => from_json(&input, &output),
        Command::ToJson { input, output } => to_json(&input, &output),
        Command::BuildAssets { maps_dir: output_dir } => build_assets(&output_dir),
    }
}

fn json_schema(output: &Path, gzip: bool) -> Result<()> {
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

fn from_json(input: &Path, output: &Path) -> Result<()> {
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

fn to_json(input: &Path, output: &Path) -> Result<()> {
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

fn build_assets(maps_dir: &Path) -> Result<()> {
    if let Err(err) = fs::create_dir(maps_dir)
        && err.kind() != io::ErrorKind::AlreadyExists
    {
        return Err(err).context("mkdir maps");
    }

    ciborium::into_writer(
        &example::file(),
        fs::File::create(maps_dir.join("example.osav")).context("create example.osav")?,
    )
    .context("write example.osav")?;

    Ok(())
}
