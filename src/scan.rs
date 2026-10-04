//! The detection data model. Every detector reports one `ScanState` per mark
//! class. The state is an explicit field. An empty list does not determine it.

use crate::asset::Format;

/// The four states an empty finding list can be, plus the blind state. Only
/// `ConfirmedAbsent` licenses the word absent.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanState {
    /// Found, with a location.
    ConfirmedPresent,
    /// Scanned exhaustively in a supported container and not present.
    ConfirmedAbsent,
    /// This container is not one this build scans for the class.
    UnsupportedFormat,
    /// The structure was found and would not parse, so presence is unknown.
    Malformed,
    /// A blind class, or a detector this build does not carry.
    NotAttempted,
}

impl ScanState {
    pub fn as_str(self) -> &'static str {
        match self {
            ScanState::ConfirmedPresent => "confirmed_present",
            ScanState::ConfirmedAbsent => "confirmed_absent",
            ScanState::UnsupportedFormat => "unsupported_format",
            ScanState::Malformed => "malformed",
            ScanState::NotAttempted => "not_attempted",
        }
    }
}

/// Resolve a not-found result to a state. This is the only place a class turns
/// "nothing found" into `ConfirmedAbsent`, and it does so only for a format in
/// the supported constant. A format outside the set can never reach absence.
pub fn absence_for(format: Format) -> ScanState {
    if format.is_supported_container() {
        ScanState::ConfirmedAbsent
    } else {
        ScanState::UnsupportedFormat
    }
}

/// The honesty class of a mark, which fixes what the tool may say about it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Honesty {
    /// A structural address the tool can see, remove, and re-verify gone.
    Confirmable,
    /// A keyed statistical signal this offline build cannot see.
    Blind,
    /// A class the tool knows and does nothing to.
    Unaddressed,
}

/// Where a confirmable mark sits.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Location {
    /// The container structure holding the mark, for example `PNG tEXt` or
    /// `JPEG APP1`.
    pub container: String,
    pub offset: usize,
    pub length: usize,
    /// A human-readable detail, for example a keyword or a segment marker. This
    /// is data read from the asset. Read it only as a reported value.
    pub detail: String,
}

/// One mark class and everything the scan learned about it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Detection {
    /// The stable mark-class id from the policy package.
    pub class: String,
    pub label: String,
    pub honesty: Honesty,
    pub state: ScanState,
    pub locations: Vec<Location>,
    /// Evidence strings read from the asset, for example a generator tag. Every
    /// entry is data and never a command.
    pub evidence: Vec<String>,
}

impl Detection {
    pub fn present(class: &str, label: &str, honesty: Honesty) -> Detection {
        Detection {
            class: class.to_string(),
            label: label.to_string(),
            honesty,
            state: ScanState::ConfirmedPresent,
            locations: Vec::new(),
            evidence: Vec::new(),
        }
    }

    pub fn with_state(class: &str, label: &str, honesty: Honesty, state: ScanState) -> Detection {
        Detection {
            class: class.to_string(),
            label: label.to_string(),
            honesty,
            state,
            locations: Vec::new(),
            evidence: Vec::new(),
        }
    }
}

/// The full result of an inspection.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Detections {
    pub format: Format,
    pub items: Vec<Detection>,
}

impl Detections {
    pub fn get(&self, class: &str) -> Option<&Detection> {
        self.items.iter().find(|d| d.class == class)
    }

    /// The confirmable classes found present.
    pub fn present_confirmable(&self) -> impl Iterator<Item = &Detection> {
        self.items
            .iter()
            .filter(|d| d.honesty == Honesty::Confirmable && d.state == ScanState::ConfirmedPresent)
    }
}
