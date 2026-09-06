use crate::model::*;
use crate::DbPool;
use chrono::Utc;
use tracing::{debug, error, instrument};

impl DbPool {
    /// Read app preferences for a hostel, creating defaults if missing.
    #[instrument(skip(self))]
    pub async fn get_app_preferences(&self, hostel_id: i64) -> anyhow::Result<AppPreferences> {
        debug!(hostel_id, "fetching app preferences");

        let row = sqlx::query_as::<_, AppPreferences>(
            r#"
            SELECT hostel_id, date_format, language, receipt_footer, updated_at
            FROM app_preferences
            WHERE hostel_id = ?1
            "#,
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error fetching app preferences"),
        )?;

        if let Some(prefs) = row {
            return Ok(prefs);
        }

        sqlx::query(
            r#"
            INSERT INTO app_preferences (hostel_id)
            VALUES (?1)
            ON CONFLICT(hostel_id) DO NOTHING
            "#,
        )
        .bind(hostel_id)
        .execute(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error creating default app preferences"),
        )?;

        Ok(AppPreferences {
            hostel_id,
            date_format: "dd/MM/yyyy".to_string(),
            language: "en".to_string(),
            receipt_footer: None,
            updated_at: Utc::now(),
        })
    }

    /// Create or update app preferences for a hostel.
    #[instrument(skip(self, prefs))]
    pub async fn upsert_app_preferences(
        &self,
        prefs: UpsertAppPreferences,
    ) -> anyhow::Result<AppPreferences> {
        debug!(hostel_id = prefs.hostel_id, "upserting app preferences");

        let valid_formats = ["dd/MM/yyyy", "MM/dd/yyyy", "yyyy-MM-dd"];
        if !valid_formats.contains(&prefs.date_format.as_str()) {
            anyhow::bail!("date_format must be one of dd/MM/yyyy, MM/dd/yyyy, yyyy-MM-dd");
        }

        if prefs.language != "en" {
            anyhow::bail!("language must be 'en'");
        }

        let row = sqlx::query_as::<_, AppPreferences>(
            r#"
            INSERT INTO app_preferences (hostel_id, date_format, language, receipt_footer, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(hostel_id) DO UPDATE SET
                date_format = excluded.date_format,
                language = excluded.language,
                receipt_footer = excluded.receipt_footer,
                updated_at = excluded.updated_at
            RETURNING hostel_id, date_format, language, receipt_footer, updated_at
            "#,
        )
        .bind(prefs.hostel_id)
        .bind(prefs.date_format)
        .bind(prefs.language)
        .bind(prefs.receipt_footer)
        .bind(Utc::now())
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id = prefs.hostel_id, "database error upserting app preferences"))?;

        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[tokio::test]
    async fn app_preferences_defaults_and_upsert_round_trip() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;

        let prefs = pool
            .get_app_preferences(hostel.id)
            .await
            .expect("get default preferences");
        assert_eq!(prefs.date_format, "dd/MM/yyyy");
        assert_eq!(prefs.language, "en");
        assert!(prefs.receipt_footer.is_none());

        let updated = pool
            .upsert_app_preferences(UpsertAppPreferences {
                hostel_id: hostel.id,
                date_format: "yyyy-MM-dd".to_string(),
                language: "en".to_string(),
                receipt_footer: Some("Thank you for staying with us!".to_string()),
            })
            .await
            .expect("upsert preferences");
        assert_eq!(updated.date_format, "yyyy-MM-dd");
        assert_eq!(
            updated.receipt_footer.as_deref(),
            Some("Thank you for staying with us!")
        );

        let fetched = pool
            .get_app_preferences(hostel.id)
            .await
            .expect("fetch preferences");
        assert_eq!(fetched.date_format, "yyyy-MM-dd");
    }
}
