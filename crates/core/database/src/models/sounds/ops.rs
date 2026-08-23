use revolt_result::Result;

use crate::{PartialSound, Sound};

#[cfg(feature = "mongodb")]
mod mongodb;
mod reference;

#[async_trait]
pub trait AbstractSounds: Sync + Send {
    /// Insert sound into database.
    async fn insert_sound(&self, sound: &Sound) -> Result<()>;

    /// Fetch a sound by its id
    async fn fetch_sound(&self, id: &str) -> Result<Sound>;

    /// Fetch sounds by their parent id
    async fn fetch_sounds_by_parent_id(&self, parent_id: &str) -> Result<Vec<Sound>>;

    /// Fetch sounds by their parent ids
    async fn fetch_sounds_by_parent_ids(&self, parent_ids: &[String]) -> Result<Vec<Sound>>;

    /// Update sound with new information
    async fn update_sound(&self, sound_id: &str, partial: &PartialSound) -> Result<()>;

    /// Detach a sound by its id
    async fn detach_sound(&self, sound: &Sound) -> Result<()>;
}
