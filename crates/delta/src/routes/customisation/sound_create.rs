use revolt_config::config;
use revolt_database::{util::permissions::DatabasePermissionQuery, Database, File, Sound, User};
use revolt_models::v0;
use revolt_permissions::{calculate_server_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use validator::Validate;

use rocket::{serde::json::Json, State};

/// # Create New Sound
///
/// Create a soundboard sound by its Autumn upload id.
#[openapi(tag = "Soundboard")]
#[put("/sound/<sound_id>", data = "<data>")]
pub async fn create_sound(
    db: &State<Database>,
    user: User,
    sound_id: String,
    data: Json<v0::DataCreateSound>,
) -> Result<Json<v0::Sound>> {
    let config = config().await;

    let data = data.into_inner();
    data.validate().map_err(|error| {
        create_error!(FailedValidation {
            error: error.to_string()
        })
    })?;

    // Validate we have permission to write into parent
    match &data.parent {
        v0::SoundParent::Server { id } => {
            let server = db.fetch_server(id).await?;

            // Reuses ManageCustomisation rather than inventing a bit: it
            // already governs emoji, which is the same kind of asset.
            let mut query = DatabasePermissionQuery::new(db, &user).server(&server);
            calculate_server_permissions(&mut query)
                .await
                .throw_if_lacking_channel_permission(ChannelPermission::ManageCustomisation)?;

            let sounds = db.fetch_sounds_by_parent_id(&server.id).await?;
            if sounds.len() >= config.features.limits.global.server_sounds {
                return Err(create_error!(TooManySounds {
                    max: config.features.limits.global.server_sounds,
                }));
            }
        }
        v0::SoundParent::Detached => return Err(create_error!(InvalidOperation)),
    };

    // Claim the upload. Without this the file has no `used_for` and autumn
    // answers 404 when anyone tries to fetch it.
    File::use_sound(db, &sound_id, &sound_id, &user.id).await?;

    let sound = Sound {
        id: sound_id,
        parent: data.parent.into(),
        creator_id: user.id.clone(),
        name: data.name,
        category: data.category,
        duration: data.duration,
    };

    sound.create(db).await?;

    Ok(Json(sound.into()))
}
