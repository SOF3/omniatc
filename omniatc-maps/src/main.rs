use std::path::PathBuf;

use anyhow::Result;

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
        Command::JsonSchema { output, gzip } => omniatc_maps::json_schema(&output, gzip),
        Command::FromJson { input, output } => omniatc_maps::from_json(&input, &output),
        Command::ToJson { input, output } => omniatc_maps::to_json(&input, &output),
        Command::BuildAssets { maps_dir: output_dir } => omniatc_maps::build_assets(&output_dir),
    }
}
