//! Filename parsing via the `hunch` crate.
//!
//! Wraps hunch's `HunchResult` into mediarr's own `MediaInfo` type,
//! adding anime detection, multi-episode support, and ambiguity flagging.

use hunch::{hunch as hunch_parse, hunch_with_context as hunch_ctx, Property};

use crate::error::{MediError, Result};
use crate::types::{MediaInfo, MediaType, ParseConfidence};

// Verify imports compile -- this stub will be expanded in Task 2.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunch_api_smoke_test() {
        let result = hunch_parse("test.mkv");
        // Just verify it doesn't panic and returns a HunchResult
        let _ = result.title();
        let _ = result.confidence();
    }
}
