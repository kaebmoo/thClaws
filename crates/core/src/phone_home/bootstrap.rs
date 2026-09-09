//! Boot a phone-home tunnel from a stored [`PhoneHomeConfig`] and own its
//! lifetime for the GUI worker (dev-plan/44 Tier 1). gui-gated for the same
//! reason as `line::bootstrap` — it forwards into the `shared_session`
//! worker, which the CLI binary doesn't have.

use std::sync::mpsc;
use std::sync::Arc;

use super::session::PhoneHomeSink;
use super::PhoneHomeConfig;
use crate::bridge::{BridgeClient, BridgeConfig};
use crate::cancel::CancelToken;
use crate::shared_session::ShellInput;

/// Live phone-home handle stored on the worker. Dropping it alone won't
/// stop the tunnel — fire `cancel.cancel()` first.
pub struct PhoneHomeHandle {
    pub cancel: CancelToken,
    pub join: tokio::task::JoinHandle<()>,
    /// Relay URL the tunnel connects to (no token — safe for the UI).
    pub server_url: String,
    /// Shared client so the worker can fan live `ViewEvent`s up the tunnel
    /// to a connected dashboard surface (dev-plan/44 streaming).
    pub client: Arc<BridgeClient<PhoneHomeConfig>>,
}

/// Spawn the phone-home tunnel on the tokio runtime. Every inbound
/// `UserMessage` arrives as `ShellInput::LineMessage`, runs through the
/// agent loop, and the captured assistant text is shipped back up the
/// tunnel.
pub fn spawn(config: PhoneHomeConfig, input_tx: mpsc::Sender<ShellInput>) -> PhoneHomeHandle {
    let cancel = CancelToken::new();
    let server_url = config.resolved_server_url();

    let client = Arc::new(super::build_client(config, cancel.clone()));

    // Phase 8: the tunnel makes this machine's agent reachable from the
    // cloud, which is the one thing a locked-down deployment cannot let
    // a user switch on. Refuse here — every path that starts a session
    // (boot autoconnect, `/remote` pairing, reconnect) lands on this
    // function. The handle is still returned, already cancelled, so
    // callers that hold one keep working without a special case.
    if !crate::policy::remote_allowed() {
        eprintln!(
            "\x1b[33m[phone-home] refused: thClaws Remote is disabled by org policy \
             (policies.runtime.allow_remote = false)\x1b[0m"
        );
        cancel.cancel();
        return PhoneHomeHandle {
            cancel,
            join: tokio::spawn(async {}),
            server_url,
            client,
        };
    }
    let sink = PhoneHomeSink::new(input_tx);

    let cancel_for_task = cancel.clone();
    let run_client = client.clone();
    let join = tokio::spawn(async move {
        if let Err(e) = run_client.run(sink).await {
            eprintln!("[phone-home] session ended: {e}");
        }
        cancel_for_task.cancel();
    });

    PhoneHomeHandle {
        cancel,
        join,
        server_url,
        client,
    }
}
