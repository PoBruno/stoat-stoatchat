use revolt_database::{
    util::{permissions::DatabasePermissionQuery, reference::Reference},
    Database, User,
};
use revolt_models::v0;
use revolt_permissions::PermissionQuery;
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};

/// # Fetch Server Sounds
///
/// Fetch all soundboard sounds on a server.
#[openapi(tag = "Server Customisation")]
#[get("/<target>/sounds")]
pub async fn list_sounds(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
) -> Result<Json<Vec<v0::Sound>>> {
    let server = target.as_server(db).await?;
    let mut query = DatabasePermissionQuery::new(db, &user).server(&server);
    if !query.are_we_a_member().await {
        return Err(create_error!(NotFound));
    }

    db.fetch_sounds_by_parent_id(&server.id)
        .await
        .map(|v| v.into_iter().map(Into::into).collect())
        .map(Json)
}
