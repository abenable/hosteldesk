//! Application service behind the Tauri IPC commands.
//!
//! Each public method is one command (`crate::ipc`). The frontend calls
//! [`Desk`] and never touches the database, money arithmetic, or permissions.

use chrono::NaiveDate;
use db::DbPool;
use serde::Serialize;

use crate::commands;
use crate::paths::AppPaths;
use crate::state::{AccountLicense, AuthState, LicenseState};

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
    /// Held for a whole sync pass, so the background loop, "Sync now", and
    /// connect never push or pull the same rows at once.
    pub(crate) sync_gate: tokio::sync::Mutex<()>,
}

/// The screen the window opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    /// No usable account license: sign in to an account, or wait for it.
    Lock,
    /// Licensed, but the hostel or its first administrator is missing.
    Onboarding,
    /// Set up; no staff member is signed in.
    SignIn,
    /// A staff member is signed in.
    Ready,
}

/// Everything the window needs to pick its first screen ([`Desk::boot_state`]).
#[derive(Debug, Clone, Serialize)]
pub struct BootState {
    /// Which screen to show.
    pub gate: Gate,
    /// Signed-in staff member; set only when `gate` is `ready`.
    pub user: Option<db::User>,
    /// This desk already holds a hostel (for example one pulled from the
    /// cloud), so onboarding only creates the first administrator.
    pub hostel_exists: bool,
    /// This computer is signed in to a HostelDesk account and syncs.
    pub enrolled: bool,
    /// The account license in force today; `None` while locked.
    pub license: Option<AccountLicense>,
    /// Why the desk is locked; set only when `gate` is `lock` and the desk is
    /// enrolled (an unenrolled desk just needs to sign in).
    pub lock_reason: Option<String>,
    /// `CARGO_PKG_VERSION` of the running binary.
    pub app_version: &'static str,
}

/// Where hostels buy and renew HostelDesk plans (docs/LICENSING.md).
pub const SUPPORT_EMAIL: &str = "ableabenaitwe@gmail.com";

/// `mailto:` link to [`SUPPORT_EMAIL`] with `subject` percent-encoded, so a
/// hostel name holding `&`, `?`, or a line break cannot add other fields.
pub fn renewal_mailto(subject: &str) -> String {
    let subject: String = subject
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect();
    format!("mailto:{SUPPORT_EMAIL}?subject={subject}")
}

/// Why a signed-in computer is locked, from the server's last answer.
pub fn lock_reason(license: Option<AccountLicense>, today: NaiveDate) -> String {
    match license {
        Some(license) if !license.approved => {
            "Your HostelDesk account is waiting for approval. This computer \
             unlocks by itself as soon as it is approved."
        }
        Some(license) if license.expires_on < today => {
            "Your HostelDesk plan has ended. Contact HostelDesk to renew it; this \
             computer unlocks by itself once it is renewed."
        }
        _ => {
            "This computer needs to check its license with HostelDesk. Connect it \
             to the internet; it checks again by itself."
        }
    }
    .to_string()
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
            sync_gate: tokio::sync::Mutex::new(()),
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
            sync_gate: tokio::sync::Mutex::new(()),
        }
    }

    /// Which screen to open on, and what it needs: the account license, then
    /// onboarding, then the staff session decide, in that order.
    pub async fn boot_state(&self) -> Result<BootState, String> {
        let enrolled = self.cloud_enrolled();
        let license = self.license_status();
        let hostel_exists = self
            .db
            .primary_hostel_id()
            .await
            .map_err(|e| e.to_string())?
            .is_some();
        let (gate, user) = if license.is_none() {
            (Gate::Lock, None)
        } else if !self.onboarding_status().await? {
            (Gate::Onboarding, None)
        } else {
            match self.session().await? {
                Some(user) => (Gate::Ready, Some(user)),
                None => (Gate::SignIn, None),
            }
        };
        let lock_reason = (gate == Gate::Lock && enrolled)
            .then(|| lock_reason(self.cached_license(), chrono::Utc::now().date_naive()));
        Ok(BootState {
            gate,
            user,
            hostel_exists,
            enrolled,
            license,
            lock_reason,
            app_version: self.app_version(),
        })
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

    /// Room classes for one hostel.
    pub async fn room_classes(&self, hostel_id: i64) -> Result<Vec<db::RoomClass>, String> {
        commands::get_room_classes(hostel_id, &self.db, &self.license, &self.auth).await
    }

    /// Create a room class, or edit one. Returns its id.
    pub async fn save_room_class(
        &self,
        request: commands::SaveRoomClassRequest,
    ) -> Result<i64, String> {
        commands::save_room_class(request, &self.db, &self.license, &self.auth).await
    }

    /// Add rooms of one class, all or none. Returns their ids.
    pub async fn add_rooms(&self, request: commands::AddRoomsRequest) -> Result<Vec<i64>, String> {
        commands::add_rooms(request, &self.db, &self.license, &self.auth).await
    }

    /// Open the computer's email app on a renewal request to HostelDesk,
    /// naming the hostel and its plan. The address is fixed here; the UI
    /// never hands Rust a URL to open.
    pub async fn email_renewal(&self) -> Result<(), String> {
        let hostel = match self
            .db
            .primary_hostel_id()
            .await
            .map_err(|e| e.to_string())?
        {
            Some(id) => self
                .db
                .get_hostel_by_id(id)
                .await
                .map_err(|e| e.to_string())?
                .map(|hostel| hostel.name),
            None => None,
        };
        let plan = self.cached_license().map(|license| license.plan);
        let subject = match (hostel, plan) {
            (Some(hostel), Some(plan)) => format!("Renew HostelDesk for {hostel} ({plan} plan)"),
            (Some(hostel), None) => format!("Renew HostelDesk for {hostel}"),
            _ => "Renew HostelDesk".to_string(),
        };
        open::that(renewal_mailto(&subject))
            .map_err(|e| format!("Could not open your email app: {e}"))
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

    /// Record a payment on an active stay and print its receipt.
    ///
    /// The amount is checked with `protocol::validate_money`; the receipt's
    /// balance is the stay's balance minus this payment, in `Decimal`, never
    /// below zero. A PDF failure does not undo the payment: it comes back as
    /// [`commands::TakePaymentResult::pdf_error`].
    pub async fn take_payment(
        &self,
        request: commands::TakePaymentRequest,
    ) -> Result<commands::TakePaymentResult, String> {
        let hostel_id = request.hostel_id;
        // Also the license and session guard, so those errors come first.
        let stays = self
            .active_allocations(commands::ListActiveAllocationsRequest { hostel_id })
            .await?;
        let amount = request.amount.trim().to_string();
        let paid = protocol::validate_money("Amount", &amount)
            .map_err(|_| "Amount must be above zero, like 150000.".to_string())?;
        let stay = stays
            .into_iter()
            .find(|stay| stay.allocation_id == request.allocation_id)
            .ok_or("Choose an active stay.")?;
        let profile = self
            .hostel_profile(hostel_id)
            .await?
            .ok_or("The hostel profile is missing.")?;
        let prefs = self
            .app_preferences(commands::GetAppPreferencesRequest { hostel_id })
            .await?;
        let notes = request
            .notes
            .map(|notes| notes.trim().to_string())
            .filter(|notes| !notes.is_empty());
        let payer = self
            .resident_detail(commands::GetResidentDetailRequest {
                hostel_id,
                resident_id: stay.resident_id,
            })
            .await?
            .resident;
        let room_type = self.room_type(hostel_id, &stay.room_number).await?;
        let staff = self.session().await?;

        let (_, receipt_number) = self
            .record_payment_and_generate_receipt(commands::RecordPaymentRequest {
                hostel_id,
                allocation_id: stay.allocation_id,
                resident_id: stay.resident_id,
                amount: amount.clone(),
                payment_method: request.payment_method.clone(),
                period_start: None,
                period_end: None,
                notes: notes.clone(),
            })
            .await?;
        let printed = self
            .generate_and_open_receipt(commands::GenerateReceiptRequest {
                hostel_id,
                hostel_name: profile.name,
                hostel_address: profile.address,
                hostel_phone: profile.phone,
                receipt_number: receipt_number.clone(),
                resident_name: stay.resident_name,
                resident_phone: payer.phone,
                resident_email: payer.email,
                room_number: stay.room_number,
                room_type,
                term: Some(stay.term),
                academic_year: Some(stay.academic_year),
                amount,
                date: chrono::Local::now().date_naive(),
                balance: commands::balance_after(&stay.balance, paid),
                notes,
                currency_code: profile.currency,
                footer_text: prefs.receipt_footer,
                logo_path: profile.branding_logo_path,
                received_by: staff.as_ref().map(|user| user.full_name.clone()),
                received_by_contact: staff.map(|user| user.email),
            })
            .await;
        Ok(commands::TakePaymentResult {
            receipt_number,
            pdf_error: printed.err(),
        })
    }

    /// Open a payment's receipt PDF again, or print it from the ledger when
    /// this desk has none (for example a payment taken on another desk). A
    /// reprint shows the balance as it stood right after that payment.
    pub async fn open_receipt(&self, request: commands::OpenReceiptRequest) -> Result<(), String> {
        let hostel_id = request.hostel_id;
        // Also the license and session guard, so those errors come first.
        let profile = self
            .hostel_profile(hostel_id)
            .await?
            .ok_or("The hostel profile is missing.")?;
        let prefs = self
            .app_preferences(commands::GetAppPreferencesRequest { hostel_id })
            .await?;
        let resident_id: Option<i64> =
            sqlx::query_scalar("SELECT resident_id FROM payments WHERE id = ?1 AND hostel_id = ?2")
                .bind(request.payment_id)
                .bind(hostel_id)
                .fetch_optional(&self.db.inner)
                .await
                .map_err(|e| e.to_string())?;
        let missing = || "That payment is no longer on record.".to_string();
        let resident_id = resident_id.ok_or_else(missing)?;
        let row = self
            .list_payments(db::ListPaymentsFilter {
                hostel_id,
                from: None,
                to: None,
                resident_id: Some(resident_id),
                method: None,
            })
            .await?
            .into_iter()
            .find(|row| row.id == request.payment_id)
            .ok_or_else(missing)?;

        if let Some(path) = row
            .pdf_path
            .as_deref()
            .filter(|path| std::path::Path::new(path).exists())
        {
            return open::that(path).map_err(|e| format!("Could not open the receipt: {e}"));
        }
        let detail = self
            .resident_detail(commands::GetResidentDetailRequest {
                hostel_id,
                resident_id,
            })
            .await?;
        let balance = commands::balance_at_payment(&detail, row.allocation_id, row.id)?;
        let stay = detail
            .allocations
            .iter()
            .find(|stay| stay.id == row.allocation_id);
        let room_type = self.room_type(hostel_id, &row.room_number).await?;
        self.generate_and_open_receipt(commands::GenerateReceiptRequest {
            hostel_id,
            hostel_name: profile.name,
            hostel_address: profile.address,
            hostel_phone: profile.phone,
            receipt_number: row.receipt_number.unwrap_or_default(),
            resident_name: row.resident_name,
            resident_phone: detail.resident.phone.clone(),
            resident_email: detail.resident.email.clone(),
            room_number: row.room_number,
            room_type,
            term: stay.map(|stay| stay.term.clone()),
            academic_year: stay.map(|stay| stay.academic_year.clone()),
            amount: row.amount,
            date: row.payment_date.with_timezone(&chrono::Local).date_naive(),
            balance,
            notes: row.notes,
            currency_code: profile.currency,
            footer_text: prefs.receipt_footer,
            logo_path: profile.branding_logo_path,
            // Payments do not record who took them: left blank to write in.
            received_by: None,
            received_by_contact: None,
        })
        .await
        .map(|_| ())
    }

    /// The type (`single`, `double`, `dorm`) of a room's class, for its receipts.
    async fn room_type(&self, hostel_id: i64, room_number: &str) -> Result<Option<String>, String> {
        Ok(self
            .db
            .get_rooms(hostel_id)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|room| room.room_number == room_number)
            .map(|room| room.room_type))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::setup_license_state;
    use crate::state::AccountLicense;
    use chrono::NaiveDate;

    async fn desk_at(root: &std::path::Path, license: LicenseState) -> Desk {
        let paths = AppPaths::for_test(root);
        std::fs::create_dir_all(&paths.data_dir).expect("data dir");
        let db = DbPool::open(&paths.database_url()).await.expect("db");
        Desk::from_parts(db, AuthState::new(), license, paths)
    }

    fn onboarding() -> commands::CompleteOnboardingRequest {
        commands::CompleteOnboardingRequest {
            hostel_name: "Sunrise".into(),
            address: None,
            phone: None,
            currency: Some("UGX".into()),
            branding_logo_path: None,
            admin_name: "Ada".into(),
            email: "ada@sunrise.co".into(),
            password: "secret-pass".into(),
        }
    }

    /// A licensed, signed-in desk with one resident checked in at 500000.
    /// Documents is a plain file, so a receipt PDF can never be written (and
    /// no PDF viewer opens while tests run).
    async fn desk_with_stay(root: &std::path::Path) -> (Desk, i64, i64) {
        let desk = desk_at(root, setup_license_state()).await;
        std::fs::write(root.join("documents"), "").expect("documents file");
        let hostel_id = desk
            .complete_onboarding(onboarding())
            .await
            .expect("onboard")
            .hostel_id;
        let class_id = desk
            .save_room_class(commands::SaveRoomClassRequest {
                hostel_id,
                class_id: None,
                room_type: "single".into(),
                semester_rate: "500000".into(),
                recess_rate: "200000".into(),
            })
            .await
            .expect("class");
        let room_id = desk
            .add_rooms(commands::AddRoomsRequest {
                hostel_id,
                class_id,
                room_numbers: vec!["1".into()],
            })
            .await
            .expect("room")[0];
        let allocation_id = desk
            .check_in(commands::CheckInRequest {
                hostel_id,
                room_id,
                resident_id: None,
                full_name: Some("Grace".into()),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
                photo_path: None,
                rate_agreed: "500000".into(),
                term: "semester_1".into(),
                academic_year: "2026/2027".into(),
            })
            .await
            .expect("check in");
        (desk, hostel_id, allocation_id)
    }

    fn pay(hostel_id: i64, allocation_id: i64, amount: &str) -> commands::TakePaymentRequest {
        commands::TakePaymentRequest {
            hostel_id,
            allocation_id,
            amount: amount.into(),
            payment_method: "cash".into(),
            notes: Some("  ".into()),
        }
    }

    #[test]
    fn lock_reason_explains_the_servers_last_answer() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 27).unwrap();
        let license = AccountLicense {
            account_name: "Sunrise".into(),
            plan: "trial".into(),
            expires_on: today + chrono::Duration::days(30),
            checked_on: today,
            approved: false,
        };
        assert!(lock_reason(Some(license.clone()), today).contains("waiting for approval"));
        let ended = AccountLicense {
            approved: true,
            expires_on: today - chrono::Duration::days(1),
            ..license.clone()
        };
        assert!(lock_reason(Some(ended), today).contains("plan has ended"));
        let stale = AccountLicense {
            approved: true,
            checked_on: today - chrono::Duration::days(20),
            ..license
        };
        assert!(lock_reason(Some(stale), today).contains("check its license"));
        assert!(lock_reason(None, today).contains("check its license"));
    }

    #[tokio::test]
    async fn boot_state_picks_the_first_screen() {
        let root = tempfile::tempdir().expect("temp");
        let desk = desk_at(root.path(), LicenseState::default()).await;

        let boot = desk.boot_state().await.expect("boot");
        assert_eq!(boot.gate, Gate::Lock);
        assert!(boot.user.is_none() && boot.license.is_none());
        assert!(!boot.enrolled && !boot.hostel_exists);
        assert_eq!(boot.lock_reason, None, "not signed in to an account yet");
        assert_eq!(boot.app_version, env!("CARGO_PKG_VERSION"));

        desk.license
            .set(setup_license_state().get().expect("license"));
        let boot = desk.boot_state().await.expect("boot");
        assert_eq!(boot.gate, Gate::Onboarding);
        assert!(boot.license.is_some() && !boot.hostel_exists);

        let admin = desk
            .complete_onboarding(onboarding())
            .await
            .expect("onboard");
        let boot = desk.boot_state().await.expect("boot");
        assert_eq!(boot.gate, Gate::Ready);
        assert_eq!(boot.user.map(|user| user.id), Some(admin.id));
        assert!(boot.hostel_exists);

        desk.logout();
        let boot = desk.boot_state().await.expect("boot");
        assert_eq!(boot.gate, Gate::SignIn);
        assert!(boot.user.is_none());
        let json = serde_json::to_value(&boot).expect("json");
        assert_eq!(
            json["gate"], "sign_in",
            "the IPC contract spells gates in snake_case"
        );
    }

    #[tokio::test]
    async fn take_payment_records_the_payment_even_when_the_receipt_pdf_fails() {
        let root = tempfile::tempdir().expect("temp");
        let (desk, hostel_id, allocation_id) = desk_with_stay(root.path()).await;

        for bad in ["", "-5", "0", "1e3", "1.2.3"] {
            let error = desk
                .take_payment(pay(hostel_id, allocation_id, bad))
                .await
                .expect_err(bad);
            assert!(error.starts_with("Amount"), "{bad}: {error}");
        }
        let error = desk
            .take_payment(pay(hostel_id, allocation_id + 1, "100"))
            .await
            .expect_err("no such stay");
        assert!(error.contains("active stay"), "{error}");

        let taken = desk
            .take_payment(pay(hostel_id, allocation_id, " 200000 "))
            .await
            .expect("take payment");
        assert!(!taken.receipt_number.is_empty());
        assert!(taken.pdf_error.is_some(), "Documents is a file here");

        let filter = db::ListPaymentsFilter {
            hostel_id,
            from: None,
            to: None,
            resident_id: None,
            method: None,
        };
        let rows = desk.list_payments(filter).await.expect("ledger");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount, "200000");
        assert_eq!(rows[0].notes, None, "blank notes are not stored");
        assert_eq!(rows[0].receipt_number.as_ref(), Some(&taken.receipt_number));

        desk.logout();
        assert_eq!(
            desk.take_payment(pay(hostel_id, allocation_id, "100"))
                .await
                .expect_err("signed out"),
            "unauthorized"
        );
    }

    #[test]
    fn renewal_mailto_encodes_the_subject() {
        assert_eq!(
            renewal_mailto("Renew Kate & Co?\nbcc=x (1-year)"),
            "mailto:ableabenaitwe@gmail.com?subject=Renew%20Kate%20%26%20Co%3F%0Abcc%3Dx%20%281-year%29"
        );
    }

    #[tokio::test]
    async fn open_receipt_reprints_a_payment_from_the_ledger() {
        let root = tempfile::tempdir().expect("temp");
        let (desk, hostel_id, allocation_id) = desk_with_stay(root.path()).await;
        desk.take_payment(pay(hostel_id, allocation_id, "100"))
            .await
            .expect("take payment");
        let payment_id = desk
            .list_payments(db::ListPaymentsFilter {
                hostel_id,
                from: None,
                to: None,
                resident_id: None,
                method: None,
            })
            .await
            .expect("ledger")[0]
            .id;

        let missing = desk
            .open_receipt(commands::OpenReceiptRequest {
                hostel_id,
                payment_id: payment_id + 1,
            })
            .await
            .expect_err("no such payment");
        assert!(missing.contains("no longer on record"), "{missing}");

        // No PDF on record, so it prints one; Documents is a file, so that fails.
        let printing = desk
            .open_receipt(commands::OpenReceiptRequest {
                hostel_id,
                payment_id,
            })
            .await
            .expect_err("PDF cannot be written here");
        assert!(printing.contains("os error"), "{printing}");

        desk.logout();
        assert_eq!(
            desk.open_receipt(commands::OpenReceiptRequest {
                hostel_id,
                payment_id,
            })
            .await
            .expect_err("signed out"),
            "unauthorized"
        );
    }
}
