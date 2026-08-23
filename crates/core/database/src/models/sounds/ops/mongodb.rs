use bson::Document;
use revolt_result::Result;

use crate::MongoDb;
use crate::{PartialSound, Sound};

use super::AbstractSounds;

static COL: &str = "sounds";

#[async_trait]
impl AbstractSounds for MongoDb {
    /// Insert sound into database.
    async fn insert_sound(&self, sound: &Sound) -> Result<()> {
        query!(self, insert_one, COL, &sound).map(|_| ())
    }

    /// Fetch a sound by its id
    async fn fetch_sound(&self, id: &str) -> Result<Sound> {
        query!(self, find_one_by_id, COL, id)?.ok_or_else(|| create_error!(NotFound))
    }

    /// Fetch sounds by their parent id
    async fn fetch_sounds_by_parent_id(&self, parent_id: &str) -> Result<Vec<Sound>> {
        query!(
            self,
            find,
            COL,
            doc! {
                "parent.id": parent_id
            }
        )
    }

    /// Fetch sounds by their parent ids
    async fn fetch_sounds_by_parent_ids(&self, parent_ids: &[String]) -> Result<Vec<Sound>> {
        query!(
            self,
            find,
            COL,
            doc! {
                "parent.id": {
                    "$in": parent_ids
                }
            }
        )
    }

    /// Update sound with new information
    async fn update_sound(&self, sound_id: &str, partial: &PartialSound) -> Result<()> {
        query!(self, update_one_by_id, COL, sound_id, partial, vec![], None).map(|_| ())
    }

    /// Detach a sound by its id
    async fn detach_sound(&self, sound: &Sound) -> Result<()> {
        self.col::<Document>(COL)
            .update_one(
                doc! {
                    "_id": &sound.id
                },
                doc! {
                    "$set": {
                        "parent": {
                            "type": "Detached"
                        }
                    }
                },
            )
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("update_one", COL))
    }
}
