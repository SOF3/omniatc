# omniatc

Yet another ATC simulator.

## Play this game

- [Web version](https://sof3.github.io/omniatc/index.html)
- Desktop version downloads
  - [Windows (x86_64)](https://sof3.github.io/omniatc/bin-x86_64-pc-windows-msvc/omniatc-client.exe)
  - [Windows (aarch64)](https://sof3.github.io/omniatc/bin-aarch64-pc-windows-msvc/omniatc-client.exe)
  - [MacOS (Intel)](https://sof3.github.io/omniatc/bin-x86_64-apple-darwin/omniatc-client)
  - [MacOS (Silicon)](https://sof3.github.io/omniatc/bin-aarch64-apple-darwin/omniatc-client)
  - [Linux (x86_64)](https://sof3.github.io/omniatc/bin-x86_64-unknown-linux-gnu/omniatc-client)
  - [Linux (aarch64)](https://sof3.github.io/omniatc/bin-aarch64-unknown-linux-gnu/omniatc-client)
- Compile from source
  1. Install [rust toolchain] (https://rustup.rs)
  2. Clone this repository
  3. `cargo run -p omniatc-maps` to generate builtin maps
  4. `cargo run --release -p omniatc-client` to run the game (remove `--release` to disable optimizations for faster compile time)

## ~~TODOs~~ ~~Planned features~~ ~~Wishlist~~ Vaporwares

... surely there are many features I want to implement, but let's not make it a vaporware for now.
Nevertheless, for those who want to contribute, I want to leave room for the following potential features,
which may significantly affect design decisions:

- Emergencies/anomalies:
  - No GNSS: Aircraft must be within the range of at least two DMEs or one VOR+DME to navigate
    in the event of GNSS failure or interference
  - No RNAV: RNAV-incapable aircraft can only navigate directly towards navaid radials
  - Pilot incapacitation: Aircraft may be uncontrollable and forcefully enter auto-land mode
  - Engine failures: Aircraft with malfunctioning engines have reduced thrust and may be unable to climb/maintain altitude
- Operations:
  - Visibility: Landing in low visibility may result in go-arounds if ILS minima exceeds visibility
  - ILS interference: Lining up planes in ILS critical area may increase visibility requirement
  - Ground: Give taxi instructions to planes on ground
  - Takeoff: Planes may take off at the middle of the runway if conditions permit
- Variations:
  - Aircraft types: There may be non-plane aircraft such as helicopters
  - Ground vehicles: There may be ground vehicles requesting clearance for runway crossing
  - Moving waypoints: Waypoint positions including runways may move, e.g. aircraft carrier landing
- Gameplay
  - World generation: Terrain may be randomly generated to produce natural altitude minima (implemented as a heightmap)
  - Noise abatement: Impact on game score when planes operate at low altitude near populated areas (implemented as a heatmap)
  - Fuel efficiency: Impact on game score when issuing:
    - altitude/speed increase instructions to arrivals
    - altitude/speed decrease instructions to departures
  - Expedite altitude change: Impact on game score
- Other features:
  - Mobile and web support
  - Spectation/Multiplayer: The UI may render objects simulated from a remote host not managed under `crate::level`
  - 3D camera: unlikely to implement, but code under `crate::level` should convey all necessary 3D information
