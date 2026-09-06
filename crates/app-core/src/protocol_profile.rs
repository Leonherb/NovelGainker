//! Host-selected protocol limits. They do not change ordinary user-message limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToolProtocolProfile {
    #[default]
    Nr4,
    Nr5,
}

impl ToolProtocolProfile {
    pub const fn argument_bytes(self) -> usize {
        match self {
            Self::Nr4 => crate::tool_runtime::MAX_TOOL_ARGUMENTS_BYTES,
            Self::Nr5 => crate::chapter_contract::NR5_TOOL_ARGUMENT_BYTES,
        }
    }
    pub const fn result_bytes(self) -> usize {
        match self {
            Self::Nr4 => crate::tool_runtime::MAX_TOOL_RESULT_BYTES,
            Self::Nr5 => crate::chapter_contract::NR5_TOOL_RESULT_BYTES,
        }
    }
    pub const fn call_envelope_bytes(self) -> usize {
        match self {
            Self::Nr4 => crate::tool_runtime::MAX_TOOL_CONTINUATION_BYTES,
            Self::Nr5 => crate::chapter_contract::NR5_TOOL_ARGUMENT_BYTES,
        }
    }
    pub const fn native_continuation_bytes(self) -> usize {
        match self {
            Self::Nr4 => crate::tool_runtime::MAX_TOOL_CONTINUATION_BYTES,
            Self::Nr5 => crate::chapter_contract::NR5_CONTINUATION_BYTES,
        }
    }
    pub const fn stream_event_bytes(self) -> usize {
        match self {
            Self::Nr4 => crate::provider_core::MAX_EVENT_BYTES,
            Self::Nr5 => crate::chapter_contract::NR5_CONTINUATION_BYTES,
        }
    }
    pub fn for_definition(name: &str, version: u32) -> Self {
        if version == 2
            && matches!(
                name,
                "novelist.read_context" | "novelist.propose_change" | "novelist.publish"
            )
        {
            Self::Nr5
        } else {
            Self::Nr4
        }
    }
}
