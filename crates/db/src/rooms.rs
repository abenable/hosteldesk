use crate::model::*;
use crate::DbPool;
use tracing::{debug, error, info, instrument};

impl DbPool {
    /// Rooms of `hostel_id` by number, each with its class's floor, type, and
    /// prices and how many residents are staying in it.
    #[instrument(skip(self))]
    pub async fn get_rooms(&self, hostel_id: i64) -> anyhow::Result<Vec<Room>> {
        debug!(hostel_id, "fetching rooms");
        let rooms = sqlx::query_as::<_, Room>(
            r#"
            SELECT
                rooms.id, rooms.hostel_id, rooms.room_number, rooms.class_id,
                c.room_type, c.floor, c.semester_rate, c.recess_rate,
                rooms.status, rooms.created_at,
                (SELECT COUNT(*) FROM allocations
                  WHERE allocations.room_id = rooms.id AND allocations.status = 'active'
                ) AS residents
            FROM rooms
            JOIN room_classes c ON c.id = rooms.class_id
            WHERE rooms.hostel_id = ? ORDER BY rooms.room_number
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching rooms"))?;

        Ok(rooms)
    }

    /// Room classes of `hostel_id`, by floor (none first) then type, each with
    /// its number of rooms.
    #[instrument(skip(self))]
    pub async fn get_room_classes(&self, hostel_id: i64) -> anyhow::Result<Vec<RoomClass>> {
        let classes = sqlx::query_as::<_, RoomClass>(
            r#"
            SELECT
                c.id, c.hostel_id, c.public_id, c.floor, c.room_type,
                c.semester_rate, c.recess_rate,
                (SELECT COUNT(*) FROM rooms WHERE rooms.class_id = c.id) AS rooms
            FROM room_classes c
            WHERE c.hostel_id = ?1
            ORDER BY c.floor, c.room_type, c.id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching room classes"))?;
        Ok(classes)
    }

    /// Create a room class and return its id.
    #[instrument(skip(self, class))]
    pub async fn add_room_class(&self, class: NewRoomClass) -> anyhow::Result<i64> {
        check_room_type(&class.room_type)?;
        let mut tx = self.inner.begin().await?;
        let id = sqlx::query(
            r#"
            INSERT INTO room_classes (hostel_id, floor, room_type, semester_rate, recess_rate)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(class.hostel_id)
        .bind(class.floor)
        .bind(&class.room_type)
        .bind(&class.semester_rate)
        .bind(&class.recess_rate)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting room class"))?
        .last_insert_rowid();
        crate::sync::enqueue_room_class(&mut tx, class.hostel_id, id).await?;
        tx.commit().await?;
        info!(class_id = id, "room class inserted");
        Ok(id)
    }

    /// Change a class's floor, type, or prices; its rooms follow. Stays
    /// already agreed keep their rate.
    #[instrument(skip(self, class))]
    pub async fn update_room_class(
        &self,
        class_id: i64,
        class: NewRoomClass,
    ) -> anyhow::Result<()> {
        check_room_type(&class.room_type)?;
        let mut tx = self.inner.begin().await?;
        let updated = sqlx::query(
            r#"
            UPDATE room_classes
            SET floor = ?1, room_type = ?2, semester_rate = ?3, recess_rate = ?4
            WHERE id = ?5 AND hostel_id = ?6
            "#,
        )
        .bind(class.floor)
        .bind(&class.room_type)
        .bind(&class.semester_rate)
        .bind(&class.recess_rate)
        .bind(class_id)
        .bind(class.hostel_id)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, class_id, "database error updating room class"))?
        .rows_affected();
        if updated == 0 {
            anyhow::bail!("room class not found");
        }
        crate::sync::enqueue_room_class(&mut tx, class.hostel_id, class_id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Insert a single room with `status = 'vacant'` and return its id.
    #[instrument(skip(self, room))]
    pub async fn add_room(&self, room: NewRoom) -> anyhow::Result<i64> {
        let ids = self.add_rooms(std::slice::from_ref(&room)).await?;
        ids.first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("room was not inserted"))
    }

    /// Insert several rooms in one transaction: all of them, or none when any
    /// fails (a number already used, a class of another hostel).
    #[instrument(skip(self, rooms), fields(count = rooms.len()))]
    pub async fn add_rooms(&self, rooms: &[NewRoom]) -> anyhow::Result<Vec<i64>> {
        let mut tx = self
            .inner
            .begin()
            .await
            .inspect_err(|e| error!(error = %e, "database error starting transaction"))?;
        let mut ids = Vec::with_capacity(rooms.len());
        for room in rooms {
            debug!(hostel_id = room.hostel_id, room_number = %room.room_number, "adding room");
            let id = insert_room(&mut tx, room).await?;
            crate::sync::enqueue_room(&mut tx, room.hostel_id, id).await?;
            ids.push(id);
        }
        tx.commit()
            .await
            .inspect_err(|e| error!(error = %e, "database error committing rooms"))?;
        info!(count = ids.len(), "rooms inserted");
        Ok(ids)
    }
}

fn check_room_type(room_type: &str) -> anyhow::Result<()> {
    if !matches!(room_type, "single" | "double" | "dorm") {
        anyhow::bail!("room_type must be one of: single, double, dorm");
    }
    Ok(())
}

/// Insert a room on the given transaction after checking its class belongs to
/// the same hostel. Used by [`DbPool::add_rooms`].
async fn insert_room(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    room: &NewRoom,
) -> anyhow::Result<i64> {
    let class: Option<i64> =
        sqlx::query_scalar("SELECT id FROM room_classes WHERE id = ?1 AND hostel_id = ?2")
            .bind(room.class_id)
            .bind(room.hostel_id)
            .fetch_optional(&mut **tx)
            .await?;
    if class.is_none() {
        anyhow::bail!("room class not found");
    }
    let id = sqlx::query(
        r#"
        INSERT INTO rooms (hostel_id, room_number, class_id, status)
        VALUES (?1, ?2, ?3, 'vacant')
        "#,
    )
    .bind(room.hostel_id)
    .bind(&room.room_number)
    .bind(room.class_id)
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
            "INSERT INTO room_classes (hostel_id, room_type, semester_rate, recess_rate) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(hostel.id)
        .bind("suite")
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

    /// Rooms must never leak across hostels when filtered by `hostel_id`, and
    /// a room cannot use another hostel's class.
    #[tokio::test]
    async fn get_rooms_isolated_by_hostel_id() {
        let pool = test_pool().await;
        let hostel_a = seed_hostel(&pool).await;
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
        let class_a = seed_class(&pool, hostel_a.id, "single").await;

        pool.add_room(NewRoom {
            hostel_id: hostel_a.id,
            room_number: "101".to_string(),
            class_id: class_a,
        })
        .await
        .expect("add room to hostel a");
        let foreign = pool
            .add_room(NewRoom {
                hostel_id: hostel_b.id,
                room_number: "101".to_string(),
                class_id: class_a,
            })
            .await
            .expect_err("hostel a's class is not hostel b's");
        assert!(foreign.to_string().contains("room class"), "{foreign}");

        assert!(pool
            .get_rooms(hostel_b.id)
            .await
            .expect("b rooms")
            .is_empty());
        let a_rooms = pool.get_rooms(hostel_a.id).await.expect("a rooms");
        assert_eq!(a_rooms.len(), 1);
        assert_eq!(a_rooms[0].room_number, "101");
        assert_eq!(a_rooms[0].room_type, "single");
    }

    /// The setup wizard adds a run of rooms at once: one bad number adds none.
    #[tokio::test]
    async fn adding_rooms_is_all_or_nothing() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let class_id = seed_class(&pool, hostel.id, "double").await;
        let room = |n: &str| NewRoom {
            hostel_id: hostel.id,
            room_number: n.to_string(),
            class_id,
        };
        pool.add_rooms(&[room("A01"), room("A02"), room("A01")])
            .await
            .expect_err("A01 twice");
        assert!(pool.get_rooms(hostel.id).await.expect("rooms").is_empty());
        let ids = pool
            .add_rooms(&[room("A01"), room("A02")])
            .await
            .expect("two rooms");
        assert_eq!(ids.len(), 2);
        let classes = pool.get_room_classes(hostel.id).await.expect("classes");
        assert_eq!(classes[0].rooms, 2);
    }

    /// Editing a class re-prices its rooms; stays already agreed keep theirs.
    #[tokio::test]
    async fn rooms_follow_their_class() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let class_id = seed_class(&pool, hostel.id, "single").await;
        let room_id = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                class_id,
            })
            .await
            .expect("room");
        let resident = seed_resident(&pool, "Ada").await;
        seed_active_allocation(&pool, hostel.id, resident, room_id).await;
        pool.update_room_class(
            class_id,
            NewRoomClass {
                hostel_id: hostel.id,
                floor: Some(2),
                room_type: "double".to_string(),
                semester_rate: "900".to_string(),
                recess_rate: "300".to_string(),
            },
        )
        .await
        .expect("edit class");
        let rooms = pool.get_rooms(hostel.id).await.expect("rooms");
        assert_eq!(
            (
                rooms[0].floor,
                rooms[0].room_type.as_str(),
                rooms[0].semester_rate.as_str()
            ),
            (Some(2), "double", "900")
        );
        assert_eq!(rooms[0].residents, 1);
    }
}
