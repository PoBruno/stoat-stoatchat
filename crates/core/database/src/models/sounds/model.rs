use revolt_models::v0;
use revolt_result::Result;

use crate::events::client::EventV1;
use crate::Database;

auto_derived_partial!(
    /// Soundboard sound
    pub struct Sound {
        /// Unique Id
        ///
        /// This is the id of the uploaded file in autumn, exactly like emoji.
        #[serde(rename = "_id")]
        pub id: String,
        /// What owns this sound
        pub parent: SoundParent,
        /// Uploader user id
        pub creator_id: String,
        /// Sound name
        pub name: String,
        /// Free-form category, defined by whoever uploads
        ///
        /// Deliberately a plain string instead of its own collection: the
        /// category list is derived from the sounds that exist, so a category
        /// cannot linger empty and there is nothing extra to keep in sync.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub category: Option<String>,
        /// Duration in milliseconds, measured by the uploader's browser
        #[serde(skip_serializing_if = "Option::is_none")]
        pub duration: Option<u32>,
    },
    "PartialSound"
);

auto_derived!(
    /// Parent Id of the sound
    #[serde(tag = "type")]
    pub enum SoundParent {
        Server { id: String },
        Detached,
    }
);

#[allow(clippy::disallowed_methods)]
impl Sound {
    /// Get parent id
    fn parent(&self) -> &str {
        match &self.parent {
            SoundParent::Server { id } => id,
            SoundParent::Detached => "",
        }
    }

    /// Create a sound
    pub async fn create(&self, db: &Database) -> Result<()> {
        db.insert_sound(self).await?;

        EventV1::SoundCreate(self.clone().into())
            .p(self.parent().to_string())
            .await;

        Ok(())
    }

    /// Delete a sound
    ///
    /// Detaches rather than removes, like emoji: the underlying file is shared
    /// through its hash, so deleting the row outright would strand it.
    pub async fn delete(&self, db: &Database) -> Result<()> {
        EventV1::SoundDelete {
            id: self.id.to_string(),
        }
        .p(self.parent().to_string())
        .await;

        db.detach_sound(self).await
    }

    /// Update a sound
    pub async fn update(&mut self, db: &Database, partial: PartialSound) -> Result<()> {
        if let Some(name) = partial.name.clone() {
            self.name = name;
        }

        if let Some(category) = partial.category.clone() {
            self.category = Some(category);
        }

        db.update_sound(&self.id, &partial).await?;

        EventV1::SoundUpdate {
            id: self.id.clone(),
            data: v0::PartialSound {
                name: partial.name.clone(),
                category: partial.category.clone(),
            },
        }
        .p(self.parent().to_string())
        .await;

        Ok(())
    }
}
