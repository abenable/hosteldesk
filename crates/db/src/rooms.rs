use crate::model::*;
use crate::DbPool;
use tracing::{debug, error, info, instrument};

impl DbPool {
    #[instrument(skip(self))]
    pub async fn get_rooms(&self, hostel_id: i64) -> anyhow::Result<Vec<Room>> {
        debug!(hostel_id, "fetching rooms");
        let rooms = sqlx::query_as::<_, Room>(
            r#"
            SELECT
                id, hostel_id, room_number, room_type, capacity, floor,
                semester_rate, recess_rate, status, created_at,
                (SELECT COUNT(*) FROM allocations
                  WHERE allocations.room_id = rooms.id AND allocations.status = 'active'
                ) AS occupied_beds
            FROM rooms WHERE hostel_id = ? ORDER BY room_number
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching rooms"))?;

        Ok(rooms)
    }

    /// Insert a single room with `status = 'vacant'` and return its id.
    ///
    /// Validates `capacity > 0` and `room_type` ∈ {single, double, dorm}.
    #[instrument(skip(self, room))]
    pub async fn add_room(&self, room: NewRoom) -> anyhow::Result<i64> {
        debug!(hostel_id = room.hostel_id, room_number = %room.room_number, "adding room");
        let mut tx = self
            .inner
            .begin()
            .await
            .inspect_err(|e| error!(error = %e, "database error starting transaction"))?;
        let id = insert_room(&mut tx, &room).await?;
        tx.commit()
            .await
            .inspect_err(|e| error!(error = %e, "database error committing room"))?;
        info!(room_id = id, "room inserted");
        Ok(id)
    }
}

/// Insert a room on the given transaction, validating capacity and room_type.
///
/// Used by [`DbPool::add_room`].
async fn insert_room(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    room: &NewRoom,
) -> anyhow::Result<i64> {
    if room.capacity <= 0 {
        anyhow::bail!("capacity must be greater than zero");
    }
    if !matches!(room.room_type.as_str(), "single" | "double" | "dorm") {
        anyhow::bail!("room_type must be one of: single, double, dorm");
    }
    let id = sqlx::query(
        r#"
        INSERT INTO rooms (hostel_id, room_number, room_type, capacity, floor, semester_rate, recess_rate, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'vacant')
        "#,
    )
    .bind(room.hostel_id)
    .bind(&room.room_number)
    .bind(&room.room_type)
    .bind(room.capacity)
    .bind(room.floor)
    .bind(&room.semester_rate)
    .bind(&room.recess_rate)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, hostel_id = room.hostel_id, room_number = %room.room_number, "database error inserting room"))?
    .last_insert_rowid();
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[tokio::test]
    async fn room_type_check_rejects_invalid_value() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let err = sqlx::query(
            "INSERT INTO rooms (hostel_id, room_number, room_type, capacity, semester_rate, recess_rate) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(hostel.id)
        .bind("101")
        .bind("suite")
        .bind(1)
        .bind("100.00")
        .bind("50.00")
        .execute(&pool.inner)
        .await
        .expect_err("invalid room_type should fail CHECK");
        assert!(
            err.to_string().contains("CHECK") || err.to_string().contains("constraint"),
            "expected CHECK constraint failure, got: {err}"
        );
    }

    /// Rooms must never leak across hostels when filtered by `hostel_id`.
    #[tokio::test]
    async fn get_rooms_isolated_by_hostel_id() {
        let pool = test_pool().await;
        let hostel_a = pool
            .create_hostel(NewHostel {
                name: "Hostel A".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel a");
        let hostel_b = pool
            .create_hostel(NewHostel {
                name: "Hostel B".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel b");

        pool.add_room(NewRoom {
            hostel_id: hostel_a.id,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 1,
            floor: None,
            semester_rate: "10".to_string(),
            recess_rate: "10".to_string(),
        })
        .await
        .expect("add room to hostel a");

        assert!(
            pool.get_rooms(hostel_b.id)
                .await
                .expect("fetch hostel b rooms")
                .is_empty(),
            "hostel b should have no rooms"
        );
        let a_rooms = pool
            .get_rooms(hostel_a.id)
            .await
            .expect("fetch hostel a rooms");
        assert_eq!(a_rooms.len(), 1);
        assert_eq!(a_rooms[0].room_number, "101");
    }
}
