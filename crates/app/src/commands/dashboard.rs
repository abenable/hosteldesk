use super::support::*;
use chrono::NaiveDate;
use db::{ActivityItem, DashboardStats, DbPool};
use serde::Deserialize;
use tracing::{info, instrument};

/// Request payload for [`get_dashboard_stats`].
#[derive(Debug, Deserialize)]
pub struct GetDashboardStatsRequest {
    pub hostel_id: i64,
    pub today: NaiveDate,
}

/// Return dashboard statistics for a hostel on the supplied day.
#[instrument(skip(state, license_state), fields(command = "get_dashboard_stats"))]
pub async fn get_dashboard_stats(
    request: GetDashboardStatsRequest,
    state: &DbPool,
    license_state: &crate::LicenseState,
    auth_state: &crate::AuthState,
) -> Result<DashboardStats, String> {
    info!(
        hostel_id = request.hostel_id,
        today = %request.today,
        "get_dashboard_stats invoked"
    );
    require_license(license_state)?;
    require_session(auth_state, request.hostel_id)?;
    state
        .get_dashboard_stats(request.hostel_id, request.today)
        .await
        .map_err(|e| map_db_error("get_dashboard_stats", e))
}

/// Request payload for [`list_recent_activity`].
#[derive(Debug, Deserialize)]
pub struct ListRecentActivityRequest {
    pub hostel_id: i64,
    pub limit: i64,
}

/// Return recent check-ins, check-outs and payments for the activity feed.
#[instrument(skip(state, license_state), fields(command = "list_recent_activity"))]
pub async fn list_recent_activity(
    request: ListRecentActivityRequest,
    state: &DbPool,
    license_state: &crate::LicenseState,
    auth_state: &crate::AuthState,
) -> Result<Vec<ActivityItem>, String> {
    info!(
        hostel_id = request.hostel_id,
        "list_recent_activity invoked"
    );
    require_license(license_state)?;
    require_session(auth_state, request.hostel_id)?;
    state
        .list_recent_activity(request.hostel_id, request.limit)
        .await
        .map_err(|e| map_db_error("list_recent_activity", e))
}
