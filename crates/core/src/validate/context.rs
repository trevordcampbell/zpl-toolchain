use crate::grammar::diag::Span;
use crate::state::DeviceState;
use std::collections::HashSet;
use zpl_toolchain_profile::Profile;

/// Shared immutable context threaded through validation passes.
#[derive(Clone, Copy)]
pub(super) struct ValidationContext<'a> {
    pub(super) profile: Option<&'a Profile>,
    pub(super) label_nodes: &'a [crate::grammar::ast::Node],
    pub(super) label_codes: &'a HashSet<&'a str>,
    pub(super) device_state: &'a DeviceState,
}

/// Per-command view used by validation helpers.
#[derive(Clone, Copy)]
pub(super) struct CommandCtx<'a> {
    pub(super) code: &'a str,
    pub(super) args: &'a [crate::grammar::ast::ArgSlot],
    pub(super) cmd: &'a zpl_toolchain_spec_tables::CommandEntry,
    pub(super) span: Option<Span>,
    pub(super) node_idx: usize,
}
