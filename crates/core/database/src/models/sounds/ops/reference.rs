use revolt_result::Result;

use crate::ReferenceDb;
use crate::SoundParent;
use crate::{PartialSound, Sound};

use super::AbstractSounds;

#[async_trait]
impl AbstractSounds for ReferenceDb {
    /// Insert sound into database.
    async fn insert_sound(&self, sound: &Sound) -> Result<()> {
        let mut sounds = self.sounds.lock().await;
        if sounds.contains_key(&sound.id) {
            Err(create_database_error!("insert", "sound"))
        } else {
            sounds.insert(sound.id.to_string(), sound.clone());
            Ok(())
        }
    }

    /// Fetch a sound by its id
    async fn fetch_sound(&self, id: &str) -> Result<Sound> {
        let sounds = self.sounds.lock().await;
        sounds
            .get(id)
            .cloned()
            .ok_or_else(|| create_error!(NotFound))
    }

    /// Fetch sounds by their parent id
    async fn fetch_sounds_by_parent_id(&self, parent_id: &str) -> Result<Vec<Sound>> {
        let sounds = self.sounds.lock().await;
        Ok(sounds
            .values()
            .filter(|sound| match &sound.parent {
                SoundParent::Server { id } => id == parent_id,
                _ => false,
            })
            .cloned()
            .collect())
    }

    /// Fetch sounds by their parent ids
    async fn fetch_sounds_by_parent_ids(&self, parent_ids: &[String]) -> Result<Vec<Sound>> {
        let sounds = self.sounds.lock().await;
        Ok(sounds
            .values()
            .filter(|sound| match &sound.parent {
                SoundParent::Server { id } => parent_ids.contains(id),
                _ => false,
            })
            .cloned()
            .collect())
    }

    /// Update sound with new information
    async fn update_sound(&self, sound_id: &str, partial: &PartialSound) -> Result<()> {
        let mut sounds = self.sounds.lock().await;
        if let Some(sound) = sounds.get_mut(sound_id) {
            if let Some(name) = partial.name.clone() {
                sound.name = name;
            }
            if let Some(category) = partial.category.clone() {
                sound.category = Some(category);
            }
            Ok(())
        } else {
            Err(create_error!(NotFound))
        }
    }

    /// Detach a sound by its id
    async fn detach_sound(&self, sound: &Sound) -> Result<()> {
        let mut sounds = self.sounds.lock().await;
        if let Some(alvo) = sounds.get_mut(&sound.id) {
            alvo.parent = SoundParent::Detached;
            Ok(())
        } else {
            Err(create_error!(NotFound))
        }
    }
}
