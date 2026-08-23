use revolt_database::{util::permissions::DatabasePermissionQuery, Database, SoundParent, User};
use revolt_models::v0;
use revolt_permissions::{calculate_server_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};
use rocket_empty::EmptyResponse;
use validator::Validate;

/// Fetch a sound and check the caller may change it.
///
/// Whoever uploaded a sound can always manage it; anyone else needs
/// ManageCustomisation on the server. Same rule as emoji.
async fn resolve_and_authorise(
    db: &Database,
    user: &User,
    sound_id: &str,
) -> Result<revolt_database::Sound> {
    let sound = db.fetch_sound(sound_id).await?;

    if sound.creator_id != user.id {
        match &sound.parent {
            SoundParent::Server { id } => {
                let server = db.fetch_server(id.as_str()).await?;
                let mut query = DatabasePermissionQuery::new(db, user).server(&server);
                calculate_server_permissions(&mut query)
                    .await
                    .throw_if_lacking_channel_permission(ChannelPermission::ManageCustomisation)?;
            }
            SoundParent::Detached => return Err(create_error!(NotFound)),
        }
    }

    Ok(sound)
}

/// # Fetch Sound
///
/// Fetch a sound by its id.
#[openapi(tag = "Soundboard")]
#[get("/sound/<sound_id>")]
pub async fn fetch_sound(db: &State<Database>, sound_id: String) -> Result<Json<v0::Sound>> {
    db.fetch_sound(&sound_id).await.map(|s| Json(s.into()))
}

/// # Edit Sound
///
/// Rename a sound or move it to another category.
#[openapi(tag = "Soundboard")]
#[patch("/sound/<sound_id>", data = "<data>")]
pub async fn edit_sound(
    db: &State<Database>,
    user: User,
    sound_id: String,
    data: Json<v0::DataEditSound>,
) -> Result<Json<v0::Sound>> {
    let data = data.into_inner();
    data.validate().map_err(|error| {
        create_error!(FailedValidation {
            error: error.to_string()
        })
    })?;

    let mut sound = resolve_and_authorise(db, &user, &sound_id).await?;

    sound
        .update(
            db,
            revolt_database::PartialSound {
                name: data.name,
                category: data.category,
                ..Default::default()
            },
        )
        .await?;

    Ok(Json(sound.into()))
}

/// # Delete Sound
///
/// Delete a sound by its id.
#[openapi(tag = "Soundboard")]
#[delete("/sound/<sound_id>")]
pub async fn delete_sound(
    db: &State<Database>,
    user: User,
    sound_id: String,
) -> Result<EmptyResponse> {
    let sound = resolve_and_authorise(db, &user, &sound_id).await?;
    sound.delete(db).await?;
    Ok(EmptyResponse)
}
