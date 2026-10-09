// SPDX-License-Identifier: GPL-3.0-or-later
//! Which role may perform which action.

use crate::engine::Engine;
use crate::protocol::{Action, ErrorCode, Role};

/// Minimum role required for an action, ignoring context-dependent rules.
pub fn required_role(action: &Action) -> Role {
    use Action::*;
    match action {
        Go | Next | Prev => Role::Presenter,
        NextCue | PrevCue | GoTo { .. } => Role::Operator,
        SetBlackout { .. }
        | ToggleBlackout
        | SetFreeze { .. }
        | ToggleFreeze
        | SetLogo { .. }
        | ToggleLogo
        | Panic => Role::Operator,
        TimerStart | TimerPause | TimerReset => Role::Operator,
        MediaPlay | MediaPause | MediaSeek { .. } | MediaRestart => Role::Operator,
        MediaLoaded { .. } | MediaEnded { .. } => Role::Operator,
        CountdownSet { .. } | CountdownStart | CountdownPause | CountdownReset => Role::Operator,
        SetStageMessage { .. } | SetOverlayVisible { .. } | ToggleOverlay { .. } => Role::Operator,
        SetTestPattern { .. } => Role::Operator,
        DrawStroke { .. } | ClearDrawing => Role::Presenter,
        AcceptUpload { .. } | RejectUpload { .. } | SetAutoAcceptUploads { .. } => Role::Admin,
        PutOutput { .. }
        | RemoveOutput { .. }
        | SetCueTargets { .. }
        | AddWeb { .. }
        | SetWebOptions { .. }
        | AddCapture { .. }
        | SetCapture { .. } => Role::Admin,
        PutOverlay { .. }
        | RemoveOverlay { .. }
        | AddText { .. }
        | SetCueText { .. }
        | AddTimer { .. }
        | SetCueTimer { .. }
        | SetCueTheme { .. }
        | SetDefaultTheme { .. }
        | SetCueTransition { .. }
        | SetDefaultTransition { .. }
        | SetCueAutoAdvance { .. }
        | SetMediaOptions { .. }
        | SetLogoImage { .. }
        | SetBackgroundImage { .. }
        | SetOverlayImage { .. } => Role::Admin,
        RenameShow { .. }
        | RenameCue { .. }
        | SetCueNotes { .. }
        | SetCueColor { .. }
        | MoveCue { .. }
        | RemoveCue { .. }
        | AddBlank { .. }
        | AddFiles { .. }
        | NewShow
        | OpenShow { .. }
        | SaveShow { .. } => Role::Admin,
        ApprovePairing { .. }
        | DenyPairing { .. }
        | SetDeviceRole { .. }
        | RevokeDevice { .. }
        | DisconnectAll
        | SetAutoApprove { .. } => Role::Admin,
    }
}

/// Actions whose arguments refer to the host's file system and therefore may only come from
/// the host machine itself.
pub fn is_local_only(action: &Action) -> bool {
    matches!(
        action,
        Action::AddFiles { .. }
            | Action::OpenShow { .. }
            | Action::SaveShow { path: Some(_), .. }
            | Action::SetLogoImage { path: Some(_) }
            | Action::SetBackgroundImage { path: Some(_), .. }
            | Action::SetOverlayImage { path: Some(_), .. }
            // Playback reports come from the host's own output windows.
            | Action::MediaLoaded { .. }
            | Action::MediaEnded { .. }
    )
}

/// Full permission check, including context-dependent rules.
pub fn check(role: Role, local: bool, action: &Action, engine: &Engine) -> Result<(), ErrorCode> {
    if role < required_role(action) {
        return Err(ErrorCode::Forbidden);
    }
    if is_local_only(action) && !local {
        return Err(ErrorCode::LocalOnly);
    }
    // Presenters may only move within the cue that is currently live (for a web page with key
    // forwarding, next/prev stay inside the page).
    let navigates = matches!(action, Action::Next | Action::Go | Action::Prev);
    if role == Role::Presenter && navigates && !engine.forwards_keys() {
        let program_cue = engine.program().map(|p| p.cue_id.clone());
        let target = match action {
            Action::Next | Action::Go => engine.next_position(),
            Action::Prev => engine.prev_position(),
            _ => None,
        };
        match (program_cue, target) {
            (Some(cue), Some(target)) if target.cue_id == cue => {}
            // At the edge of the cue the action is a no-op rather than an error.
            (Some(_), None) => {}
            _ => return Err(ErrorCode::Forbidden),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tests::two_cue_engine;

    #[test]
    fn stage_viewer_cannot_do_anything() {
        let e = two_cue_engine();
        for a in [
            Action::Next,
            Action::ToggleBlackout,
            Action::TimerStart,
            Action::NewShow,
        ] {
            assert_eq!(
                check(Role::StageViewer, true, &a, &e),
                Err(ErrorCode::Forbidden)
            );
        }
    }

    #[test]
    fn operator_runs_the_show_but_cannot_edit() {
        let e = two_cue_engine();
        assert!(check(Role::Operator, false, &Action::ToggleBlackout, &e).is_ok());
        assert!(check(Role::Operator, false, &Action::NextCue, &e).is_ok());
        assert_eq!(
            check(
                Role::Operator,
                false,
                &Action::RemoveCue { cue_id: "x".into() },
                &e
            ),
            Err(ErrorCode::Forbidden)
        );
        assert_eq!(
            check(Role::Operator, false, &Action::DisconnectAll, &e),
            Err(ErrorCode::Forbidden)
        );
    }

    #[test]
    fn file_actions_are_local_only() {
        let e = two_cue_engine();
        let add = Action::AddFiles {
            paths: vec!["/etc/passwd".into()],
            at_index: None,
        };
        assert_eq!(
            check(Role::Admin, false, &add, &e),
            Err(ErrorCode::LocalOnly)
        );
        assert!(check(Role::Admin, true, &add, &e).is_ok());
        let save_here = Action::SaveShow {
            path: None,
            embed_media: true,
        };
        assert!(check(Role::Admin, false, &save_here, &e).is_ok());
    }

    #[test]
    fn presenter_stays_inside_current_cue() {
        let mut e = two_cue_engine();
        // Nothing live yet: presenter cannot start the show.
        assert_eq!(
            check(Role::Presenter, false, &Action::Next, &e),
            Err(ErrorCode::Forbidden)
        );

        let first = e.show().cues[0].id.clone();
        e.go_to(&first, 0, 0).unwrap();
        assert!(check(Role::Presenter, false, &Action::Next, &e).is_ok());

        // Last slide of the first cue (3 slides): next would leave the cue.
        e.go_to(&first, 2, 0).unwrap();
        assert_eq!(
            check(Role::Presenter, false, &Action::Next, &e),
            Err(ErrorCode::Forbidden)
        );
        assert!(check(Role::Presenter, false, &Action::Prev, &e).is_ok());

        // First slide: prev would leave the cue backwards.
        let second = e.show().cues[1].id.clone();
        e.go_to(&second, 0, 0).unwrap();
        assert_eq!(
            check(Role::Presenter, false, &Action::Prev, &e),
            Err(ErrorCode::Forbidden)
        );
        assert_eq!(
            check(Role::Presenter, false, &Action::ToggleBlackout, &e),
            Err(ErrorCode::Forbidden)
        );
    }

    #[test]
    fn presenter_at_show_end_is_noop_not_error() {
        let mut e = two_cue_engine();
        let second = e.show().cues[1].id.clone();
        e.go_to(&second, 1, 0).unwrap();
        assert!(check(Role::Presenter, false, &Action::Next, &e).is_ok());
    }
}
