//! Application service used by the egui shell.
//!
//! Each method is the behavior that used to live behind a Tauri command.
//! Widgets call [`Desk`] and do not touch the database or decide permissions.

use db::DbPool;

use crate::commands;
use crate::paths::AppPaths;
use crate::state::{AuthState, LicenseState};

/// Live hostel desk: database, session, license, and on-disk paths.
pub struct Desk {
    /// SQLite pool for the local ledger.
    pub(crate) db: DbPool,
    /// In-memory staff session.
    pub(crate) auth: AuthState,
    /// What the server last said about the account's license.
    pub(crate) license: LicenseState,
    /// Data and documents directories.
    pub(crate) paths: AppPaths,
}

impl Desk {
    /// Open the database in `paths` and load the cached account license.
    pub async fn open(paths: AppPaths) -> Result<Self, String> {
        std::fs::create_dir_all(&paths.data_dir)
            .map_err(|e| format!("could not create app data dir: {e}"))?;
        let db = DbPool::open(&paths.database_url())
            .await
            .map_err(|e| format!("could not open database: {e}"))?;
        let license = LicenseState::default();
        if let Some(cached) = commands::load_license(&paths) {
            license.set(cached);
        }
        Ok(Self {
            db,
            auth: AuthState::new(),
            license,
            paths,
        })
    }

    /// Build a desk around an already-open pool. Used by tests.
    #[cfg(test)]
    pub(crate) fn from_parts(
        db: DbPool,
        auth: AuthState,
        license: LicenseState,
        paths: AppPaths,
    ) -> Self {
        Self {
            db,
            auth,
            license,
            paths,
        }
    }

    /// Currently loaded license, if activation succeeded and the file verified.
    pub fn license_status(&self) -> Option<crate::state::AccountLicense> {
        commands::get_license_status(&self.license)
    }

    /// The server's last answer about this account's license, usable or not.
    pub fn cached_license(&self) -> Option<crate::state::AccountLicense> {
        self.license.get()
    }

    /// Cache the server's answer about this account's license.
    pub(crate) fn store_license(
        &self,
        license: crate::state::AccountLicense,
    ) -> Result<(), String> {
        commands::store_license(license, &self.license, &self.paths)
    }

    /// `CARGO_PKG_VERSION` of the running binary.
    pub fn app_version(&self) -> &'static str {
        commands::get_app_version()
    }

    /// Staff user for the current session, if one is still signed in.
    pub async fn session(&self) -> Result<Option<db::User>, String> {
        commands::get_session(&self.db, &self.auth).await
    }

    /// Clear the in-memory staff session.
    pub fn logout(&self) {
        commands::logout(&self.auth);
    }

    /// Create a staff account. Requires an admin session.
    pub async fn create_user(&self, request: commands::CreateUserRequest) -> Result<i64, String> {
        commands::create_user(request, &self.db, &self.license, &self.auth).await
    }

    /// Sign a staff user in and start their session.
    pub async fn login(&self, request: commands::LoginRequest) -> Result<db::User, String> {
        commands::login(request, &self.db, &self.license, &self.auth).await
    }

    /// List staff for a hostel. Admin only.
    pub async fn list_users(
        &self,
        request: commands::ListUsersRequest,
    ) -> Result<Vec<db::User>, String> {
        commands::list_users(request, &self.db, &self.license, &self.auth).await
    }

    /// Update a staff profile or password.
    pub async fn update_user(
        &self,
        request: commands::UpdateUserRequest,
    ) -> Result<db::User, String> {
        commands::update_user(request, &self.db, &self.license, &self.auth).await
    }

    /// Delete a staff user. Admin only, and never the last admin.
    pub async fn delete_user(&self, request: commands::DeleteUserRequest) -> Result<(), String> {
        commands::delete_user(request, &self.db, &self.license, &self.auth).await
    }

    /// Whether the first hostel and staff account already exist.
    pub async fn onboarding_status(&self) -> Result<bool, String> {
        commands::get_onboarding_status(&self.db).await
    }

    /// Create the hostel and its first administrator.
    pub async fn complete_onboarding(
        &self,
        request: commands::CompleteOnboardingRequest,
    ) -> Result<db::User, String> {
        commands::complete_onboarding(request, &self.db, &self.license, &self.auth).await
    }

    /// Hostel profile plus room and admin counts.
    pub async fn hostel_profile(
        &self,
        hostel_id: i64,
    ) -> Result<Option<db::HostelProfile>, String> {
        commands::get_hostel_profile(hostel_id, &self.db, &self.license, &self.auth).await
    }

    /// Hostels the current session may see.
    pub async fn list_hostels(&self) -> Result<Vec<db::Hostel>, String> {
        commands::list_hostels(&self.db, &self.license, &self.auth).await
    }

    /// Update the hostel profile. Admin only.
    pub async fn update_hostel(
        &self,
        request: commands::UpdateHostelRequest,
    ) -> Result<db::Hostel, String> {
        commands::update_hostel(request, &self.db, &self.license, &self.auth).await
    }

    /// Store a logo image and return its path. Admin only.
    pub async fn upload_hostel_logo(
        &self,
        request: commands::UploadHostelLogoRequest,
    ) -> Result<String, String> {
        commands::upload_hostel_logo(request, &self.paths, &self.license, &self.auth).await
    }

    /// Rooms for one hostel.
    pub async fn rooms(&self, hostel_id: i64) -> Result<Vec<db::Room>, String> {
        commands::get_rooms(hostel_id, &self.db, &self.license, &self.auth).await
    }

    /// Add one room.
    pub async fn add_room(&self, request: commands::AddRoomRequest) -> Result<i64, String> {
        commands::add_room(request, &self.db, &self.license, &self.auth).await
    }

    /// Check a resident in. Returns the new allocation id.
    pub async fn check_in(&self, request: commands::CheckInRequest) -> Result<i64, String> {
        commands::check_in(request, &self.db, &self.license, &self.auth).await
    }

    /// Check an allocation out.
    pub async fn check_out(&self, request: commands::CheckOutRequest) -> Result<(), String> {
        commands::check_out(request, &self.db, &self.license, &self.auth).await
    }

    /// Active stays for the payment form.
    pub async fn active_allocations(
        &self,
        request: commands::ListActiveAllocationsRequest,
    ) -> Result<Vec<db::ActiveAllocation>, String> {
        commands::list_active_allocations(request, &self.db, &self.license, &self.auth).await
    }

    /// Record a payment and allocate the next receipt number.
    pub async fn record_payment_and_generate_receipt(
        &self,
        request: commands::RecordPaymentRequest,
    ) -> Result<(i64, String), String> {
        commands::record_payment_and_generate_receipt(request, &self.db, &self.license, &self.auth)
            .await
    }

    /// Write a PDF receipt and open it.
    pub async fn generate_and_open_receipt(
        &self,
        request: commands::GenerateReceiptRequest,
    ) -> Result<String, String> {
        commands::generate_and_open_receipt(
            request,
            &self.paths,
            &self.db,
            &self.license,
            &self.auth,
        )
        .await
    }

    /// Payments ledger.
    pub async fn list_payments(
        &self,
        request: db::ListPaymentsFilter,
    ) -> Result<Vec<db::PaymentRow>, String> {
        commands::list_payments(request, &self.db, &self.license, &self.auth).await
    }

    /// Occupancy and revenue figures.
    pub async fn dashboard_stats(
        &self,
        request: commands::GetDashboardStatsRequest,
    ) -> Result<db::DashboardStats, String> {
        commands::get_dashboard_stats(request, &self.db, &self.license, &self.auth).await
    }

    /// Recent check-ins, check-outs, and payments.
    pub async fn recent_activity(
        &self,
        request: commands::ListRecentActivityRequest,
    ) -> Result<Vec<db::ActivityItem>, String> {
        commands::list_recent_activity(request, &self.db, &self.license, &self.auth).await
    }

    /// Resident directory.
    pub async fn list_residents(
        &self,
        request: commands::ListResidentsRequest,
    ) -> Result<Vec<db::ResidentSummary>, String> {
        commands::list_residents(request, &self.db, &self.license, &self.auth).await
    }

    /// One resident, their stays, and their payments.
    pub async fn resident_detail(
        &self,
        request: commands::GetResidentDetailRequest,
    ) -> Result<db::ResidentDetail, String> {
        commands::get_resident_detail(request, &self.db, &self.license, &self.auth).await
    }

    /// Update a resident's contact details.
    pub async fn update_resident(
        &self,
        request: commands::UpdateResidentRequest,
    ) -> Result<(), String> {
        commands::update_resident(request, &self.db, &self.license, &self.auth).await
    }

    /// Preferences for a hostel, or the defaults.
    pub async fn app_preferences(
        &self,
        request: commands::GetAppPreferencesRequest,
    ) -> Result<db::AppPreferences, String> {
        commands::get_app_preferences(request, &self.db, &self.license, &self.auth).await
    }

    /// Save preferences. Admin only.
    pub async fn update_app_preferences(
        &self,
        request: commands::UpsertAppPreferencesRequest,
    ) -> Result<db::AppPreferences, String> {
        commands::update_app_preferences(request, &self.db, &self.license, &self.auth).await
    }

    /// Write a consistent database backup and return its path.
    pub async fn backup_database(&self) -> Result<String, String> {
        commands::backup_database(&self.paths, &self.db, &self.license, &self.auth).await
    }

    /// Backup files in Documents/HostelDesk/backups, newest first.
    pub async fn list_backups(&self) -> Result<Vec<String>, String> {
        commands::list_backups(&self.paths, &self.license, &self.auth).await
    }

    /// Replace the live database with a backup and restart the process.
    ///
    /// Refused while the desk syncs with the cloud: the cloud already holds
    /// the data, and an old copy would reissue receipt numbers already printed.
    pub async fn restore_backup(
        &self,
        request: commands::RestoreBackupRequest,
    ) -> Result<(), String> {
        if self.db.is_enrolled().await.map_err(|e| e.to_string())? {
            return Err(
                "This computer syncs with the cloud, so restoring a backup is turned off. \
                 Your data is already kept in the cloud."
                    .to_string(),
            );
        }
        commands::restore_backup(request, &self.paths, &self.db, &self.license, &self.auth).await
    }
}
