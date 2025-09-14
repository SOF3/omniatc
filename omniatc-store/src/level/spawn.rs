use std::time::Duration;

use math::{Heading, Position, Speed};
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};

use crate::{
    AerodromeRef, Destination, NamedWaypointRef, ObjectTypeRef, RoutePresetRef, RunwayRef, Score,
    WeightedList,
};

/// A setup for spawning objects.
#[derive(Clone, Serialize, Deserialize)]
pub struct SpawnSet {
    /// Reference to the route preset that spawned objects will follow.
    pub route:    WeightedList<SpawnRoute>,
    /// Rules for generating names for spawned objects.
    pub gen_name: WeightedList<NameGenerator>,
    /// The types of objects that may be spawned in this set.
    pub types:    WeightedList<ObjectTypeRef>,
    /// Position at which objects in this set will be spawned.
    pub position: WeightedList<SpawnPosition>,
}

/// The route and destination for a spawned object.
#[derive(Clone, Serialize, Deserialize)]
pub struct SpawnRoute {
    /// Reference to the route preset that spawned objects will follow.
    pub preset:      RoutePresetRef,
    /// Completion condition if the object takes this route.
    pub destination: Destination,
    /// Completion score if the object takes this route.
    pub score:       Score,
}

/// Rules for generating names for spawned objects.
#[derive(Clone, Serialize, Deserialize)]
pub enum NameGenerator {
    /// A regular airline-style name,
    /// with a prefix, a numeric part, and an optional trailing letter.
    Airline {
        /// ICAO code of the airline.
        prefix:          String,
        /// Number of digits in the numeric part of the name.
        digits:          u16,
        /// An optional letter to append to the end of the name.
        ///
        /// If multiple letters are specified, one will be chosen randomly with uniform probability.
        /// If an empty string is specified, a random alphabet will be chosen.
        /// If set to `None`, no letter will be appended.
        trailing_letter: Option<String>,
    },
    /// A custom name generated from a sequence of elements.
    Elements {
        /// Elements that produce the name.
        elements: Vec<NameGeneratorElement>,
    },
}

/// An element of [`NameGenerator::Elements`].
#[derive(Clone, Serialize, Deserialize)]
pub enum NameGeneratorElement {
    /// A fixed string of characters.
    Fixed(String),
    /// A random letter from A-Z.
    RandomAlphabet,
    /// A random digit from 0-9.
    RandomDigit,
    /// A random alphanumeric character from A-Z and 0-9.
    RandomAlphanumeric,
}

impl NameGenerator {
    /// Generates a name according to the rules of this generator.
    pub fn generate(&self, rng: &mut impl rand::Rng) -> String {
        match *self {
            NameGenerator::Airline { ref prefix, digits, ref trailing_letter } => {
                let mut output = String::with_capacity(
                    prefix.len() + usize::from(digits) + usize::from(trailing_letter.is_some()),
                );
                output.push_str(prefix);
                for _ in 0..digits {
                    output.push(rng.random_range('0'..='9'));
                }
                if let Some(chars) = trailing_letter {
                    output.push(
                        chars.chars().choose(rng).unwrap_or_else(|| rng.random_range('A'..='Z')),
                    );
                }
                output
            }
            NameGenerator::Elements { ref elements } => {
                let mut output = String::new();
                for element in elements {
                    match element {
                        NameGeneratorElement::Fixed(str) => output.push_str(str),
                        NameGeneratorElement::RandomAlphabet => {
                            output.push(rng.random_range('A'..='Z'));
                        }
                        NameGeneratorElement::RandomDigit => {
                            output.push(rng.random_range('0'..='9'));
                        }
                        NameGeneratorElement::RandomAlphanumeric => {
                            let rand = rng.random_range(0..36_u8);
                            if rand < 10 {
                                output.push(char::from_digit(rand.into(), 10).expect("rand < 10"));
                            } else {
                                output.push((b'A' + (rand - 10)) as char);
                            }
                        }
                    }
                }
                output
            }
        }
    }
}

/// Position at which objects in a spawn set will be spawned.
#[derive(Clone, Serialize, Deserialize)]
pub enum SpawnPosition {
    /// Objects created by this set will be spawned in a random occupied apron in the aerodrome.
    Aprons {
        /// Aerodrome on which objects will be spawned.
        aerodrome: AerodromeRef,
        /// If specified, only these aprons will be used.
        aprons:    Option<Vec<String>>,
    },
    /// Objects created by this set will be spawned on a taxiway next to a runway.
    Runway {
        /// The runway that the object is expected to use.
        runway:   RunwayRef,
        /// Taxiways next to the runway that may be used for spawning.
        ///
        /// If the taxiway is not next to the runway,
        /// the object would be spawned at and facing the endpoint closest to the runway start.
        taxiways: Vec<String>,
    },
    /// Objects created by this set will be spawned in the air near a waypoint.
    Airborne {
        /// Waypoint near which objects will be spawned.
        waypoint: NamedWaypointRef,
        /// Initial altitude of spawned objects.
        altitude: Position<f32>,
        /// Initial speed of spawned objects.
        speed:    Speed<f32>,
        /// Initial heading of spawned objects.
        heading:  Heading,
    },
}

/// Determines when new objects may spawn.
#[derive(Clone, Serialize, Deserialize)]
pub enum SpawnTrigger {
    /// Do not spawn any new objects.
    Disabled,
    /// New objects may spawn at fixed time intervals.
    Periodic {
        /// Time interval between spawns.
        duration: Duration,
    },
    /// New objects may spawn when the number of active objects is below a threshold.
    ObjectCount {
        /// Number of active objects to maintain.
        count: u32,
    },
}
