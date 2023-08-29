use fleek_crypto::ClientPublicKey;

use crate::internal::{OnEventResponseArgs, OnStartArgs};

/// This method should always be called during the initialization of the SDK context.
pub fn setup(_args: OnStartArgs) {}

/// Handle a response from core.
pub fn on_event_response(_args: OnEventResponseArgs) {}

pub async fn query_client_balance(_pk: ClientPublicKey) {
    todo!()
}
