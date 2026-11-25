//! Group target precision, and the environment variable that sets it.

/// The precision group targets are held at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GroupPrecision {
    /// Eight bits per channel where a group's composite is plain source-over, and half floats
    /// elsewhere.
    #[default]
    Compact,
    /// Half floats for every group.
    High,
}

impl GroupPrecision {
    /// The environment variable that sets the precision.
    pub(crate) const VARIABLE: &str = "ICEBLINK_GROUP_PRECISION";

    /// Returns the precision the environment variable sets, read once per process.
    ///
    /// An unknown value is logged and sets nothing.
    pub(crate) fn from_environment() -> Option<Self> {
        static NAMED: std::sync::OnceLock<Option<GroupPrecision>> = std::sync::OnceLock::new();
        *NAMED.get_or_init(|| {
            let value = std::env::var(Self::VARIABLE).ok()?;
            let named = Self::parse(&value);
            if named.is_none() {
                tracing::warn!(
                    variable = Self::VARIABLE,
                    value = %value,
                    "unknown group precision, expected `high` or `compact`"
                );
            }
            named
        })
    }

    /// Parses `high` or `compact`, ignoring case and surrounding whitespace.
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("high") {
            Some(Self::High)
        } else if value.eq_ignore_ascii_case("compact") {
            Some(Self::Compact)
        } else {
            None
        }
    }
}
