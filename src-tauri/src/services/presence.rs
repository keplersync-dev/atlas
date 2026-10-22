//! What friends see this PC doing: "playing" plus the instance while a tracked game
//! runs (only "playing" if that instance hides its activity), otherwise "online".

use atlas_protocol::ws::PresenceUpdate;

use crate::db;
use crate::dto::PresenceState;
use crate::state::AppState;

/// Work out the presence from the running games and hand it to the socket.
pub async fn refresh(state: &AppState) {
    let instances = match db::instances::list(&state.db).await {
        Ok(instances) => instances,
        Err(e) => {
            tracing::warn!(error = %e, "couldn't read instances for presence");
            return;
        }
    };
    let playing = instances.iter().find(|i| state.is_running(&i.id));
    let update = match playing {
        None => PresenceUpdate::online(),
        Some(i) if i.hide_activity => PresenceUpdate {
            state: PresenceState::Playing,
            ..PresenceUpdate::online()
        },
        Some(i) => PresenceUpdate {
            state: PresenceState::Playing,
            instance_name: Some(i.name.clone()),
            minecraft_version: i.minecraft_version.clone(),
            shared_instance_id: i.shared_instance_id.clone(),
        },
    };
    state.cloud.set_presence(update);
}
