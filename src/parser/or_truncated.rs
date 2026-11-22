//! Ending a parser in the error every parser here returns, with failures
//! that have no reason of their own counted as truncation

use super::Failure;
use crate::error::Reason;
use parsicomb::combinator::{MapErrExt, ReasonExt};
use parsicomb::{ByteCursor, Parser, ParsicombError, Reasons};

/// Ends a parser in a [`Failure`], the error every parser here returns
pub(crate) trait OrTruncatedExt<'a>: Parser<Cursor = ByteCursor<'a>> + Sized
where
    Self::Error: Reasons<Reason> + Into<ParsicombError<'a>>,
{
    /// The parser, failing with a [`Failure`]: a failure that has a reason
    /// keeps it, and one without counts as [`Reason::Truncated`]
    ///
    /// The only failures without a reason are parsicomb's own end of input,
    /// so `Truncated` is right for those; every other failure here gets its
    /// reason where it happens. A `Failure` needs some default, so this is it.
    fn or_truncated(
        self,
    ) -> impl Parser<Cursor = ByteCursor<'a>, Output = Self::Output, Error = Failure<'a>> {
        self.reason(Reason::Truncated).map_err(Failure::from)
    }
}

impl<'a, P> OrTruncatedExt<'a> for P
where
    P: Parser<Cursor = ByteCursor<'a>>,
    P::Error: Reasons<Reason> + Into<ParsicombError<'a>>,
{
}
