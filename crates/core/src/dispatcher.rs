// SPDX-License-Identifier: GPL-3.0-or-later
//! The single entry point for every action, regardless of where it came from.

use crate::engine::{Change, Engine};
use crate::permissions;
use crate::protocol::{Action, ErrorCode, Role};

/// Where an action came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Origin {
    pub role: Role,
    /// The action originates from the host machine (operator UI, output window, keyboard).
    pub local: bool,
}

impl Origin {
    pub const LOCAL_ADMIN: Origin = Origin {
        role: Role::Admin,
        local: true,
    };
}

/// Result of a successful dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum Dispatched {
    /// The engine applied the action.
    Applied(Change),
    /// The action is authorized but must be carried out by a host service
    /// (file system, devices). The caller performs it.
    Host(Box<Action>),
}

/// Checks permissions, then applies the action to the engine or hands it back for a host
/// service to execute.
pub fn dispatch(
    engine: &mut Engine,
    origin: Origin,
    action: Action,
    now_ms: i64,
) -> Result<Dispatched, ErrorCode> {
    permissions::check(origin.role, origin.local, &action, engine)?;
    if is_host_action(&action) {
        return Ok(Dispatched::Host(Box::new(action)));
    }
    engine.apply(&action, now_ms).map(Dispatched::Applied)
}

/// Actions executed by host services rather than the engine.
pub fn is_host_action(action: &Action) -> bool {
    use Action::*;
    matches!(
        action,
        AddFiles { .. }
            | SetLogoImage { .. }
            | SetBackgroundImage { .. }
            | SetOverlayImage { .. }
            | NewShow
            | OpenShow { .. }
            | SaveShow { .. }
            | ApprovePairing { .. }
            | DenyPairing { .. }
            | SetDeviceRole { .. }
            | RevokeDevice { .. }
            | DisconnectAll
            | SetAutoApprove { .. }
            | AcceptUpload { .. }
            | RejectUpload { .. }
            | SetAutoAcceptUploads { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tests::two_cue_engine;

    #[test]
    fn engine_actions_are_applied() {
        let mut e = two_cue_engine();
        let r = dispatch(&mut e, Origin::LOCAL_ADMIN, Action::Next, 0).unwrap();
        assert_eq!(r, Dispatched::Applied(Change::LIVE));
        assert!(e.program().is_some());
    }

    #[test]
    fn host_actions_are_returned() {
        let mut e = two_cue_engine();
        let r = dispatch(&mut e, Origin::LOCAL_ADMIN, Action::NewShow, 0).unwrap();
        assert_eq!(r, Dispatched::Host(Box::new(Action::NewShow)));
    }

    #[test]
    fn permission_failures_do_not_mutate() {
        let mut e = two_cue_engine();
        let origin = Origin {
            role: Role::StageViewer,
            local: false,
        };
        assert_eq!(
            dispatch(&mut e, origin, Action::Next, 0),
            Err(ErrorCode::Forbidden)
        );
        assert!(e.program().is_none());
    }

    #[test]
    fn host_action_list_matches_engine() {
        // Every host action must be rejected by the engine and vice versa.
        let samples = [
            Action::Next,
            Action::ToggleBlackout,
            Action::NewShow,
            Action::DisconnectAll,
            Action::SaveShow {
                path: None,
                embed_media: false,
            },
        ];
        for a in samples {
            let mut e = two_cue_engine();
            let engine_rejects = e.apply(&a, 0) == Err(ErrorCode::InvalidState);
            assert_eq!(engine_rejects, is_host_action(&a), "{a:?}");
        }
    }
}
