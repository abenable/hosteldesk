//! Tauri IPC commands. Each one forwards to the [`Desk`] method of the same
//! name and adds nothing: the contract (`request` argument, snake_case
//! fields, plain-string errors) is `specs/v2.1.0-ui-rebuild.md`.
//!
//! Commands are async so the window thread never waits on SQLite, a PDF, or
//! the network; async commands that borrow state must return `Result`.

use std::sync::Arc;

use tauri::State;

use crate::commands::{
    AddRoomRequest, CheckInRequest, CheckOutRequest, CompleteOnboardingRequest, CreateUserRequest,
    DeleteUserRequest, GetAppPreferencesRequest, GetDashboardStatsRequest,
    GetResidentDetailRequest, ListActiveAllocationsRequest, ListRecentActivityRequest,
    ListResidentsRequest, ListUsersRequest, LoginRequest, OpenReceiptRequest, RestoreBackupRequest,
    TakePaymentRequest, TakePaymentResult, UpdateHostelRequest, UpdateResidentRequest,
    UpdateUserRequest, UploadHostelLogoRequest, UpsertAppPreferencesRequest,
};
use crate::{
    AccountLicense, BootState, CloudStatus, ConnectCloudRequest, ConnectResult, Desk, SyncEvent,
};

type DeskState<'a> = State<'a, Arc<Desk>>;

/// Every command, for [`tauri::Builder::invoke_handler`].
pub(crate) fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        boot_state,
        connect_cloud,
        sync_now,
        cloud_status,
        cloud_enrolled,
        take_payment,
        open_receipt,
        license_status,
        cached_license,
        app_version,
        session,
        logout,
        create_user,
        login,
        list_users,
        update_user,
        delete_user,
        onboarding_status,
        complete_onboarding,
        hostel_profile,
        list_hostels,
        update_hostel,
        upload_hostel_logo,
        rooms,
        add_room,
        check_in,
        check_out,
        active_allocations,
        list_payments,
        dashboard_stats,
        recent_activity,
        list_residents,
        resident_detail,
        update_resident,
        app_preferences,
        update_app_preferences,
        backup_database,
        list_backups,
        restore_backup,
    ]
}

/// [`Desk::boot_state`].
#[tauri::command]
pub async fn boot_state(desk: DeskState<'_>) -> Result<BootState, String> {
    desk.boot_state().await
}

/// [`Desk::connect_cloud`].
#[tauri::command]
pub async fn connect_cloud(
    desk: DeskState<'_>,
    request: ConnectCloudRequest,
) -> Result<ConnectResult, String> {
    desk.connect_cloud(request).await
}

/// [`Desk::sync_now`].
#[tauri::command]
pub async fn sync_now(desk: DeskState<'_>) -> Result<SyncEvent, String> {
    Ok(desk.sync_now().await)
}

/// [`Desk::cloud_status`].
#[tauri::command]
pub async fn cloud_status(desk: DeskState<'_>) -> Result<CloudStatus, String> {
    desk.cloud_status().await
}

/// [`Desk::cloud_enrolled`].
#[tauri::command]
pub async fn cloud_enrolled(desk: DeskState<'_>) -> Result<bool, String> {
    Ok(desk.cloud_enrolled())
}

/// [`Desk::take_payment`].
#[tauri::command]
pub async fn take_payment(
    desk: DeskState<'_>,
    request: TakePaymentRequest,
) -> Result<TakePaymentResult, String> {
    desk.take_payment(request).await
}

/// [`Desk::open_receipt`].
#[tauri::command]
pub async fn open_receipt(desk: DeskState<'_>, request: OpenReceiptRequest) -> Result<(), String> {
    desk.open_receipt(request).await
}

/// [`Desk::license_status`].
#[tauri::command]
pub async fn license_status(desk: DeskState<'_>) -> Result<Option<AccountLicense>, String> {
    Ok(desk.license_status())
}

/// [`Desk::cached_license`].
#[tauri::command]
pub async fn cached_license(desk: DeskState<'_>) -> Result<Option<AccountLicense>, String> {
    Ok(desk.cached_license())
}

/// [`Desk::app_version`].
#[tauri::command]
pub async fn app_version(desk: DeskState<'_>) -> Result<&'static str, String> {
    Ok(desk.app_version())
}

/// [`Desk::session`].
#[tauri::command]
pub async fn session(desk: DeskState<'_>) -> Result<Option<db::User>, String> {
    desk.session().await
}

/// [`Desk::logout`].
#[tauri::command]
pub async fn logout(desk: DeskState<'_>) -> Result<(), String> {
    desk.logout();
    Ok(())
}

/// [`Desk::create_user`].
#[tauri::command]
pub async fn create_user(desk: DeskState<'_>, request: CreateUserRequest) -> Result<i64, String> {
    desk.create_user(request).await
}

/// [`Desk::login`].
#[tauri::command]
pub async fn login(desk: DeskState<'_>, request: LoginRequest) -> Result<db::User, String> {
    desk.login(request).await
}

/// [`Desk::list_users`].
#[tauri::command]
pub async fn list_users(
    desk: DeskState<'_>,
    request: ListUsersRequest,
) -> Result<Vec<db::User>, String> {
    desk.list_users(request).await
}

/// [`Desk::update_user`].
#[tauri::command]
pub async fn update_user(
    desk: DeskState<'_>,
    request: UpdateUserRequest,
) -> Result<db::User, String> {
    desk.update_user(request).await
}

/// [`Desk::delete_user`].
#[tauri::command]
pub async fn delete_user(desk: DeskState<'_>, request: DeleteUserRequest) -> Result<(), String> {
    desk.delete_user(request).await
}

/// [`Desk::onboarding_status`].
#[tauri::command]
pub async fn onboarding_status(desk: DeskState<'_>) -> Result<bool, String> {
    desk.onboarding_status().await
}

/// [`Desk::complete_onboarding`].
#[tauri::command]
pub async fn complete_onboarding(
    desk: DeskState<'_>,
    request: CompleteOnboardingRequest,
) -> Result<db::User, String> {
    desk.complete_onboarding(request).await
}

/// [`Desk::hostel_profile`]. JS passes `{ hostelId }`.
#[tauri::command]
pub async fn hostel_profile(
    desk: DeskState<'_>,
    hostel_id: i64,
) -> Result<Option<db::HostelProfile>, String> {
    desk.hostel_profile(hostel_id).await
}

/// [`Desk::list_hostels`].
#[tauri::command]
pub async fn list_hostels(desk: DeskState<'_>) -> Result<Vec<db::Hostel>, String> {
    desk.list_hostels().await
}

/// [`Desk::update_hostel`].
#[tauri::command]
pub async fn update_hostel(
    desk: DeskState<'_>,
    request: UpdateHostelRequest,
) -> Result<db::Hostel, String> {
    desk.update_hostel(request).await
}

/// [`Desk::upload_hostel_logo`].
#[tauri::command]
pub async fn upload_hostel_logo(
    desk: DeskState<'_>,
    request: UploadHostelLogoRequest,
) -> Result<String, String> {
    desk.upload_hostel_logo(request).await
}

/// [`Desk::rooms`]. JS passes `{ hostelId }`.
#[tauri::command]
pub async fn rooms(desk: DeskState<'_>, hostel_id: i64) -> Result<Vec<db::Room>, String> {
    desk.rooms(hostel_id).await
}

/// [`Desk::add_room`].
#[tauri::command]
pub async fn add_room(desk: DeskState<'_>, request: AddRoomRequest) -> Result<i64, String> {
    desk.add_room(request).await
}

/// [`Desk::check_in`].
#[tauri::command]
pub async fn check_in(desk: DeskState<'_>, request: CheckInRequest) -> Result<i64, String> {
    desk.check_in(request).await
}

/// [`Desk::check_out`].
#[tauri::command]
pub async fn check_out(desk: DeskState<'_>, request: CheckOutRequest) -> Result<(), String> {
    desk.check_out(request).await
}

/// [`Desk::active_allocations`].
#[tauri::command]
pub async fn active_allocations(
    desk: DeskState<'_>,
    request: ListActiveAllocationsRequest,
) -> Result<Vec<db::ActiveAllocation>, String> {
    desk.active_allocations(request).await
}

/// [`Desk::list_payments`].
#[tauri::command]
pub async fn list_payments(
    desk: DeskState<'_>,
    request: db::ListPaymentsFilter,
) -> Result<Vec<db::PaymentRow>, String> {
    desk.list_payments(request).await
}

/// [`Desk::dashboard_stats`].
#[tauri::command]
pub async fn dashboard_stats(
    desk: DeskState<'_>,
    request: GetDashboardStatsRequest,
) -> Result<db::DashboardStats, String> {
    desk.dashboard_stats(request).await
}

/// [`Desk::recent_activity`].
#[tauri::command]
pub async fn recent_activity(
    desk: DeskState<'_>,
    request: ListRecentActivityRequest,
) -> Result<Vec<db::ActivityItem>, String> {
    desk.recent_activity(request).await
}

/// [`Desk::list_residents`].
#[tauri::command]
pub async fn list_residents(
    desk: DeskState<'_>,
    request: ListResidentsRequest,
) -> Result<Vec<db::ResidentSummary>, String> {
    desk.list_residents(request).await
}

/// [`Desk::resident_detail`].
#[tauri::command]
pub async fn resident_detail(
    desk: DeskState<'_>,
    request: GetResidentDetailRequest,
) -> Result<db::ResidentDetail, String> {
    desk.resident_detail(request).await
}

/// [`Desk::update_resident`].
#[tauri::command]
pub async fn update_resident(
    desk: DeskState<'_>,
    request: UpdateResidentRequest,
) -> Result<(), String> {
    desk.update_resident(request).await
}

/// [`Desk::app_preferences`].
#[tauri::command]
pub async fn app_preferences(
    desk: DeskState<'_>,
    request: GetAppPreferencesRequest,
) -> Result<db::AppPreferences, String> {
    desk.app_preferences(request).await
}

/// [`Desk::update_app_preferences`].
#[tauri::command]
pub async fn update_app_preferences(
    desk: DeskState<'_>,
    request: UpsertAppPreferencesRequest,
) -> Result<db::AppPreferences, String> {
    desk.update_app_preferences(request).await
}

/// [`Desk::backup_database`].
#[tauri::command]
pub async fn backup_database(desk: DeskState<'_>) -> Result<String, String> {
    desk.backup_database().await
}

/// [`Desk::list_backups`].
#[tauri::command]
pub async fn list_backups(desk: DeskState<'_>) -> Result<Vec<String>, String> {
    desk.list_backups().await
}

/// [`Desk::restore_backup`].
#[tauri::command]
pub async fn restore_backup(
    desk: DeskState<'_>,
    request: RestoreBackupRequest,
) -> Result<(), String> {
    desk.restore_backup(request).await
}
