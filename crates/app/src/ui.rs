//! Native screens.
//!
//! The UI thread only paints. Every [`Desk`] call runs on the Tokio runtime
//! through [`HostelApp::run`], and its result comes back as a closure applied
//! at the start of the next frame, so a slow disk, a PDF, or an unreachable
//! sync server never freezes the window.
//!
//! What the signed-in user can see lives in one [`Workspace`], replaced
//! wholesale on sign-out. Loaded rows and edit forms are separate fields, so a
//! background refresh never overwrites something the user is typing.

use std::collections::HashSet;
use std::future::Future;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Datelike, NaiveDate};
use db::User;
use eframe::egui::{self, RichText};
use rust_decimal::Decimal;

use crate::commands::{
    AddRoomRequest, CheckInRequest, CheckOutRequest, CompleteOnboardingRequest, CreateUserRequest,
    DeleteUserRequest, GenerateReceiptRequest, GetAppPreferencesRequest, GetDashboardStatsRequest,
    GetResidentDetailRequest, ListActiveAllocationsRequest, ListRecentActivityRequest,
    ListResidentsRequest, ListUsersRequest, LoginRequest, RecordPaymentRequest,
    RestoreBackupRequest, UpdateHostelRequest, UpdateResidentRequest, UpdateUserRequest,
    UploadHostelLogoRequest, UpsertAppPreferencesRequest,
};
use crate::sync::CloudStatus;
use crate::{runtime, Desk};

const SYNC_EVERY: Duration = Duration::from_secs(30);
const TERMS: [(&str, &str); 3] = [
    ("semester_1", "Semester 1"),
    ("semester_2", "Semester 2"),
    ("recess", "Recess"),
];
const METHODS: [(&str, &str); 4] = [
    ("cash", "Cash"),
    ("mobile_money", "Mobile money"),
    ("bank_transfer", "Bank transfer"),
    ("card", "Card"),
];

type Update = Box<dyn FnOnce(&mut HostelApp) + Send>;

enum Gate {
    Boot,
    Failed(String),
    Lock,
    Onboarding,
    SignIn,
    Ready,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Nav {
    #[default]
    Dashboard,
    Rooms,
    Residents,
    Payments,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SettingsTab {
    #[default]
    Account,
    Business,
    Team,
    Preferences,
    Backup,
    Cloud,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ResidentTab {
    #[default]
    Current,
    Past,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PaymentTab {
    #[default]
    Outstanding,
    History,
}

/// An action that waits for the user to confirm it.
enum Confirm {
    Restore(String),
    DeleteUser(User),
    CheckOut {
        allocation_id: i64,
        name: String,
        balance: String,
    },
}

#[derive(Default)]
struct Status {
    text: String,
    error: bool,
}

struct RoomForm {
    number: String,
    kind: String,
    capacity: String,
    floor: String,
    semester: String,
    recess: String,
}

impl Default for RoomForm {
    fn default() -> Self {
        Self {
            number: String::new(),
            kind: "single".into(),
            capacity: "1".into(),
            floor: String::new(),
            semester: String::new(),
            recess: String::new(),
        }
    }
}

struct CheckInForm {
    returning: Option<i64>,
    name: String,
    phone: String,
    email: String,
    id_number: String,
    kin: String,
    kin_phone: String,
    room: Option<i64>,
    rate: String,
    term: String,
    year: String,
}

impl Default for CheckInForm {
    fn default() -> Self {
        Self {
            returning: None,
            name: String::new(),
            phone: String::new(),
            email: String::new(),
            id_number: String::new(),
            kin: String::new(),
            kin_phone: String::new(),
            room: None,
            rate: String::new(),
            term: "semester_1".into(),
            year: academic_year(chrono::Local::now().date_naive()),
        }
    }
}

struct PayForm {
    allocation: Option<i64>,
    amount: String,
    method: String,
    notes: String,
}

impl Default for PayForm {
    fn default() -> Self {
        Self {
            allocation: None,
            amount: String::new(),
            method: "cash".into(),
            notes: String::new(),
        }
    }
}

#[derive(Default)]
struct BusinessForm {
    name: String,
    address: String,
    phone: String,
    currency: String,
    logo: Option<String>,
    dirty: bool,
}

#[derive(Default)]
struct PrefsForm {
    date_format: String,
    footer: String,
    dirty: bool,
}

#[derive(Default)]
struct ResidentForm {
    name: String,
    phone: String,
    email: String,
    id_number: String,
    kin: String,
    kin_phone: String,
}

struct TeamForm {
    name: String,
    email: String,
    password: String,
    role: String,
}

impl Default for TeamForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            password: String::new(),
            role: "manager".into(),
        }
    }
}

#[derive(Default)]
struct AccountForm {
    name: String,
    email: String,
    current: String,
    new: String,
    confirm: String,
}

/// Everything that belongs to the signed-in user.
#[derive(Default)]
struct Workspace {
    user: Option<User>,
    nav: Nav,
    settings: SettingsTab,
    resident_tab: ResidentTab,
    payment_tab: PaymentTab,
    loaded: bool,
    refresh_again: bool,
    profile: Option<db::HostelProfile>,
    prefs: Option<db::AppPreferences>,
    stats: Option<db::DashboardStats>,
    activity: Vec<db::ActivityItem>,
    rooms: Vec<db::Room>,
    residents: Vec<db::ResidentSummary>,
    stays: Vec<db::ActiveAllocation>,
    payments: Vec<db::PaymentRow>,
    users: Vec<User>,
    backups: Vec<String>,
    cloud: Option<CloudStatus>,
    room_filter: Option<&'static str>,
    room_form: RoomForm,
    check_in: CheckInForm,
    pay: PayForm,
    business: BusinessForm,
    prefs_form: PrefsForm,
    resident_query: String,
    detail: Option<db::ResidentDetail>,
    resident_form: ResidentForm,
    team: TeamForm,
    account: AccountForm,
}

impl Workspace {
    fn hostel_id(&self) -> Option<i64> {
        self.user.as_ref().map(|user| user.hostel_id)
    }

    fn is_admin(&self) -> bool {
        self.user.as_ref().is_some_and(|user| user.role == "admin")
    }

    fn stay(&self, allocation_id: i64) -> Option<&db::ActiveAllocation> {
        self.stays
            .iter()
            .find(|stay| stay.allocation_id == allocation_id)
    }

    fn currency(&self) -> &str {
        self.profile
            .as_ref()
            .map_or("UGX", |profile| profile.currency.as_str())
    }
}

#[derive(Default)]
struct CloudForm {
    server: String,
    org: String,
    email: String,
    password: String,
}

#[derive(Default)]
struct OnboardForm {
    name: String,
    address: String,
    phone: String,
    admin: String,
    email: String,
    password: String,
    /// The hostel already came down from the cloud; only the admin is missing.
    hostel_exists: bool,
}

/// Everything loaded by one [`HostelApp::refresh`].
struct Loaded {
    profile: Result<Option<db::HostelProfile>, String>,
    prefs: Result<db::AppPreferences, String>,
    stats: Result<db::DashboardStats, String>,
    activity: Result<Vec<db::ActivityItem>, String>,
    rooms: Result<Vec<db::Room>, String>,
    residents: Result<Vec<db::ResidentSummary>, String>,
    stays: Result<Vec<db::ActiveAllocation>, String>,
    payments: Result<Vec<db::PaymentRow>, String>,
    users: Option<Result<Vec<User>, String>>,
    backups: Option<Result<Vec<String>, String>>,
    cloud: Result<CloudStatus, String>,
}

/// Unwraps one loaded result, collecting the error for the status bar.
struct Taker<'a>(&'a mut Vec<String>);

impl Taker<'_> {
    fn of<T>(&mut self, result: Result<T, String>, what: &str) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.0
                    .push(if error == "unauthorized" || error == "unlicensed" {
                        error
                    } else {
                        format!("Could not load {what}: {error}")
                    });
                None
            }
        }
    }
}

/// Top-level egui application.
pub struct HostelApp {
    desk: Arc<Desk>,
    ctx: egui::Context,
    tx: Sender<Update>,
    rx: Receiver<Update>,
    busy: HashSet<&'static str>,
    /// Bumped on sign-in and sign-out; results from an older generation are dropped.
    generation: u64,
    gate: Gate,
    status: Status,
    confirm: Option<Confirm>,
    w: Workspace,
    cloud_form: CloudForm,
    onboard: OnboardForm,
    sign_in_email: String,
    sign_in_password: String,
    last_sync: Option<Instant>,
}

impl HostelApp {
    /// Start on the gate (lock, onboarding, sign-in) that matches local state.
    pub fn new(desk: Desk, ctx: egui::Context) -> Self {
        let (tx, rx) = channel();
        let mut app = Self {
            desk: Arc::new(desk),
            ctx,
            tx,
            rx,
            busy: HashSet::new(),
            generation: 0,
            gate: Gate::Boot,
            status: Status::default(),
            confirm: None,
            w: Workspace::default(),
            cloud_form: CloudForm::default(),
            onboard: OnboardForm::default(),
            sign_in_email: String::new(),
            sign_in_password: String::new(),
            last_sync: None,
        };
        app.boot();
        app
    }

    /// Run `work` on the runtime, then `then` on the UI thread. `key` names the
    /// action: while one with the same key is in flight a second is refused,
    /// so a double click never records a payment twice. Returns whether it started.
    fn run<T, F>(
        &mut self,
        key: &'static str,
        work: F,
        then: impl FnOnce(&mut HostelApp, T) + Send + 'static,
    ) -> bool
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        if !self.busy.insert(key) {
            return false;
        }
        let (tx, ctx, generation) = (self.tx.clone(), self.ctx.clone(), self.generation);
        runtime().spawn(async move {
            let value = work.await;
            let update: Update = Box::new(move |app: &mut HostelApp| {
                app.busy.remove(key);
                if app.generation == generation {
                    then(app, value);
                }
            });
            let _ = tx.send(update);
            ctx.request_repaint();
        });
        true
    }

    fn busy(&self, key: &str) -> bool {
        self.busy.contains(key)
    }

    fn ok(&mut self, text: impl Into<String>) {
        self.status = Status {
            text: text.into(),
            error: false,
        };
    }

    /// Show an error. A lapsed session or license sends the user back to the right gate.
    fn fail(&mut self, error: impl Into<String>) {
        let error = error.into();
        if error == "unauthorized" {
            self.sign_out();
            self.status = Status {
                text: "Your session timed out. Sign in again.".into(),
                error: true,
            };
            return;
        }
        if error == "unlicensed" {
            self.generation += 1;
            self.w = Workspace::default();
            self.gate = Gate::Lock;
        }
        self.status = Status {
            text: error,
            error: true,
        };
    }

    fn boot(&mut self) {
        self.gate = Gate::Boot;
        if self.desk.license_status().is_none() {
            self.gate = Gate::Lock;
            return;
        }
        let desk = self.desk.clone();
        self.run(
            "boot",
            async move {
                let complete = desk.onboarding_status().await?;
                let hostel_exists = desk
                    .db
                    .primary_hostel_id()
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some();
                let session = if complete {
                    desk.session().await?
                } else {
                    None
                };
                Ok::<_, String>((complete, hostel_exists, session))
            },
            |app, result| match result {
                Ok((true, _, Some(user))) => app.enter(user),
                Ok((true, _, None)) => app.gate = Gate::SignIn,
                Ok((false, hostel_exists, _)) => {
                    app.onboard.hostel_exists = hostel_exists;
                    app.gate = Gate::Onboarding;
                }
                Err(error) => app.gate = Gate::Failed(error),
            },
        );
    }

    fn enter(&mut self, user: User) {
        self.generation += 1;
        self.w = Workspace {
            account: AccountForm {
                name: user.full_name.clone(),
                email: user.email.clone(),
                ..AccountForm::default()
            },
            user: Some(user),
            ..Workspace::default()
        };
        self.confirm = None;
        self.gate = Gate::Ready;
        self.refresh();
    }

    fn sign_out(&mut self) {
        self.desk.logout();
        self.generation += 1;
        self.w = Workspace::default();
        self.confirm = None;
        self.gate = Gate::SignIn;
    }

    /// Reload everything the workspace shows, in one background job.
    fn refresh(&mut self) {
        let Some(hostel_id) = self.w.hostel_id() else {
            return;
        };
        let admin = self.w.is_admin();
        let desk = self.desk.clone();
        let started = self.run(
            "refresh",
            async move {
                let today = chrono::Local::now().date_naive();
                Loaded {
                    profile: desk.hostel_profile(hostel_id).await,
                    prefs: desk
                        .app_preferences(GetAppPreferencesRequest { hostel_id })
                        .await,
                    stats: desk
                        .dashboard_stats(GetDashboardStatsRequest { hostel_id, today })
                        .await,
                    activity: desk
                        .recent_activity(ListRecentActivityRequest {
                            hostel_id,
                            limit: 12,
                        })
                        .await,
                    rooms: desk.rooms(hostel_id).await,
                    residents: desk
                        .list_residents(ListResidentsRequest { hostel_id })
                        .await,
                    stays: desk
                        .active_allocations(ListActiveAllocationsRequest { hostel_id })
                        .await,
                    payments: desk
                        .list_payments(db::ListPaymentsFilter {
                            hostel_id,
                            from: None,
                            to: None,
                            resident_id: None,
                            method: None,
                        })
                        .await,
                    users: if admin {
                        Some(desk.list_users(ListUsersRequest { hostel_id }).await)
                    } else {
                        None
                    },
                    backups: if admin {
                        Some(desk.list_backups().await)
                    } else {
                        None
                    },
                    cloud: desk.cloud_status().await,
                }
            },
            Self::apply_loaded,
        );
        if !started {
            self.w.refresh_again = true;
        }
    }

    fn apply_loaded(&mut self, loaded: Loaded) {
        let mut errors = Vec::new();
        let mut take = Taker(&mut errors);
        let w = &mut self.w;
        if let Some(profile) = take.of(loaded.profile, "the hostel profile") {
            if let Some(profile) = &profile {
                if !w.business.dirty {
                    w.business = BusinessForm {
                        name: profile.name.clone(),
                        address: profile.address.clone().unwrap_or_default(),
                        phone: profile.phone.clone().unwrap_or_default(),
                        currency: profile.currency.clone(),
                        logo: profile.branding_logo_path.clone(),
                        dirty: false,
                    };
                }
            }
            w.profile = profile;
        }
        if let Some(prefs) = take.of(loaded.prefs, "preferences") {
            if !w.prefs_form.dirty {
                w.prefs_form = PrefsForm {
                    date_format: prefs.date_format.clone(),
                    footer: prefs.receipt_footer.clone().unwrap_or_default(),
                    dirty: false,
                };
            }
            w.prefs = Some(prefs);
        }
        if let Some(stats) = take.of(loaded.stats, "dashboard figures") {
            w.stats = Some(stats);
        }
        if let Some(activity) = take.of(loaded.activity, "recent activity") {
            w.activity = activity;
        }
        if let Some(rooms) = take.of(loaded.rooms, "rooms") {
            w.rooms = rooms;
        }
        if let Some(residents) = take.of(loaded.residents, "residents") {
            w.residents = residents;
        }
        if let Some(stays) = take.of(loaded.stays, "active stays") {
            w.stays = stays;
        }
        if let Some(payments) = take.of(loaded.payments, "payments") {
            w.payments = payments;
        }
        if let Some(users) = loaded.users.and_then(|users| take.of(users, "staff")) {
            w.users = users;
        }
        if let Some(backups) = loaded
            .backups
            .and_then(|backups| take.of(backups, "backups"))
        {
            w.backups = backups;
        }
        if let Some(cloud) = take.of(loaded.cloud, "cloud status") {
            w.cloud = Some(cloud);
        }
        w.loaded = true;
        if let Some(error) = errors.into_iter().next() {
            self.fail(error);
        }
        if std::mem::take(&mut self.w.refresh_again) {
            self.refresh();
        }
    }

    /// Run a write. On success, show `done`, run `after`, and reload.
    fn write<T: Send + 'static>(
        &mut self,
        key: &'static str,
        work: impl Future<Output = Result<T, String>> + Send + 'static,
        done: &'static str,
        after: impl FnOnce(&mut HostelApp, T) + Send + 'static,
    ) {
        self.run(key, work, move |app, result| match result {
            Ok(value) => {
                app.ok(done);
                after(app, value);
                app.refresh();
            }
            Err(error) => app.fail(error),
        });
    }

    fn maybe_sync(&mut self) {
        self.ctx.request_repaint_after(SYNC_EVERY);
        let due = self.last_sync.is_none_or(|at| at.elapsed() >= SYNC_EVERY);
        if !due || self.busy("sync") || !self.desk.cloud_enrolled() {
            return;
        }
        self.sync_now();
    }

    fn sync_now(&mut self) {
        self.last_sync = Some(Instant::now());
        let desk = self.desk.clone();
        self.run(
            "sync",
            async move { desk.sync_once().await },
            |app, result| {
                match result {
                    Ok(report) => {
                        if !report.summary.is_empty() {
                            app.ok(report.summary);
                            app.refresh();
                        }
                    }
                    Err(error) => app.fail(format!("Sync failed, will retry: {error}")),
                }
                // A renewed license unlocks a locked desk without a restart.
                if matches!(app.gate, Gate::Lock) && app.desk.license_status().is_some() {
                    app.boot();
                }
            },
        );
    }
}

impl eframe::App for HostelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(update) = self.rx.try_recv() {
            update(self);
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if !self.busy.is_empty() {
                    ui.spinner();
                }
                let text = RichText::new(&self.status.text).small();
                if self.status.error {
                    ui.colored_label(ui.visuals().error_fg_color, text);
                } else {
                    ui.label(text);
                }
            });
        });
        self.confirm_dialog(ui);
        match &self.gate {
            Gate::Boot => {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.centered_and_justified(|ui| ui.spinner());
                });
            }
            Gate::Failed(error) => {
                let error = error.clone();
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(48.0);
                        ui.heading("Could not start HostelDesk");
                        ui.colored_label(ui.visuals().error_fg_color, error);
                        if ui.button("Try again").clicked() {
                            self.boot();
                        }
                    });
                });
            }
            Gate::Lock => lock_screen(self, ui),
            Gate::Onboarding => onboarding(self, ui),
            Gate::SignIn => sign_in(self, ui),
            Gate::Ready => shell(self, ui),
        }
    }
}

impl HostelApp {
    fn confirm_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let (title, body, action) = match confirm {
            Confirm::Restore(path) => (
                "Restore this backup?".to_string(),
                format!(
                    "{path}\n\nEverything recorded since this backup was made will be \
                     replaced. This cannot be undone. HostelDesk restarts afterwards."
                ),
                "Restore",
            ),
            Confirm::DeleteUser(user) => (
                format!("Remove {}?", user.full_name),
                format!("{} will no longer be able to sign in.", user.email),
                "Remove",
            ),
            Confirm::CheckOut { name, balance, .. } => {
                let owes = Decimal::from_str(balance).is_ok_and(|b| b > Decimal::ZERO);
                let warning = if owes {
                    format!(
                        "{name} still owes {} {balance}. Check out anyway?",
                        self.w.currency()
                    )
                } else {
                    format!("{name} has paid in full.")
                };
                (format!("Check out {name}?"), warning, "Check out")
            }
        };
        let mut decision = None;
        egui::Modal::new(egui::Id::new("confirm")).show(ui.ctx(), |ui| {
            ui.heading(title);
            ui.label(body);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(action).clicked() {
                    decision = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(false);
                }
            });
        });
        match decision {
            Some(true) => {
                if let Some(confirm) = self.confirm.take() {
                    self.confirmed(confirm);
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    fn confirmed(&mut self, confirm: Confirm) {
        let desk = self.desk.clone();
        match confirm {
            Confirm::Restore(backup_path) => self.run(
                "restore",
                async move {
                    desk.restore_backup(RestoreBackupRequest { backup_path })
                        .await
                },
                |app, result| {
                    if let Err(error) = result {
                        app.fail(error);
                    }
                },
            ),
            Confirm::DeleteUser(user) => {
                self.write(
                    "delete-user",
                    async move {
                        desk.delete_user(DeleteUserRequest { user_id: user.id })
                            .await
                    },
                    "Staff member removed.",
                    |_, ()| {},
                );
                true
            }
            Confirm::CheckOut { allocation_id, .. } => {
                let Some(hostel_id) = self.w.hostel_id() else {
                    return;
                };
                self.write(
                    "check-out",
                    async move {
                        desk.check_out(CheckOutRequest {
                            hostel_id,
                            allocation_id,
                        })
                        .await
                    },
                    "Checked out.",
                    |app, ()| {
                        app.w.detail = None;
                    },
                );
                true
            }
        };
    }
}

/// HostelDesk is licensed per account: this computer unlocks by signing in.
/// A computer that already signed in only needs to reach the server again
/// to confirm the license, which it retries on its own.
fn lock_screen(app: &mut HostelApp, ui: &mut egui::Ui) {
    let enrolled = app.desk.cloud_enrolled();
    if enrolled {
        app.maybe_sync();
    }
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(48.0);
                ui.heading("HostelDesk");
            });
            if enrolled {
                ui.label(
                    "This computer's license needs renewing. HostelDesk renews it \
                     automatically when it can reach the internet; if your account's \
                     plan has ended, contact HostelDesk to renew it.",
                );
                if ui
                    .add_enabled(!app.busy("sync"), egui::Button::new("Check again"))
                    .clicked()
                {
                    app.sync_now();
                }
                ui.separator();
                ui.label("Or sign in with a different account:");
            } else {
                ui.label(
                    "Sign in to your HostelDesk account to use this computer. New to \
                     HostelDesk? Create an account to start your free trial.",
                );
            }
            cloud_connect_form(app, ui);
        });
    });
}

fn cloud_connect_form(app: &mut HostelApp, ui: &mut egui::Ui) {
    let f = &mut app.cloud_form;
    labeled(ui, "Email", &mut f.email, false);
    labeled(ui, "Password", &mut f.password, true);
    labeled(
        ui,
        "Hostel or business name (new accounts)",
        &mut f.org,
        false,
    );
    egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
        labeled(
            ui,
            "Server (blank for HostelDesk Cloud)",
            &mut f.server,
            false,
        );
    });
    let busy = app.busy("connect");
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new("Sign in"))
            .clicked()
        {
            connect_cloud(app, false);
        }
        if ui
            .add_enabled(!busy, egui::Button::new("Create account"))
            .clicked()
        {
            connect_cloud(app, true);
        }
    });
}

fn connect_cloud(app: &mut HostelApp, create_account: bool) {
    let f = &app.cloud_form;
    let (server, email, password, org) = (
        f.server.clone(),
        f.email.clone(),
        f.password.clone(),
        f.org.clone(),
    );
    let desk = app.desk.clone();
    app.run(
        "connect",
        async move {
            desk.connect_cloud(&server, &email, &password, create_account, &org)
                .await?;
            Ok::<_, String>(desk.sync_once().await)
        },
        |app, result| match result {
            Ok(sync) => {
                app.cloud_form.password.clear();
                app.last_sync = Some(Instant::now());
                match sync {
                    Ok(_) if app.desk.license_status().is_none() => app.fail(
                        "Signed in, but this account's plan has ended. Contact \
                         HostelDesk to renew it.",
                    ),
                    Ok(_) => app.ok("Signed in."),
                    Err(error) => app.fail(format!("Connected. Sync will retry: {error}")),
                }
                if matches!(app.gate, Gate::Lock | Gate::Onboarding | Gate::SignIn) {
                    app.boot();
                } else {
                    app.refresh();
                }
            }
            Err(error) => app.fail(error),
        },
    );
}

fn onboarding(app: &mut HostelApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let f = &mut app.onboard;
            if f.hostel_exists {
                ui.heading("Create the first administrator");
                ui.label(
                    "This computer received its hostel from the cloud. Create the account \
                     you will sign in with here.",
                );
            } else {
                ui.heading("Set up your hostel");
                labeled(ui, "Hostel name", &mut f.name, false);
                labeled(ui, "Address", &mut f.address, false);
                labeled(ui, "Phone", &mut f.phone, false);
                ui.separator();
                ui.label("First administrator");
            }
            labeled(ui, "Full name", &mut f.admin, false);
            labeled(ui, "Email", &mut f.email, false);
            labeled(ui, "Password (8+ characters)", &mut f.password, true);
            let busy = app.busy("onboard");
            if ui
                .add_enabled(!busy, egui::Button::new("Continue"))
                .clicked()
            {
                let f = &app.onboard;
                if !f.email.contains('@') {
                    app.fail("Enter a valid email address.");
                    return;
                }
                let request = CompleteOnboardingRequest {
                    hostel_name: f.name.trim().to_string(),
                    address: blank(&f.address),
                    phone: blank(&f.phone),
                    currency: Some("UGX".to_string()),
                    branding_logo_path: None,
                    admin_name: f.admin.trim().to_string(),
                    email: f.email.trim().to_string(),
                    password: f.password.clone(),
                };
                let desk = app.desk.clone();
                app.run(
                    "onboard",
                    async move { desk.complete_onboarding(request).await },
                    |app, result| match result {
                        Ok(user) => {
                            app.onboard = OnboardForm::default();
                            app.ok("Your hostel is ready.");
                            app.enter(user);
                        }
                        Err(error) => app.fail(error),
                    },
                );
            }
        });
    });
}

fn sign_in(app: &mut HostelApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("Sign in");
        });
        labeled(ui, "Email", &mut app.sign_in_email, false);
        let response = labeled(ui, "Password", &mut app.sign_in_password, true);
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let busy = app.busy("sign-in");
        if (ui
            .add_enabled(!busy, egui::Button::new("Sign in"))
            .clicked()
            || enter)
            && !busy
        {
            let request = LoginRequest {
                email: app.sign_in_email.trim().to_string(),
                password: app.sign_in_password.clone(),
            };
            let desk = app.desk.clone();
            app.run(
                "sign-in",
                async move { desk.login(request).await },
                |app, result| match result {
                    Ok(user) => {
                        app.sign_in_password.clear();
                        app.ok(format!("Welcome, {}.", user.full_name));
                        app.enter(user);
                    }
                    Err(error) => app.fail(error),
                },
            );
        }
    });
}

fn shell(app: &mut HostelApp, ui: &mut egui::Ui) {
    app.maybe_sync();
    egui::Panel::left("nav").show(ui, |ui| {
        ui.heading("HostelDesk");
        if let Some(profile) = &app.w.profile {
            ui.label(&profile.name);
        }
        ui.add_space(8.0);
        for (label, nav) in [
            ("Dashboard", Nav::Dashboard),
            ("Rooms", Nav::Rooms),
            ("Residents", Nav::Residents),
            ("Payments", Nav::Payments),
            ("Settings", Nav::Settings),
        ] {
            if ui.selectable_label(app.w.nav == nav, label).clicked() {
                app.w.nav = nav;
            }
        }
        ui.separator();
        if let Some(user) = &app.w.user {
            ui.label(&user.full_name);
            ui.label(RichText::new(&user.role).small());
        }
        if ui.button("Sign out").clicked() {
            app.sign_out();
            app.ok("Signed out.");
            return;
        }
        ui.separator();
        license_badge(app, ui);
        ui.label(RichText::new(format!("v{}", app.desk.app_version())).small());
    });
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match app.w.nav {
                Nav::Dashboard => dashboard(app, ui),
                Nav::Rooms => rooms(app, ui),
                Nav::Residents => residents(app, ui),
                Nav::Payments => payments(app, ui),
                Nav::Settings => settings(app, ui),
            });
    });
}

fn license_badge(app: &HostelApp, ui: &mut egui::Ui) {
    let Some(license) = app.desk.license_status() else {
        return;
    };
    let today = chrono::Local::now().date_naive();
    let days = (license.expires_on - today).num_days();
    let plan = match license.plan.as_str() {
        "trial" => "Free trial",
        "semester" => "Semester plan",
        "1-year" => "1-year plan",
        "3-year" => "3-year plan",
        "5-year" => "5-year plan",
        _ => "License",
    };
    let text = format!("{plan}: {days} days left");
    if days <= 7 {
        ui.colored_label(ui.visuals().error_fg_color, text);
    } else if days <= 30 {
        ui.colored_label(ui.visuals().warn_fg_color, text);
    } else {
        ui.label(RichText::new(text).small());
    }
}

fn dashboard(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Dashboard");
    let currency = app.w.currency().to_string();
    if let Some(stats) = &app.w.stats {
        let occupancy = if stats.total_rooms > 0 {
            stats.occupied * 100 / stats.total_rooms
        } else {
            0
        };
        egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
            for (label, value) in [
                (
                    "Rooms",
                    format!(
                        "{} · {} occupied · {} vacant · {} maintenance",
                        stats.total_rooms, stats.occupied, stats.vacant, stats.maintenance
                    ),
                ),
                ("Occupancy", format!("{occupancy}%")),
                ("Check-ins today", stats.check_ins_today.to_string()),
                (
                    "Revenue today",
                    format!("{currency} {}", stats.revenue_today),
                ),
                (
                    "Revenue this month",
                    format!("{currency} {}", stats.revenue_month),
                ),
                (
                    "Outstanding",
                    format!("{currency} {}", stats.outstanding_total),
                ),
            ] {
                ui.label(label);
                ui.strong(value);
                ui.end_row();
            }
        });
    } else if !app.w.loaded {
        ui.spinner();
    }
    ui.horizontal(|ui| {
        if ui.button("New check-in").clicked() {
            app.w.nav = Nav::Residents;
        }
        if ui.button("Record payment").clicked() {
            app.w.nav = Nav::Payments;
        }
    });
    ui.separator();
    ui.strong("Active stays");
    let stays = app.w.stays.clone();
    if stays.is_empty() {
        ui.label("No one is checked in.");
    }
    egui::Grid::new("active-stays")
        .striped(true)
        .show(ui, |ui| {
            for stay in stays {
                ui.label(&stay.resident_name);
                ui.label(format!("Room {}", stay.room_number));
                ui.label(format!("Balance {currency} {}", stay.balance));
                if ui.small_button("Check out").clicked() {
                    app.confirm = Some(Confirm::CheckOut {
                        allocation_id: stay.allocation_id,
                        name: stay.resident_name.clone(),
                        balance: stay.balance.clone(),
                    });
                }
                ui.end_row();
            }
        });
    ui.separator();
    ui.strong("Recent activity");
    for item in &app.w.activity {
        let amount = item
            .amount
            .as_ref()
            .map(|amount| format!(" · {currency} {amount}"))
            .unwrap_or_default();
        ui.label(format!(
            "{} · {} · {} · room {}{amount}",
            item.at
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M"),
            item.kind.replace('_', " "),
            item.resident_name,
            item.room_number
        ));
    }
}

fn rooms(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Rooms");
    let count = |status: &str| app.w.rooms.iter().filter(|r| r.status == status).count();
    let filters = [
        (None, format!("All ({})", app.w.rooms.len())),
        (Some("vacant"), format!("Vacant ({})", count("vacant"))),
        (
            Some("occupied"),
            format!("Occupied ({})", count("occupied")),
        ),
        (
            Some("maintenance"),
            format!("Maintenance ({})", count("maintenance")),
        ),
    ];
    ui.horizontal(|ui| {
        for (filter, label) in filters {
            if ui
                .selectable_label(app.w.room_filter == filter, label)
                .clicked()
            {
                app.w.room_filter = filter;
            }
        }
    });
    let currency = app.w.currency().to_string();
    egui::Grid::new("rooms").striped(true).show(ui, |ui| {
        for heading in [
            "Room", "Floor", "Type", "Beds", "Free", "Semester", "Recess", "Status",
        ] {
            ui.strong(heading);
        }
        ui.end_row();
        for room in app
            .w
            .rooms
            .iter()
            .filter(|room| app.w.room_filter.is_none_or(|f| room.status == f))
        {
            ui.label(&room.room_number);
            ui.label(room.floor.map(|f| f.to_string()).unwrap_or_default());
            ui.label(&room.room_type);
            ui.label(room.capacity.to_string());
            ui.label((room.capacity - room.occupied_beds).max(0).to_string());
            ui.label(format!("{currency} {}", room.semester_rate));
            ui.label(format!("{currency} {}", room.recess_rate));
            ui.label(&room.status);
            ui.end_row();
        }
    });
    if !app.w.is_admin() {
        return;
    }
    ui.separator();
    ui.strong("Add a room");
    let f = &mut app.w.room_form;
    labeled(ui, "Room number", &mut f.number, false);
    combo(
        ui,
        "Type",
        &mut f.kind,
        &[("single", "Single"), ("double", "Double"), ("dorm", "Dorm")],
    );
    labeled(ui, "Beds", &mut f.capacity, false);
    labeled(ui, "Floor (number, optional)", &mut f.floor, false);
    labeled(ui, "Semester rate", &mut f.semester, false);
    labeled(ui, "Recess rate", &mut f.recess, false);
    let busy = app.busy("add-room");
    if ui
        .add_enabled(!busy, egui::Button::new("Add room"))
        .clicked()
    {
        add_room(app);
    }
}

fn add_room(app: &mut HostelApp) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.room_form;
    let Ok(capacity) = f.capacity.trim().parse::<i32>() else {
        app.fail("Beds must be a whole number.");
        return;
    };
    if capacity < 1 {
        app.fail("A room needs at least one bed.");
        return;
    }
    let floor = match blank(&f.floor).map(|floor| floor.parse::<i32>()) {
        None => None,
        Some(Ok(floor)) => Some(floor),
        Some(Err(_)) => {
            app.fail("Floor must be a whole number, or left blank.");
            return;
        }
    };
    let bad_rate = [("Semester rate", &f.semester), ("Recess rate", &f.recess)]
        .into_iter()
        .find(|(label, rate)| protocol::validate_money(label, rate.trim()).is_err())
        .map(|(label, _)| label);
    if let Some(label) = bad_rate {
        app.fail(format!(
            "{label} must be an amount above zero, like 450000."
        ));
        return;
    }
    let request = AddRoomRequest {
        hostel_id,
        room_number: f.number.trim().to_string(),
        room_type: f.kind.clone(),
        capacity,
        floor,
        semester_rate: f.semester.trim().to_string(),
        recess_rate: f.recess.trim().to_string(),
    };
    let desk = app.desk.clone();
    app.write(
        "add-room",
        async move { desk.add_room(request).await },
        "Room added.",
        |app, _| {
            let kind = std::mem::take(&mut app.w.room_form.kind);
            app.w.room_form = RoomForm {
                kind,
                ..RoomForm::default()
            };
        },
    );
}

fn residents(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Residents");
    let current = app
        .w
        .residents
        .iter()
        .filter(|r| r.status == "active")
        .count();
    let past = app
        .w
        .residents
        .iter()
        .filter(|r| r.status == "checked_out")
        .count();
    ui.horizontal(|ui| {
        for (tab, label) in [
            (ResidentTab::Current, format!("Current ({current})")),
            (ResidentTab::Past, format!("Past ({past})")),
            (ResidentTab::All, format!("All ({})", app.w.residents.len())),
        ] {
            if ui
                .selectable_label(app.w.resident_tab == tab, label)
                .clicked()
            {
                app.w.resident_tab = tab;
            }
        }
    });
    labeled(
        ui,
        "Search name, phone, or room",
        &mut app.w.resident_query,
        false,
    );
    let query = app.w.resident_query.trim().to_lowercase();
    let tab = app.w.resident_tab;
    let rows: Vec<db::ResidentSummary> = app
        .w
        .residents
        .iter()
        .filter(|r| match tab {
            ResidentTab::Current => r.status == "active",
            ResidentTab::Past => r.status == "checked_out",
            ResidentTab::All => true,
        })
        .filter(|r| {
            query.is_empty()
                || r.full_name.to_lowercase().contains(&query)
                || r.phone.as_deref().unwrap_or("").contains(&query)
                || r.room_number
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query)
        })
        .cloned()
        .collect();
    let currency = app.w.currency().to_string();
    let selected = app.w.detail.as_ref().map(|d| d.resident.id);
    egui::Grid::new("residents").striped(true).show(ui, |ui| {
        for resident in rows {
            let label = format!(
                "{} · room {} · {}",
                resident.full_name,
                resident.room_number.as_deref().unwrap_or("—"),
                resident.status.replace('_', " ")
            );
            if ui
                .selectable_label(selected == Some(resident.id), label)
                .clicked()
            {
                load_resident(app, resident.id);
            }
            ui.label(format!("Balance {currency} {}", resident.balance));
            ui.end_row();
        }
    });
    ui.separator();
    resident_detail(app, ui);
    ui.separator();
    check_in_form(app, ui);
}

fn load_resident(app: &mut HostelApp, resident_id: i64) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let desk = app.desk.clone();
    app.run(
        "resident",
        async move {
            desk.resident_detail(GetResidentDetailRequest {
                hostel_id,
                resident_id,
            })
            .await
        },
        |app, result| match result {
            Ok(detail) => {
                let r = &detail.resident;
                app.w.resident_form = ResidentForm {
                    name: r.full_name.clone(),
                    phone: r.phone.clone().unwrap_or_default(),
                    email: r.email.clone().unwrap_or_default(),
                    id_number: r.id_number.clone().unwrap_or_default(),
                    kin: r.next_of_kin.clone().unwrap_or_default(),
                    kin_phone: r.next_of_kin_phone.clone().unwrap_or_default(),
                };
                app.w.detail = Some(detail);
            }
            Err(error) => app.fail(error),
        },
    );
}

fn resident_detail(app: &mut HostelApp, ui: &mut egui::Ui) {
    let Some(detail) = app.w.detail.clone() else {
        ui.label("Select a resident to see their stays and payments.");
        return;
    };
    let currency = app.w.currency().to_string();
    ui.horizontal(|ui| {
        ui.strong(&detail.resident.full_name);
        ui.label(format!("· balance {currency} {}", detail.balance));
        if ui.small_button("Close").clicked() {
            app.w.detail = None;
        }
    });
    let f = &mut app.w.resident_form;
    labeled(ui, "Name", &mut f.name, false);
    labeled(ui, "Phone", &mut f.phone, false);
    labeled(ui, "Email", &mut f.email, false);
    labeled(ui, "ID number", &mut f.id_number, false);
    labeled(ui, "Next of kin", &mut f.kin, false);
    labeled(ui, "Next-of-kin phone", &mut f.kin_phone, false);
    let active = detail
        .allocations
        .iter()
        .find(|stay| stay.status == "active")
        .cloned();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !app.busy("save-resident"),
                egui::Button::new("Save details"),
            )
            .clicked()
        {
            save_resident(app, detail.resident.id);
        }
        if let Some(stay) = &active {
            if ui.button("Record payment").clicked() {
                start_payment(app, stay.id);
            }
            if ui.button("Check out").clicked() {
                let balance = app
                    .w
                    .stay(stay.id)
                    .map_or_else(|| detail.balance.clone(), |s| s.balance.clone());
                app.confirm = Some(Confirm::CheckOut {
                    allocation_id: stay.id,
                    name: detail.resident.full_name.clone(),
                    balance,
                });
            }
        } else if ui.button("Check in again").clicked() {
            app.w.check_in = CheckInForm {
                returning: Some(detail.resident.id),
                ..CheckInForm::default()
            };
        }
    });
    ui.strong("Stays");
    egui::Grid::new("stays").striped(true).show(ui, |ui| {
        for stay in &detail.allocations {
            ui.label(format!("Room {}", stay.room_number));
            ui.label(format!("{} {}", term_label(&stay.term), stay.academic_year));
            ui.label(format!("{currency} {}", stay.rate_agreed));
            ui.label(stay.status.replace('_', " "));
            ui.label(
                stay.created_at
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d")
                    .to_string(),
            );
            ui.end_row();
        }
    });
    ui.strong("Payments");
    if detail.payments.is_empty() {
        ui.label("No payments yet.");
    }
    egui::Grid::new("resident-payments")
        .striped(true)
        .show(ui, |ui| {
            for payment in &detail.payments {
                ui.label(
                    payment
                        .payment_date
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d")
                        .to_string(),
                );
                ui.label(format!("{currency} {}", payment.amount));
                ui.label(method_label(&payment.payment_method));
                ui.end_row();
            }
        });
}

fn save_resident(app: &mut HostelApp, resident_id: i64) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.resident_form;
    let request = UpdateResidentRequest {
        hostel_id,
        resident_id,
        full_name: f.name.trim().to_string(),
        phone: blank(&f.phone),
        email: blank(&f.email),
        id_number: blank(&f.id_number),
        next_of_kin: blank(&f.kin),
        next_of_kin_phone: blank(&f.kin_phone),
    };
    let desk = app.desk.clone();
    app.write(
        "save-resident",
        async move { desk.update_resident(request).await },
        "Resident saved.",
        move |app, ()| load_resident(app, resident_id),
    );
}

fn check_in_form(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.strong("Check in");
    let returning_label = app
        .w
        .check_in
        .returning
        .and_then(|id| app.w.residents.iter().find(|r| r.id == id))
        .map_or_else(|| "New resident".to_string(), |r| r.full_name.clone());
    let returning_choices: Vec<(i64, String)> = app
        .w
        .residents
        .iter()
        .filter(|r| r.status != "active")
        .map(|r| (r.id, r.full_name.clone()))
        .collect();
    let f = &mut app.w.check_in;
    egui::ComboBox::from_label("Resident")
        .selected_text(returning_label)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut f.returning, None, "New resident");
            for (id, name) in &returning_choices {
                ui.selectable_value(&mut f.returning, Some(*id), name);
            }
        });
    if f.returning.is_none() {
        labeled(ui, "Full name", &mut f.name, false);
        labeled(ui, "Phone", &mut f.phone, false);
        labeled(ui, "Email", &mut f.email, false);
        labeled(ui, "ID number", &mut f.id_number, false);
        labeled(ui, "Next of kin", &mut f.kin, false);
        labeled(ui, "Next-of-kin phone", &mut f.kin_phone, false);
    }
    let free: Vec<&db::Room> = app
        .w
        .rooms
        .iter()
        .filter(|room| room.status != "maintenance" && room.occupied_beds < room.capacity)
        .collect();
    let room_text = f
        .room
        .and_then(|id| free.iter().find(|room| room.id == id))
        .map_or_else(|| "Choose a room".to_string(), |room| room_choice(room));
    let before = (f.room, f.term.clone());
    egui::ComboBox::from_label("Room")
        .selected_text(room_text)
        .show_ui(ui, |ui| {
            for room in &free {
                ui.selectable_value(&mut f.room, Some(room.id), room_choice(room));
            }
        });
    combo(ui, "Term", &mut f.term, &TERMS);
    if (f.room, f.term.clone()) != before {
        if let Some(room) = f.room.and_then(|id| free.iter().find(|room| room.id == id)) {
            f.rate = rate_for_term(room, &f.term).to_string();
        }
    }
    if free.is_empty() {
        ui.colored_label(ui.visuals().warn_fg_color, "No room has a free bed.");
    }
    labeled(ui, "Rate agreed", &mut f.rate, false);
    labeled(ui, "Academic year", &mut f.year, false);
    if ui
        .add_enabled(!app.busy("check-in"), egui::Button::new("Check in"))
        .clicked()
    {
        check_in(app);
    }
}

fn check_in(app: &mut HostelApp) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.check_in;
    let Some(room_id) = f.room else {
        app.fail("Choose a room.");
        return;
    };
    if f.returning.is_none() && f.name.trim().is_empty() {
        app.fail("Enter the resident's full name.");
        return;
    }
    if protocol::validate_money("Rate", f.rate.trim()).is_err() {
        app.fail("Rate must be an amount above zero, like 450000.");
        return;
    }
    let new = f.returning.is_none();
    let request = CheckInRequest {
        hostel_id,
        room_id,
        resident_id: f.returning,
        full_name: if new { blank(&f.name) } else { None },
        phone: if new { blank(&f.phone) } else { None },
        email: if new { blank(&f.email) } else { None },
        id_number: if new { blank(&f.id_number) } else { None },
        next_of_kin: if new { blank(&f.kin) } else { None },
        next_of_kin_phone: if new { blank(&f.kin_phone) } else { None },
        photo_path: None,
        rate_agreed: f.rate.trim().to_string(),
        term: f.term.clone(),
        academic_year: f.year.trim().to_string(),
    };
    let desk = app.desk.clone();
    app.write(
        "check-in",
        async move { desk.check_in(request).await },
        "Checked in.",
        |app, _| {
            let f = &mut app.w.check_in;
            let (term, year) = (std::mem::take(&mut f.term), std::mem::take(&mut f.year));
            *f = CheckInForm {
                term,
                year,
                ..CheckInForm::default()
            };
        },
    );
}

fn payments(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Payments");
    ui.horizontal(|ui| {
        for (tab, label) in [
            (PaymentTab::Outstanding, "Outstanding"),
            (PaymentTab::History, "History"),
        ] {
            if ui
                .selectable_label(app.w.payment_tab == tab, label)
                .clicked()
            {
                app.w.payment_tab = tab;
            }
        }
    });
    let currency = app.w.currency().to_string();
    match app.w.payment_tab {
        PaymentTab::Outstanding => {
            let stays = app.w.stays.clone();
            egui::Grid::new("outstanding").striped(true).show(ui, |ui| {
                for heading in ["Resident", "Room", "Term", "Rate", "Paid", "Balance", ""] {
                    ui.strong(heading);
                }
                ui.end_row();
                for stay in &stays {
                    ui.label(&stay.resident_name);
                    ui.label(&stay.room_number);
                    ui.label(format!("{} {}", term_label(&stay.term), stay.academic_year));
                    ui.label(format!("{currency} {}", stay.rate_agreed));
                    ui.label(format!("{currency} {}", stay.total_paid));
                    ui.strong(format!("{currency} {}", stay.balance));
                    if ui.small_button("Take payment").clicked() {
                        start_payment(app, stay.allocation_id);
                    }
                    ui.end_row();
                }
            });
        }
        PaymentTab::History => {
            let rows = app.w.payments.clone();
            egui::Grid::new("history").striped(true).show(ui, |ui| {
                for heading in [
                    "Date", "Receipt", "Resident", "Room", "Amount", "Method", "",
                ] {
                    ui.strong(heading);
                }
                ui.end_row();
                for row in rows {
                    ui.label(
                        row.payment_date
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d")
                            .to_string(),
                    );
                    ui.label(row.receipt_number.clone().unwrap_or_else(|| "—".into()));
                    ui.label(&row.resident_name);
                    ui.label(&row.room_number);
                    ui.label(format!("{currency} {}", row.amount));
                    ui.label(method_label(&row.payment_method));
                    if row.receipt_number.is_some()
                        && ui
                            .add_enabled(!app.busy("receipt"), egui::Button::new("Open receipt"))
                            .clicked()
                    {
                        reopen_receipt(app, row);
                    }
                    ui.end_row();
                }
            });
        }
    }
    ui.separator();
    pay_form(app, ui);
}

/// Select `allocation_id` in the payment form and pre-fill its balance.
fn start_payment(app: &mut HostelApp, allocation_id: i64) {
    app.w.nav = Nav::Payments;
    app.w.pay.allocation = Some(allocation_id);
    app.w.pay.amount = app
        .w
        .stay(allocation_id)
        .map(|stay| stay.balance.clone())
        .filter(|balance| Decimal::from_str(balance).is_ok_and(|b| b > Decimal::ZERO))
        .unwrap_or_default();
}

fn pay_form(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.strong("Record a payment");
    let current = app
        .w
        .pay
        .allocation
        .and_then(|id| app.w.stay(id))
        .map_or_else(
            || "Choose a stay".to_string(),
            |stay| format!("{} · room {}", stay.resident_name, stay.room_number),
        );
    let choices: Vec<(i64, String)> = app
        .w
        .stays
        .iter()
        .map(|stay| {
            (
                stay.allocation_id,
                format!(
                    "{} · room {} · owes {}",
                    stay.resident_name, stay.room_number, stay.balance
                ),
            )
        })
        .collect();
    let mut chosen = app.w.pay.allocation;
    egui::ComboBox::from_label("Stay")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (id, label) in &choices {
                ui.selectable_value(&mut chosen, Some(*id), label);
            }
        });
    if chosen != app.w.pay.allocation {
        if let Some(id) = chosen {
            start_payment(app, id);
        }
    }
    let f = &mut app.w.pay;
    labeled(ui, "Amount", &mut f.amount, false);
    combo(ui, "Method", &mut f.method, &METHODS);
    labeled(ui, "Notes", &mut f.notes, false);
    if ui
        .add_enabled(
            !app.busy("pay"),
            egui::Button::new("Record payment and print receipt"),
        )
        .clicked()
    {
        record_payment(app);
    }
}

fn record_payment(app: &mut HostelApp) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.pay;
    let Some(stay) = f.allocation.and_then(|id| app.w.stay(id)).cloned() else {
        app.fail("Choose an active stay.");
        return;
    };
    let amount = f.amount.trim().to_string();
    let Ok(paid) = protocol::validate_money("Amount", &amount) else {
        app.fail("Amount must be above zero, like 150000.");
        return;
    };
    let (Some(profile), Some(prefs)) = (app.w.profile.clone(), app.w.prefs.clone()) else {
        app.fail("The hostel profile has not loaded yet. Try again in a moment.");
        return;
    };
    let method = f.method.clone();
    let notes = blank(&f.notes);
    let request = RecordPaymentRequest {
        hostel_id,
        allocation_id: stay.allocation_id,
        resident_id: stay.resident_id,
        amount: amount.clone(),
        payment_method: method.clone(),
        period_start: None,
        period_end: None,
        notes: notes.clone(),
    };
    let balance = balance_after(&stay.balance, paid);
    let desk = app.desk.clone();
    app.run(
        "pay",
        async move {
            let (_, receipt_number) = desk.record_payment_and_generate_receipt(request).await?;
            let receipt = GenerateReceiptRequest {
                hostel_id,
                hostel_name: profile.name,
                hostel_address: profile.address,
                hostel_phone: profile.phone,
                receipt_number: receipt_number.clone(),
                resident_name: stay.resident_name,
                room_number: stay.room_number,
                amount,
                date: chrono::Local::now().date_naive(),
                balance,
                payment_method: method,
                notes,
                currency_code: profile.currency,
                footer_text: prefs.receipt_footer,
                logo_path: profile.branding_logo_path,
            };
            let printed = desk.generate_and_open_receipt(receipt).await;
            Ok::<_, String>((receipt_number, printed))
        },
        |app, result| match result {
            Ok((receipt_number, printed)) => {
                app.w.pay = PayForm::default();
                match printed {
                    Ok(_) => app.ok(format!(
                        "Payment recorded. Receipt {receipt_number} opened."
                    )),
                    Err(error) => app.fail(format!(
                        "Payment recorded as receipt {receipt_number}, but the PDF failed: {error}"
                    )),
                }
                app.refresh();
            }
            Err(error) => app.fail(error),
        },
    );
}

/// Open a printed receipt again, or print it from the ledger when this desk
/// has no PDF for it (for example a payment taken on another desk).
fn reopen_receipt(app: &mut HostelApp, row: db::PaymentRow) {
    let (Some(hostel_id), Some(profile), Some(prefs)) = (
        app.w.hostel_id(),
        app.w.profile.clone(),
        app.w.prefs.clone(),
    ) else {
        return;
    };
    if let Some(path) = row
        .pdf_path
        .as_deref()
        .filter(|p| std::path::Path::new(p).exists())
    {
        match open::that(path) {
            Ok(()) => app.ok("Receipt opened."),
            Err(error) => app.fail(format!("Could not open the receipt: {error}")),
        }
        return;
    }
    let desk = app.desk.clone();
    app.run(
        "receipt",
        async move {
            let detail = desk
                .resident_detail(GetResidentDetailRequest {
                    hostel_id,
                    resident_id: row.resident_id,
                })
                .await?;
            let balance = balance_at_payment(&detail, row.allocation_id, row.id)?;
            desk.generate_and_open_receipt(GenerateReceiptRequest {
                hostel_id,
                hostel_name: profile.name,
                hostel_address: profile.address,
                hostel_phone: profile.phone,
                receipt_number: row.receipt_number.unwrap_or_default(),
                resident_name: row.resident_name,
                room_number: row.room_number,
                amount: row.amount,
                date: row.payment_date.with_timezone(&chrono::Local).date_naive(),
                balance,
                payment_method: row.payment_method,
                notes: row.notes,
                currency_code: profile.currency,
                footer_text: prefs.receipt_footer,
                logo_path: profile.branding_logo_path,
            })
            .await
        },
        |app, result| match result {
            Ok(_) => app.ok("Receipt opened."),
            Err(error) => app.fail(error),
        },
    );
}

fn settings(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Settings");
    let tabs: &[(SettingsTab, &str)] = if app.w.is_admin() {
        &[
            (SettingsTab::Account, "Account"),
            (SettingsTab::Business, "Business"),
            (SettingsTab::Team, "Team"),
            (SettingsTab::Preferences, "Receipts"),
            (SettingsTab::Backup, "Backup"),
            (SettingsTab::Cloud, "Cloud"),
        ]
    } else {
        &[
            (SettingsTab::Account, "Account"),
            (SettingsTab::Cloud, "Cloud"),
        ]
    };
    if !tabs.iter().any(|(tab, _)| *tab == app.w.settings) {
        app.w.settings = SettingsTab::Account;
    }
    ui.horizontal(|ui| {
        for (tab, label) in tabs {
            if ui
                .selectable_label(app.w.settings == *tab, *label)
                .clicked()
            {
                app.w.settings = *tab;
            }
        }
    });
    ui.separator();
    match app.w.settings {
        SettingsTab::Account => account(app, ui),
        SettingsTab::Business => business(app, ui),
        SettingsTab::Team => team(app, ui),
        SettingsTab::Preferences => preferences(app, ui),
        SettingsTab::Backup => backup(app, ui),
        SettingsTab::Cloud => cloud(app, ui),
    }
}

fn account(app: &mut HostelApp, ui: &mut egui::Ui) {
    let f = &mut app.w.account;
    labeled(ui, "Name", &mut f.name, false);
    labeled(ui, "Email", &mut f.email, false);
    ui.add_space(8.0);
    ui.label("Change password (leave blank to keep it)");
    labeled(ui, "Current password", &mut f.current, true);
    labeled(ui, "New password", &mut f.new, true);
    labeled(ui, "Repeat new password", &mut f.confirm, true);
    if !ui
        .add_enabled(!app.busy("account"), egui::Button::new("Save account"))
        .clicked()
    {
        return;
    }
    let Some(user) = app.w.user.clone() else {
        return;
    };
    let f = &app.w.account;
    let password = blank(&f.new);
    if password.is_some() && f.new != f.confirm {
        app.fail("The new passwords do not match.");
        return;
    }
    let request = UpdateUserRequest {
        user_id: user.id,
        full_name: f.name.trim().to_string(),
        email: f.email.trim().to_string(),
        role: user.role.clone(),
        current_password: password.as_ref().map(|_| f.current.clone()),
        password,
    };
    let desk = app.desk.clone();
    app.run(
        "account",
        async move { desk.update_user(request).await },
        |app, result| match result {
            Ok(updated) => {
                let f = &mut app.w.account;
                f.current.clear();
                f.new.clear();
                f.confirm.clear();
                app.w.user = Some(updated);
                app.ok("Account saved.");
            }
            Err(error) => app.fail(error),
        },
    );
}

fn business(app: &mut HostelApp, ui: &mut egui::Ui) {
    let loaded = app.w.profile.is_some();
    let f = &mut app.w.business;
    let mut changed = false;
    changed |= labeled(ui, "Hostel name", &mut f.name, false).changed();
    changed |= labeled(ui, "Address", &mut f.address, false).changed();
    changed |= labeled(ui, "Phone", &mut f.phone, false).changed();
    changed |= labeled(ui, "Currency (3 letters)", &mut f.currency, false).changed();
    f.dirty |= changed;
    ui.label(match &f.logo {
        Some(path) => format!("Logo: {path}"),
        None => "No logo.".to_string(),
    });
    let dirty = f.dirty;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                loaded && !app.busy("logo"),
                egui::Button::new("Choose logo"),
            )
            .clicked()
        {
            upload_logo(app);
        }
        if ui
            .add_enabled(
                loaded && dirty && !app.busy("business"),
                egui::Button::new("Save business profile"),
            )
            .clicked()
        {
            save_business(app);
        }
        if dirty && ui.button("Discard changes").clicked() {
            app.w.business.dirty = false;
            app.refresh();
        }
    });
    if !loaded {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "The saved profile has not loaded, so it cannot be edited yet.",
        );
    }
}

fn save_business(app: &mut HostelApp) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.business;
    let request = UpdateHostelRequest {
        hostel_id,
        name: f.name.trim().to_string(),
        address: blank(&f.address),
        phone: blank(&f.phone),
        currency: f.currency.trim().to_uppercase(),
        branding_logo_path: f.logo.clone(),
    };
    let desk = app.desk.clone();
    app.write(
        "business",
        async move { desk.update_hostel(request).await },
        "Business profile saved.",
        |app, _| app.w.business.dirty = false,
    );
}

fn upload_logo(app: &mut HostelApp) {
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let Some(path) = rfd::FileDialog::new()
        .set_title("Hostel logo")
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
    else {
        return;
    };
    let desk = app.desk.clone();
    app.run(
        "logo",
        async move {
            let data = tokio::fs::read(&path)
                .await
                .map_err(|e| format!("Could not read the logo: {e}"))?;
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("logo.png")
                .to_string();
            desk.upload_hostel_logo(UploadHostelLogoRequest {
                hostel_id,
                data,
                filename,
            })
            .await
        },
        |app, result| match result {
            Ok(stored) => {
                app.w.business.logo = Some(stored);
                app.w.business.dirty = true;
                save_business(app);
            }
            Err(error) => app.fail(error),
        },
    );
}

fn team(app: &mut HostelApp, ui: &mut egui::Ui) {
    let me = app.w.user.as_ref().map(|user| user.id);
    egui::Grid::new("team").striped(true).show(ui, |ui| {
        for user in app.w.users.clone() {
            ui.label(&user.full_name);
            ui.label(&user.email);
            ui.label(&user.role);
            if Some(user.id) == me {
                ui.label(RichText::new("you").small());
            } else if ui.small_button("Remove").clicked() {
                app.confirm = Some(Confirm::DeleteUser(user));
            }
            ui.end_row();
        }
    });
    ui.separator();
    ui.strong("Add staff");
    let f = &mut app.w.team;
    labeled(ui, "Name", &mut f.name, false);
    labeled(ui, "Email", &mut f.email, false);
    labeled(ui, "Password (8+ characters)", &mut f.password, true);
    combo(
        ui,
        "Role",
        &mut f.role,
        &[("manager", "Manager"), ("admin", "Admin")],
    );
    if !ui
        .add_enabled(
            !app.busy("add-user"),
            egui::Button::new("Create staff account"),
        )
        .clicked()
    {
        return;
    }
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.team;
    let request = CreateUserRequest {
        hostel_id,
        full_name: f.name.trim().to_string(),
        email: f.email.trim().to_string(),
        password: f.password.clone(),
        role: f.role.clone(),
    };
    let desk = app.desk.clone();
    app.write(
        "add-user",
        async move { desk.create_user(request).await },
        "Staff account created.",
        |app, _| app.w.team = TeamForm::default(),
    );
}

fn preferences(app: &mut HostelApp, ui: &mut egui::Ui) {
    let loaded = app.w.prefs.is_some();
    let f = &mut app.w.prefs_form;
    let before = f.date_format.clone();
    combo(
        ui,
        "Date format",
        &mut f.date_format,
        &[
            ("dd/MM/yyyy", "dd/MM/yyyy"),
            ("MM/dd/yyyy", "MM/dd/yyyy"),
            ("yyyy-MM-dd", "yyyy-MM-dd"),
        ],
    );
    let changed = before != f.date_format;
    f.dirty |= changed | labeled(ui, "Receipt footer", &mut f.footer, false).changed();
    let dirty = f.dirty;
    if !ui
        .add_enabled(
            loaded && dirty && !app.busy("prefs"),
            egui::Button::new("Save receipt settings"),
        )
        .clicked()
    {
        return;
    }
    let Some(hostel_id) = app.w.hostel_id() else {
        return;
    };
    let f = &app.w.prefs_form;
    let request = UpsertAppPreferencesRequest {
        hostel_id,
        date_format: f.date_format.clone(),
        language: "en".to_string(),
        receipt_footer: blank(&f.footer),
    };
    let desk = app.desk.clone();
    app.write(
        "prefs",
        async move { desk.update_app_preferences(request).await },
        "Receipt settings saved.",
        |app, _| app.w.prefs_form.dirty = false,
    );
}

fn backup(app: &mut HostelApp, ui: &mut egui::Ui) {
    if ui
        .add_enabled(!app.busy("backup"), egui::Button::new("Back up now"))
        .clicked()
    {
        let desk = app.desk.clone();
        app.write(
            "backup",
            async move { desk.backup_database().await },
            "Backup written.",
            |_, _| {},
        );
    }
    let enrolled = app.w.cloud.as_ref().is_some_and(|c| c.server_url.is_some());
    if enrolled {
        ui.label("This computer syncs with the cloud, which keeps your data. Restoring is off.");
    }
    ui.separator();
    if app.w.backups.is_empty() {
        ui.label("No backups yet.");
    }
    for path in app.w.backups.clone() {
        ui.horizontal(|ui| {
            ui.label(&path);
            if !enrolled && ui.small_button("Restore").clicked() {
                app.confirm = Some(Confirm::Restore(path.clone()));
            }
        });
    }
}

fn cloud(app: &mut HostelApp, ui: &mut egui::Ui) {
    let Some(status) = app.w.cloud.clone() else {
        ui.spinner();
        return;
    };
    match &status.server_url {
        None => {
            ui.label(
                "This computer is not connected to the cloud. Connecting uploads this \
                 hostel so other computers in your organization can share it.",
            );
            if app.w.is_admin() {
                cloud_connect_form(app, ui);
            }
            return;
        }
        Some(url) => {
            egui::Grid::new("cloud").num_columns(2).show(ui, |ui| {
                ui.label("Server");
                ui.label(url);
                ui.end_row();
                ui.label("Receipt prefix");
                ui.label(status.device_prefix.as_deref().unwrap_or("—"));
                ui.end_row();
                ui.label("Waiting to send");
                ui.label(status.pending.to_string());
                ui.end_row();
            });
        }
    }
    if ui
        .add_enabled(!app.busy("sync"), egui::Button::new("Sync now"))
        .clicked()
    {
        app.sync_now();
    }
    if !status.issues.is_empty() {
        ui.separator();
        ui.strong("Needs attention");
        for issue in &status.issues {
            let text = format!(
                "{} · {} · {}",
                issue.created_at,
                issue.command_type.replace('_', " "),
                issue.message
            );
            if issue.rejected {
                ui.colored_label(ui.visuals().error_fg_color, format!("Not accepted: {text}"));
            } else {
                ui.colored_label(ui.visuals().warn_fg_color, text);
            }
        }
    }
}

fn labeled(ui: &mut egui::Ui, label: &str, value: &mut String, password: bool) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).password(password))
    })
    .inner
}

fn combo(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[(&str, &str)]) {
    let current = options
        .iter()
        .find(|(key, _)| key == value)
        .map_or(value.as_str(), |(_, text)| *text)
        .to_string();
    egui::ComboBox::from_label(label)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (key, text) in options {
                ui.selectable_value(value, (*key).to_string(), *text);
            }
        });
}

fn room_choice(room: &db::Room) -> String {
    format!(
        "{} · {} · {} of {} beds free",
        room.room_number,
        room.room_type,
        room.capacity - room.occupied_beds,
        room.capacity
    )
}

fn term_label(term: &str) -> &str {
    TERMS
        .iter()
        .find(|(key, _)| *key == term)
        .map_or(term, |(_, label)| label)
}

fn method_label(method: &str) -> &str {
    METHODS
        .iter()
        .find(|(key, _)| *key == method)
        .map_or(method, |(_, label)| label)
}

fn blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Academic years run August to July: March 2027 is still `2026/2027`.
fn academic_year(today: NaiveDate) -> String {
    let start = if today.month() >= 8 {
        today.year()
    } else {
        today.year() - 1
    };
    format!("{start}/{}", start + 1)
}

/// The room's rate for `term`: recess has its own rate.
fn rate_for_term<'a>(room: &'a db::Room, term: &str) -> &'a str {
    if term == "recess" {
        &room.recess_rate
    } else {
        &room.semester_rate
    }
}

/// What is still owed once `paid` comes off `balance`, never below zero.
fn balance_after(balance: &str, paid: Decimal) -> String {
    let owed = Decimal::from_str(balance.trim()).unwrap_or(Decimal::ZERO);
    (owed - paid).max(Decimal::ZERO).to_string()
}

/// Balance left on `allocation_id` right after payment `payment_id`, from the
/// resident's ledger: the stay's rate minus that payment and every earlier one.
fn balance_at_payment(
    detail: &db::ResidentDetail,
    allocation_id: i64,
    payment_id: i64,
) -> Result<String, String> {
    let stay = detail
        .allocations
        .iter()
        .find(|stay| stay.id == allocation_id)
        .ok_or("That stay is no longer on record.")?;
    let rate = Decimal::from_str(&stay.rate_agreed).map_err(|e| e.to_string())?;
    let target = detail
        .payments
        .iter()
        .find(|p| p.id == payment_id)
        .ok_or("That payment is no longer on record.")?;
    let mut paid = Decimal::ZERO;
    for payment in detail.payments.iter().filter(|p| {
        p.allocation_id == allocation_id
            && (p.payment_date, p.id) <= (target.payment_date, target.id)
    }) {
        paid += Decimal::from_str(&payment.amount).map_err(|e| e.to_string())?;
    }
    Ok((rate - paid).max(Decimal::ZERO).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn academic_year_turns_over_in_august() {
        let day = |y, m| NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        assert_eq!(academic_year(day(2027, 3)), "2026/2027");
        assert_eq!(academic_year(day(2027, 7)), "2026/2027");
        assert_eq!(academic_year(day(2027, 8)), "2027/2028");
    }

    #[test]
    fn receipt_balance_is_what_remains_after_this_payment() {
        assert_eq!(balance_after("500000", Decimal::from(200_000)), "300000");
        assert_eq!(
            balance_after("100.50", Decimal::from_str("0.50").unwrap()),
            "100.00"
        );
        assert_eq!(
            balance_after("50", Decimal::from(80)),
            "0",
            "never negative"
        );
    }
}
