//! A refused value: why, and what to tell people, failing at a position

use super::Failure;
use crate::check::DataSizeError;
use crate::error::ParseErrorReason;
use parsicomb::combinator::ReasonError;
use parsicomb::{CodeLoc, ParsicombError};

/// A refused value: its reason, and the message for people, taken together
/// so the two can't disagree
pub struct Rejection {
    reason: ParseErrorReason,
    message: String,
}

impl Rejection {
    /// A reason with a more specific message than its own description
    pub fn new(reason: ParseErrorReason, message: String) -> Self {
        Rejection { reason, message }
    }

    /// Fail at `loc`, where the refused value starts
    pub fn at(self, loc: CodeLoc<'_>) -> Failure<'_> {
        let message = self.message.into();
        let located = ParsicombError::Invalid { message, loc };
        Failure::from(ReasonError::new(self.reason, located))
    }
}

impl From<ParseErrorReason> for Rejection {
    fn from(reason: ParseErrorReason) -> Self {
        Rejection::new(reason, reason.to_string())
    }
}

impl From<DataSizeError<'_>> for Rejection {
    fn from(error: DataSizeError<'_>) -> Self {
        Rejection::new(error.reason(), error.to_string())
    }
}
