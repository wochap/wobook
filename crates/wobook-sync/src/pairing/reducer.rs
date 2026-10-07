//! Pure pairing state machine for both roles (D6). The manager does all IO.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Offerer,
    Joiner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadProof,
    Expired,
    Rejected,
    RateLimited,
    Protocol,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadProof => "bad_proof",
            Self::Expired => "expired",
            Self::Rejected => "rejected",
            Self::RateLimited => "rate_limited",
            Self::Protocol => "protocol",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum State {
    /// Joiner before `pair.begin`, offerer before receiving it.
    Start,
    AwaitNonce,
    AwaitProof,
    Confirming {
        local: Option<bool>,
        remote: Option<bool>,
    },
    AwaitProvision,
    AwaitDone,
    Done,
    Failed {
        code: ErrorCode,
    },
}

impl State {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::AwaitNonce => "await_nonce",
            Self::AwaitProof => "await_proof",
            Self::Confirming { .. } => "confirming",
            Self::AwaitProvision => "await_provision",
            Self::AwaitDone => "await_done",
            Self::Done => "done",
            Self::Failed { .. } => "failed",
        }
    }
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Joiner: connection open. Offerer: `pair.begin` received.
    Begin,
    /// Joiner: `pair.nonce` received.
    Nonce,
    /// Offerer: `pair.complete` received; `valid` is the MAC check result.
    Complete {
        valid: bool,
    },
    LocalDecision(bool),
    RemoteDecision(bool),
    /// Joiner: `pair.provision` received.
    Provision,
    /// Offerer: `pair.done` received.
    Done,
    Timeout,
    /// `pair.error` received or the stream failed.
    Error(ErrorCode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    SendBegin,
    SendNonce,
    SendProof,
    SendDecision(bool),
    SendProvision,
    SendDone,
    SendError(ErrorCode),
    ExposePending,
    /// Joiner: adopt the provisioned root (export, quarantine, join, import).
    Join,
    PersistTrust,
    Fail(ErrorCode),
}

fn fail(code: ErrorCode, send: bool) -> (State, Vec<Action>) {
    let mut actions = Vec::new();
    if send {
        actions.push(Action::SendError(code));
    }
    actions.push(Action::Fail(code));
    (State::Failed { code }, actions)
}

fn confirming(role: Role, local: Option<bool>, remote: Option<bool>) -> (State, Vec<Action>) {
    match (local, remote) {
        (Some(true), Some(true)) => match role {
            Role::Offerer => (State::AwaitDone, vec![Action::SendProvision]),
            Role::Joiner => (State::AwaitProvision, vec![]),
        },
        _ => (State::Confirming { local, remote }, vec![]),
    }
}

#[must_use]
pub fn reduce(role: Role, state: &State, event: Event) -> (State, Vec<Action>) {
    use Event as E;
    use State as S;
    if state.is_terminal() {
        return (state.clone(), vec![]);
    }
    match (role, state, event) {
        (_, _, E::Timeout) => fail(ErrorCode::Expired, true),
        (_, _, E::Error(code)) => fail(code, false),
        (Role::Joiner, S::Start, E::Begin) => (S::AwaitNonce, vec![Action::SendBegin]),
        (Role::Offerer, S::Start, E::Begin) => (S::AwaitProof, vec![Action::SendNonce]),
        (Role::Joiner, S::AwaitNonce, E::Nonce) => (
            S::Confirming {
                local: None,
                remote: None,
            },
            vec![Action::SendProof, Action::ExposePending],
        ),
        (Role::Offerer, S::AwaitProof, E::Complete { valid: true }) => (
            S::Confirming {
                local: None,
                remote: None,
            },
            vec![Action::ExposePending],
        ),
        (Role::Offerer, S::AwaitProof, E::Complete { valid: false }) => {
            fail(ErrorCode::BadProof, true)
        }
        (
            _,
            S::Confirming {
                local: None,
                remote,
            },
            E::LocalDecision(decision),
        ) => {
            if !decision {
                return fail(ErrorCode::Rejected, true);
            }
            let (next, mut actions) = confirming(role, Some(true), *remote);
            actions.insert(0, Action::SendDecision(true));
            (next, actions)
        }
        (
            _,
            S::Confirming {
                local,
                remote: None,
            },
            E::RemoteDecision(decision),
        ) => {
            if !decision {
                return fail(ErrorCode::Rejected, false);
            }
            confirming(role, *local, Some(true))
        }
        (Role::Joiner, S::AwaitProvision, E::Provision) => (
            S::Done,
            vec![Action::Join, Action::PersistTrust, Action::SendDone],
        ),
        (Role::Offerer, S::AwaitDone, E::Done) => (S::Done, vec![Action::PersistTrust]),
        _ => fail(ErrorCode::Protocol, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(role: Role, events: Vec<Event>) -> (State, Vec<Action>) {
        let mut state = State::Start;
        let mut all = Vec::new();
        for e in events {
            let (next, actions) = reduce(role, &state, e);
            state = next;
            all.extend(actions);
        }
        (state, all)
    }

    #[test]
    fn offerer_both_confirm() {
        let (s, a) = run(
            Role::Offerer,
            vec![
                Event::Begin,
                Event::Complete { valid: true },
                Event::RemoteDecision(true),
                Event::LocalDecision(true),
                Event::Done,
            ],
        );
        assert_eq!(s, State::Done);
        assert_eq!(
            a,
            vec![
                Action::SendNonce,
                Action::ExposePending,
                Action::SendDecision(true),
                Action::SendProvision,
                Action::PersistTrust
            ]
        );
    }

    #[test]
    fn joiner_both_confirm() {
        let (s, a) = run(
            Role::Joiner,
            vec![
                Event::Begin,
                Event::Nonce,
                Event::LocalDecision(true),
                Event::RemoteDecision(true),
                Event::Provision,
            ],
        );
        assert_eq!(s, State::Done);
        assert_eq!(
            a,
            vec![
                Action::SendBegin,
                Action::SendProof,
                Action::ExposePending,
                Action::SendDecision(true),
                Action::Join,
                Action::PersistTrust,
                Action::SendDone
            ]
        );
    }

    #[test]
    fn local_reject() {
        let (s, a) = run(
            Role::Joiner,
            vec![Event::Begin, Event::Nonce, Event::LocalDecision(false)],
        );
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::Rejected
            }
        );
        assert!(a.contains(&Action::SendError(ErrorCode::Rejected)));
        assert!(!a.contains(&Action::PersistTrust));
    }

    #[test]
    fn remote_reject() {
        let (s, a) = run(
            Role::Offerer,
            vec![
                Event::Begin,
                Event::Complete { valid: true },
                Event::LocalDecision(true),
                Event::RemoteDecision(false),
            ],
        );
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::Rejected
            }
        );
        assert!(!a.contains(&Action::PersistTrust));
        assert!(!a.contains(&Action::SendProvision));
    }

    #[test]
    fn bad_proof() {
        let (s, a) = run(
            Role::Offerer,
            vec![Event::Begin, Event::Complete { valid: false }],
        );
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::BadProof
            }
        );
        assert_eq!(a.last(), Some(&Action::Fail(ErrorCode::BadProof)));
    }

    #[test]
    fn expired_window_error_and_timeout() {
        let (s, _) = run(
            Role::Joiner,
            vec![Event::Begin, Event::Error(ErrorCode::Expired)],
        );
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::Expired
            }
        );
        let (s, a) = run(Role::Offerer, vec![Event::Begin, Event::Timeout]);
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::Expired
            }
        );
        assert!(a.contains(&Action::SendError(ErrorCode::Expired)));
    }

    #[test]
    fn out_of_order_is_protocol_and_terminal_is_sticky() {
        let (s, _) = run(Role::Offerer, vec![Event::Provision]);
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::Protocol
            }
        );
        let (s2, a) = reduce(Role::Offerer, &s, Event::Begin);
        assert_eq!(s2, s);
        assert!(a.is_empty());
        let (s, _) = run(
            Role::Joiner,
            vec![
                Event::Begin,
                Event::Nonce,
                Event::LocalDecision(true),
                Event::LocalDecision(true),
            ],
        );
        assert_eq!(
            s,
            State::Failed {
                code: ErrorCode::Protocol
            }
        );
    }
}
