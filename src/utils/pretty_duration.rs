use pretty_duration::pretty_duration as _pretty_duration;
use std::time;

/// A simple wrapper function over [`pretty_duration::pretty_duration`] for default values.
pub fn pretty_duration(duration: &time::Duration) -> String {
    return _pretty_duration(duration, None);
}
