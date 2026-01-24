use std::time::Duration;

use math::{Heading, Position, Speed};
use serde::{Deserialize, Serialize};

use crate::{QuestRef, Range, Score, SegmentRef};

/// All quests.
#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct QuestTree {
    /// All completed and incomplete quests.
    ///
    /// This list is order-sensitive.
    /// Client display would rank quests earlier in this list higher.
    pub quests: Vec<Quest>,
}

/// A pending quest.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Quest {
    /// Unique identifier for the quest.
    pub id:           QuestRef,
    /// Human-readable title of the quest.
    pub title:        String,
    /// Description of the quest.
    pub description:  String,
    /// Type of the quest.
    pub class:        QuestClass,
    /// List of quests that must be completed before this quest is displayed.
    pub dependencies: Vec<QuestRef>,
    /// Conditions for completing the quest.
    ///
    /// If a condition has been completed, it is removed from the quest.
    /// If `conditions` is empty, the quest is considered completed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions:   Vec<QuestCompletionCondition>,
    /// UI elements to highlight when the quest is focused.
    pub ui_highlight: Vec<HighlightableUiElement>,
}

/// Classifies the quest type.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum QuestClass {
    /// A tutorial quest is for onboarding new players,
    /// so the interface is as accessible as possible
    /// but still skippable for experienced players.
    Tutorial,
    /// An achievement quest indicates a challenging task,
    /// so it should not block the regular gameplay flow.
    Achievement,
}

impl QuestClass {
    /// Whether the quest can be skipped by the player.
    #[must_use]
    pub fn is_skippable(self) -> bool {
        match self {
            QuestClass::Tutorial => true,
            QuestClass::Achievement => false,
        }
    }

    /// Whether the quest should be displayed in the active quest list.
    #[must_use]
    pub fn display_in_list(self) -> bool {
        match self {
            QuestClass::Tutorial => false,
            QuestClass::Achievement => true,
        }
    }

    /// Whether the quest should be displayed as an overlay popup
    /// when it is the first active quest.
    #[must_use]
    pub fn display_in_popup(self) -> bool {
        match self {
            QuestClass::Tutorial => true,
            QuestClass::Achievement => false,
        }
    }
}

/// A condition that triggers quest completion.
#[derive(Clone, Serialize, Deserialize, derive_more::From)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum QuestCompletionCondition {
    /// Simple tutorial actions for UI camera interaction.
    Camera(CameraQuestCompletionCondition),
    /// Object control actions.
    ObjectControl(ObjectControlQuestCompletionCondition),
    /// Conditions based on statistics.
    Statistic(StatisticQuestCompletionCondition),
}

/// Simple tutorial actions for UI camera interaction.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum CameraQuestCompletionCondition {
    /// Dragging camera.
    Drag,
    /// Zooming camera.
    Zoom,
    /// Rotating camera.
    Rotate,
}

/// Object control actions for tutorial quests.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ObjectControlQuestCompletionCondition {
    // Airborne
    /// Instructing an object to climb or descend.
    ///
    /// Only one altitude condition can be used in the same quest.
    /// Behavior is unspecified if multiple altitude conditions are used.
    ReachAltitude(Range<Position<f32>>),
    /// Instructing an object to change speed.
    ///
    /// Only one speed condition can be used in the same quest.
    /// Behavior is unspecified if multiple speed conditions are used.
    ReachSpeed(Range<Speed<f32>>),
    /// Instructing an object to turn to a specific heading.
    ///
    /// Only one heading condition can be used in the same quest.
    /// Behavior is unspecified if multiple heading conditions are used.
    ReachHeading(Range<Heading>),
    /// Instructing an object to navigate directly to an arbitrary waypoint.
    DirectToWaypoint,
    /// Instructing an object to align with a localizer.
    ClearIls,

    // Ground
    /// Instructing a ground object to taxi to a specific taxiway.
    ///
    /// Only one taxi condition can be used in the same quest.
    /// Behavior is unspecified if multiple taxiway conditions are used.
    TaxiSegment(SegmentRef),
    /// Instructing a ground object to line up with a runway.
    ClearLineUp,
    /// Instructing a ground object to take off from a runway.
    ClearTakeoff,

    // Generic
    /// Instructing an object to follow a route.
    FollowRoute,
}

/// Statistical achievement conditions.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum StatisticQuestCompletionCondition {
    /// Complete a number of landings.
    MinLanding(u32),
    /// Complete a number of apron arrivals.
    MinParking(u32),
    /// Complete a number of departures.
    MinDeparture(u32),
    /// Achieve at least the given score.
    MinScore(Score),
    /// Completes immediately if the number of conflicts is below or equal to the given number.
    /// Never completes if the number of conflicts exceeds this value.
    ///
    /// Typically used as a dependent quest after another statistic quest.
    MaxConflicts(u32),
    /// Minimum play time.
    TimeElapsed(Duration),
}

/// A UI element that can be highlighted for tutorial purposes.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum HighlightableUiElement {
    /// Outline of the main radar view.
    RadarView,
    /// Camera rotation controls in level info.
    SetCameraRotation,
    /// Camera zoom controls in level info.
    SetCameraZoom,
    /// UI for setting altitude.
    SetAltitude,
    /// UI for setting speed.
    SetSpeed,
    /// UI for setting heading.
    SetHeading,
}
