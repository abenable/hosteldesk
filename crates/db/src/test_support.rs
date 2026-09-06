use crate::allocations::insert_allocation;
use crate::model::*;
use crate::residents::insert_resident;
use crate::DbPool;
use chrono::{Local, NaiveDate};

/// Open an in-memory SQLite pool with migrations applied for tests.
pub(crate) async fn test_pool() -> DbPool {
    DbPool::open("sqlite::memory:").await.expect("open test db")
}

pub(crate) fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub(crate) async fn seed_hostel(pool: &DbPool) -> Hostel {
    pool.create_hostel(NewHostel {
        name: "Test Hostel".to_string(),
        address: None,
        phone: None,
        currency: None,
        branding_logo_path: None,
    })
    .await
    .expect("seed hostel")
}

pub(crate) async fn seed_room(
    pool: &DbPool,
    hostel_id: i64,
    room_number: &str,
    room_type: &str,
) -> i64 {
    pool.add_room(NewRoom {
        hostel_id,
        room_number: room_number.to_string(),
        room_type: room_type.to_string(),
        capacity: 1,
        floor: None,
        semester_rate: "100".to_string(),
        recess_rate: "50".to_string(),
    })
    .await
    .expect("seed room")
}

pub(crate) async fn seed_resident(pool: &DbPool, name: &str) -> i64 {
    let mut tx = pool.inner.begin().await.expect("begin seed resident tx");
    let id = insert_resident(
        &mut tx,
        &NewResident {
            full_name: name.to_string(),
            phone: None,
            email: None,
            id_number: None,
            next_of_kin: None,
            next_of_kin_phone: None,
            photo_path: None,
        },
    )
    .await
    .expect("seed resident");
    tx.commit().await.expect("commit seed resident tx");
    id
}

pub(crate) async fn seed_active_allocation(
    pool: &DbPool,
    hostel_id: i64,
    resident_id: i64,
    room_id: i64,
) -> i64 {
    let mut tx = pool.inner.begin().await.expect("begin seed allocation tx");
    let id = insert_allocation(
        &mut tx,
        &NewAllocation {
            hostel_id,
            resident_id,
            room_id,
            rate_agreed: "100".to_string(),
            term: "semester_1".to_string(),
            academic_year: "2026/2027".to_string(),
        },
    )
    .await
    .expect("seed allocation");
    tx.commit().await.expect("commit seed allocation tx");
    id
}
