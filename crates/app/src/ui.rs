//! Native screens. Each screen calls [`crate::Desk`] and never the database.

use chrono::Datelike;
use db::User;
use eframe::egui::{self, RichText};

use crate::commands::{
    AddRoomRequest, CheckInRequest, CheckOutRequest, CompleteOnboardingRequest, CreateUserRequest,
    DeleteUserRequest, GenerateReceiptRequest, GetAppPreferencesRequest, GetDashboardStatsRequest,
    GetResidentDetailRequest, ListActiveAllocationsRequest, ListRecentActivityRequest,
    ListResidentsRequest, ListUsersRequest, LoginRequest, RecordPaymentRequest,
    RestoreBackupRequest, UpdateHostelRequest, UpdateResidentRequest, UpdateUserRequest,
    UploadHostelLogoRequest, UpsertAppPreferencesRequest,
};
use crate::runtime;
use crate::Desk;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gate {
    Boot,
    Lock,
    Onboarding,
    SignIn,
    Ready,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Dashboard,
    Rooms,
    Residents,
    Payments,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Business,
    Team,
    Preferences,
    Backup,
    Account,
    Cloud,
}

/// Top-level egui application.
pub struct HostelApp {
    desk: Desk,
    gate: Gate,
    nav: Nav,
    settings: SettingsTab,
    status: String,
    user: Option<User>,
    hostel_name: String,
    hostel_address: String,
    hostel_phone: String,
    currency: String,
    logo_path: Option<String>,
    onboard_name: String,
    onboard_address: String,
    onboard_phone: String,
    onboard_admin: String,
    onboard_email: String,
    onboard_password: String,
    email: String,
    password: String,
    rooms: Vec<db::Room>,
    room_number: String,
    room_type: String,
    room_capacity: String,
    room_floor: String,
    semester_rate: String,
    recess_rate: String,
    residents: Vec<db::ResidentSummary>,
    resident_query: String,
    selected_resident: Option<i64>,
    detail_name: String,
    detail_phone: String,
    detail_email: String,
    detail_id: String,
    detail_kin: String,
    detail_kin_phone: String,
    detail_balance: String,
    detail_allocation: Option<i64>,
    checkin_name: String,
    checkin_phone: String,
    checkin_email: String,
    checkin_id: String,
    checkin_kin: String,
    checkin_kin_phone: String,
    checkin_room: Option<i64>,
    checkin_rate: String,
    checkin_term: String,
    checkin_year: String,
    allocations: Vec<db::ActiveAllocation>,
    payments: Vec<db::PaymentRow>,
    pay_allocation: Option<i64>,
    pay_amount: String,
    pay_method: String,
    pay_notes: String,
    stats: Option<db::DashboardStats>,
    activity: Vec<db::ActivityItem>,
    users: Vec<User>,
    new_user_name: String,
    new_user_email: String,
    new_user_password: String,
    new_user_role: String,
    account_name: String,
    account_email: String,
    account_password: String,
    date_format: String,
    receipt_footer: String,
    backups: Vec<String>,
    cloud_server: String,
    cloud_email: String,
    cloud_password: String,
    cloud_org: String,
    cloud_summary: String,
    cloud_conflicts: Vec<String>,
    last_sync: Option<std::time::Instant>,
    loaded: bool,
}

impl HostelApp {
    /// Start on the lock / onboarding / sign-in gate that matches local state.
    pub fn new(desk: Desk) -> Self {
        let mut app = Self {
            desk,
            gate: Gate::Boot,
            nav: Nav::Dashboard,
            settings: SettingsTab::Business,
            status: String::new(),
            user: None,
            hostel_name: String::new(),
            hostel_address: String::new(),
            hostel_phone: String::new(),
            currency: "UGX".to_string(),
            logo_path: None,
            onboard_name: String::new(),
            onboard_address: String::new(),
            onboard_phone: String::new(),
            onboard_admin: String::new(),
            onboard_email: String::new(),
            onboard_password: String::new(),
            email: String::new(),
            password: String::new(),
            rooms: Vec::new(),
            room_number: String::new(),
            room_type: "single".to_string(),
            room_capacity: "1".to_string(),
            room_floor: String::new(),
            semester_rate: String::new(),
            recess_rate: String::new(),
            residents: Vec::new(),
            resident_query: String::new(),
            selected_resident: None,
            detail_name: String::new(),
            detail_phone: String::new(),
            detail_email: String::new(),
            detail_id: String::new(),
            detail_kin: String::new(),
            detail_kin_phone: String::new(),
            detail_balance: String::new(),
            detail_allocation: None,
            checkin_name: String::new(),
            checkin_phone: String::new(),
            checkin_email: String::new(),
            checkin_id: String::new(),
            checkin_kin: String::new(),
            checkin_kin_phone: String::new(),
            checkin_room: None,
            checkin_rate: String::new(),
            checkin_term: "semester_1".to_string(),
            checkin_year: default_academic_year(),
            allocations: Vec::new(),
            payments: Vec::new(),
            pay_allocation: None,
            pay_amount: String::new(),
            pay_method: "cash".to_string(),
            pay_notes: String::new(),
            stats: None,
            activity: Vec::new(),
            users: Vec::new(),
            new_user_name: String::new(),
            new_user_email: String::new(),
            new_user_password: String::new(),
            new_user_role: "manager".to_string(),
            account_name: String::new(),
            account_email: String::new(),
            account_password: String::new(),
            date_format: "dd/MM/yyyy".to_string(),
            receipt_footer: String::new(),
            backups: Vec::new(),
            cloud_server: String::new(),
            cloud_email: String::new(),
            cloud_password: String::new(),
            cloud_org: String::new(),
            cloud_summary: String::new(),
            cloud_conflicts: Vec::new(),
            last_sync: None,
            loaded: false,
        };
        app.boot();
        app
    }

    fn boot(&mut self) {
        if self.desk.license_status().is_none() {
            self.gate = Gate::Lock;
            return;
        }
        match runtime().block_on(self.desk.onboarding_status()) {
            Ok(true) => match runtime().block_on(self.desk.session()) {
                Ok(Some(user)) => self.enter(user),
                Ok(None) => self.gate = Gate::SignIn,
                Err(e) => {
                    self.status = e;
                    self.gate = Gate::SignIn;
                }
            },
            Ok(false) => self.gate = Gate::Onboarding,
            Err(e) => {
                self.status = e;
                self.gate = Gate::Lock;
            }
        }
    }

    fn enter(&mut self, user: User) {
        self.account_name.clone_from(&user.full_name);
        self.account_email.clone_from(&user.email);
        self.user = Some(user);
        self.gate = Gate::Ready;
        self.loaded = false;
        self.refresh();
    }

    fn hostel_id(&self) -> Option<i64> {
        self.user.as_ref().map(|user| user.hostel_id)
    }

    fn refresh(&mut self) {
        let Some(hostel_id) = self.hostel_id() else {
            return;
        };
        if let Ok(Some(profile)) = runtime().block_on(self.desk.hostel_profile(hostel_id)) {
            self.hostel_name.clone_from(&profile.name);
            self.hostel_address = profile.address.unwrap_or_default();
            self.hostel_phone = profile.phone.unwrap_or_default();
            self.currency.clone_from(&profile.currency);
            self.logo_path = profile.branding_logo_path;
        }
        let today = chrono::Local::now().date_naive();
        self.stats = runtime()
            .block_on(
                self.desk
                    .dashboard_stats(GetDashboardStatsRequest { hostel_id, today }),
            )
            .ok();
        self.activity = runtime()
            .block_on(self.desk.recent_activity(ListRecentActivityRequest {
                hostel_id,
                limit: 12,
            }))
            .unwrap_or_default();
        self.rooms = runtime()
            .block_on(self.desk.rooms(hostel_id))
            .unwrap_or_default();
        self.residents = runtime()
            .block_on(self.desk.list_residents(ListResidentsRequest { hostel_id }))
            .unwrap_or_default();
        self.allocations = runtime()
            .block_on(
                self.desk
                    .active_allocations(ListActiveAllocationsRequest { hostel_id }),
            )
            .unwrap_or_default();
        self.payments = runtime()
            .block_on(self.desk.list_payments(db::ListPaymentsFilter {
                hostel_id,
                from: None,
                to: None,
                resident_id: None,
                method: None,
            }))
            .unwrap_or_default();
        if let Ok(prefs) = runtime().block_on(
            self.desk
                .app_preferences(GetAppPreferencesRequest { hostel_id }),
        ) {
            self.date_format = prefs.date_format;
            self.receipt_footer = prefs.receipt_footer.unwrap_or_default();
        }
        if self.user.as_ref().is_some_and(|user| user.role == "admin") {
            self.users = runtime()
                .block_on(self.desk.list_users(ListUsersRequest { hostel_id }))
                .unwrap_or_default();
            self.backups = runtime()
                .block_on(self.desk.list_backups())
                .unwrap_or_default();
        }
        if let Ok(cloud) = runtime().block_on(self.desk.cloud_status()) {
            self.cloud_summary = match &cloud.server_url {
                Some(url) => format!(
                    "{url} · prefix {} · cursor {} · {} pending",
                    cloud.device_prefix.as_deref().unwrap_or("—"),
                    cloud.cursor,
                    cloud.pending
                ),
                None => "This computer is not enrolled.".to_string(),
            };
            self.cloud_conflicts = cloud
                .issues
                .into_iter()
                .map(|issue| {
                    let kind = if issue.rejected {
                        "Rejected"
                    } else {
                        "Warning"
                    };
                    format!("{kind} · {} · {}", issue.command_type, issue.message)
                })
                .collect();
        }
        self.loaded = true;
    }

    fn note(&mut self, result: Result<impl std::fmt::Display, String>, ok: &str) {
        match result {
            Ok(value) => {
                let extra = value.to_string();
                self.status = if extra.is_empty() {
                    ok.to_string()
                } else {
                    format!("{ok} {extra}")
                };
            }
            Err(e) => self.status = e,
        }
    }
}

impl eframe::App for HostelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(RichText::new(&self.status).small());
        });

        match self.gate {
            Gate::Boot => {
                self.boot();
            }
            Gate::Lock => lock_screen(self, ui),
            Gate::Onboarding => onboarding(self, ui),
            Gate::SignIn => sign_in(self, ui),
            Gate::Ready => shell(self, ui),
        }
    }
}

fn lock_screen(app: &mut HostelApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("HostelDesk");
            ui.label("Choose the license file for this computer.");
            if ui.button("Select license file").clicked() {
                match app.desk.activate_license_from_file() {
                    Ok(payload) => {
                        app.status = format!(
                            "Licensed {} until {}",
                            payload.hostel_name, payload.expiry_date
                        );
                        app.boot();
                    }
                    Err(e) => app.status = e,
                }
            }
            ui.separator();
            ui.label("Or sign in to your organization.");
            labeled(ui, "Server", &mut app.cloud_server);
            labeled(ui, "Organization", &mut app.cloud_org);
            labeled(ui, "Email", &mut app.cloud_email);
            labeled_password(ui, "Password", &mut app.cloud_password);
            ui.horizontal(|ui| {
                if ui.button("Create account").clicked() {
                    connect_cloud(app, true);
                }
                if ui.button("Sign in").clicked() {
                    connect_cloud(app, false);
                }
            });
        });
    });
}

fn connect_cloud(app: &mut HostelApp, create_account: bool) {
    let result = runtime().block_on(app.desk.connect_cloud(
        &app.cloud_server,
        &app.cloud_email,
        &app.cloud_password,
        create_account,
        &app.cloud_org,
    ));
    match result {
        Ok(()) => {
            app.cloud_password.clear();
            if let Err(error) = runtime().block_on(app.desk.sync_once()) {
                app.status = format!("Signed in. Sync will retry. {error}");
            }
            app.boot();
        }
        Err(error) => app.status = error,
    }
}

fn onboarding(app: &mut HostelApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading("Set up your hostel");
        ui.label("Hostel profile");
        labeled(ui, "Name", &mut app.onboard_name);
        labeled(ui, "Address", &mut app.onboard_address);
        labeled(ui, "Phone", &mut app.onboard_phone);
        ui.separator();
        ui.label("First administrator");
        labeled(ui, "Full name", &mut app.onboard_admin);
        labeled(ui, "Email", &mut app.onboard_email);
        labeled_password(ui, "Password", &mut app.onboard_password);
        if ui.button("Create hostel").clicked() {
            let request = CompleteOnboardingRequest {
                hostel_name: app.onboard_name.clone(),
                address: blank(&app.onboard_address),
                phone: blank(&app.onboard_phone),
                currency: Some("UGX".to_string()),
                branding_logo_path: None,
                admin_name: app.onboard_admin.clone(),
                email: app.onboard_email.clone(),
                password: app.onboard_password.clone(),
            };
            match runtime().block_on(app.desk.complete_onboarding(request)) {
                Ok(user) => app.enter(user),
                Err(e) => app.status = e,
            }
        }
    });
}

fn sign_in(app: &mut HostelApp, ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading("Sign in");
        labeled(ui, "Email", &mut app.email);
        labeled_password(ui, "Password", &mut app.password);
        if ui.button("Sign in").clicked() {
            let request = LoginRequest {
                email: app.email.clone(),
                password: app.password.clone(),
            };
            match runtime().block_on(app.desk.login(request)) {
                Ok(user) => {
                    app.password.clear();
                    app.enter(user);
                }
                Err(e) => app.status = e,
            }
        }
    });
}

fn shell(app: &mut HostelApp, ui: &mut egui::Ui) {
    maybe_sync(app, ui);
    if !app.loaded {
        app.refresh();
    }
    egui::Panel::left("nav").show(ui, |ui| {
        ui.heading("HostelDesk");
        ui.label(&app.hostel_name);
        nav_button(ui, app, "Dashboard", Nav::Dashboard);
        nav_button(ui, app, "Rooms", Nav::Rooms);
        nav_button(ui, app, "Residents", Nav::Residents);
        nav_button(ui, app, "Payments", Nav::Payments);
        nav_button(ui, app, "Settings", Nav::Settings);
        ui.separator();
        if let Some(user) = &app.user {
            ui.label(&user.full_name);
            ui.label(user.role.as_str());
        }
        if ui.button("Sign out").clicked() {
            app.desk.logout();
            app.user = None;
            app.gate = Gate::SignIn;
            app.loaded = false;
        }
        ui.label(format!("v{}", app.desk.app_version()));
    });
    egui::CentralPanel::default().show(ui, |ui| match app.nav {
        Nav::Dashboard => dashboard(app, ui),
        Nav::Rooms => rooms(app, ui),
        Nav::Residents => residents(app, ui),
        Nav::Payments => payments(app, ui),
        Nav::Settings => settings(app, ui),
    });
}

fn maybe_sync(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(30));
    if !app.desk.cloud_enrolled() {
        return;
    }
    let due = app
        .last_sync
        .map(|at| at.elapsed() >= std::time::Duration::from_secs(30))
        .unwrap_or(true);
    if !due {
        return;
    }
    app.last_sync = Some(std::time::Instant::now());
    match runtime().block_on(app.desk.sync_once()) {
        Ok(report) => {
            if !report.summary.is_empty() {
                app.status = report.summary;
                app.loaded = false;
            }
        }
        Err(error) => app.status = format!("Sync failed: {error}"),
    }
}

fn nav_button(ui: &mut egui::Ui, app: &mut HostelApp, label: &str, nav: Nav) {
    if ui.selectable_label(app.nav == nav, label).clicked() {
        app.nav = nav;
    }
}

fn dashboard(app: &HostelApp, ui: &mut egui::Ui) {
    ui.heading("Dashboard");
    if let Some(stats) = &app.stats {
        ui.label(format!(
            "Rooms {} · occupied {} · vacant {} · maintenance {}",
            stats.total_rooms, stats.occupied, stats.vacant, stats.maintenance
        ));
        ui.label(format!("Check-ins today: {}", stats.check_ins_today));
        ui.label(format!(
            "Revenue today: {} {}",
            app.currency, stats.revenue_today
        ));
        ui.label(format!(
            "Revenue this month: {} {}",
            app.currency, stats.revenue_month
        ));
        ui.label(format!(
            "Outstanding: {} {}",
            app.currency, stats.outstanding_total
        ));
    }
    ui.separator();
    ui.label("Recent activity");
    for item in &app.activity {
        let amount = item
            .amount
            .as_ref()
            .map(|amount| format!(" {amount}"))
            .unwrap_or_default();
        ui.label(format!(
            "{} {} · {} · room {}{amount}",
            item.at.format("%Y-%m-%d %H:%M"),
            item.kind,
            item.resident_name,
            item.room_number
        ));
    }
}

fn rooms(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Rooms");
    egui::Grid::new("rooms").striped(true).show(ui, |ui| {
        ui.label("Number");
        ui.label("Type");
        ui.label("Beds");
        ui.label("Occupied");
        ui.label("Semester");
        ui.label("Recess");
        ui.label("Status");
        ui.end_row();
        for room in &app.rooms {
            ui.label(&room.room_number);
            ui.label(&room.room_type);
            ui.label(room.capacity.to_string());
            ui.label(room.occupied_beds.to_string());
            ui.label(&room.semester_rate);
            ui.label(&room.recess_rate);
            ui.label(&room.status);
            ui.end_row();
        }
    });
    ui.separator();
    ui.label("Add a room");
    labeled(ui, "Number", &mut app.room_number);
    combo(
        ui,
        "Type",
        &mut app.room_type,
        &["single", "double", "dorm"],
    );
    labeled(ui, "Capacity", &mut app.room_capacity);
    labeled(ui, "Floor", &mut app.room_floor);
    labeled(ui, "Semester rate", &mut app.semester_rate);
    labeled(ui, "Recess rate", &mut app.recess_rate);
    if ui.button("Add room").clicked() {
        let Some(hostel_id) = app.hostel_id() else {
            return;
        };
        let capacity = app.room_capacity.parse().unwrap_or(0);
        let floor = blank(&app.room_floor).and_then(|value| value.parse().ok());
        let request = AddRoomRequest {
            hostel_id,
            room_number: app.room_number.clone(),
            room_type: app.room_type.clone(),
            capacity,
            floor,
            semester_rate: app.semester_rate.clone(),
            recess_rate: app.recess_rate.clone(),
        };
        let result = runtime().block_on(app.desk.add_room(request));
        app.note(result, "Room added.");
        app.refresh();
    }
}

fn residents(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Residents");
    labeled(ui, "Search", &mut app.resident_query);
    let query = app.resident_query.to_lowercase();
    let rows: Vec<_> = app
        .residents
        .iter()
        .filter(|resident| {
            query.is_empty()
                || resident.full_name.to_lowercase().contains(&query)
                || resident
                    .phone
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query)
        })
        .cloned()
        .collect();
    for resident in rows {
        let label = format!(
            "{} · {} · {} · balance {}",
            resident.full_name,
            resident.room_number.as_deref().unwrap_or("—"),
            resident.status,
            resident.balance
        );
        if ui
            .selectable_label(app.selected_resident == Some(resident.id), label)
            .clicked()
        {
            app.selected_resident = Some(resident.id);
            load_resident(app, resident.id);
        }
    }
    ui.separator();
    if app.selected_resident.is_some() {
        ui.label(format!("Balance {}", app.detail_balance));
        labeled(ui, "Name", &mut app.detail_name);
        labeled(ui, "Phone", &mut app.detail_phone);
        labeled(ui, "Email", &mut app.detail_email);
        labeled(ui, "ID number", &mut app.detail_id);
        labeled(ui, "Next of kin", &mut app.detail_kin);
        labeled(ui, "Kin phone", &mut app.detail_kin_phone);
        if ui.button("Save resident").clicked() {
            save_resident(app);
        }
        if app.detail_allocation.is_some() && ui.button("Check out").clicked() {
            check_out(app);
        }
    }
    ui.separator();
    ui.label("Check in");
    labeled(ui, "Full name", &mut app.checkin_name);
    labeled(ui, "Phone", &mut app.checkin_phone);
    labeled(ui, "Email", &mut app.checkin_email);
    labeled(ui, "ID number", &mut app.checkin_id);
    labeled(ui, "Next of kin", &mut app.checkin_kin);
    labeled(ui, "Kin phone", &mut app.checkin_kin_phone);
    room_picker(ui, &app.rooms, &mut app.checkin_room);
    labeled(ui, "Rate agreed", &mut app.checkin_rate);
    combo(
        ui,
        "Term",
        &mut app.checkin_term,
        &["semester_1", "semester_2", "recess"],
    );
    labeled(ui, "Academic year", &mut app.checkin_year);
    if ui.button("Check in").clicked() {
        check_in(app);
    }
}

fn load_resident(app: &mut HostelApp, resident_id: i64) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    match runtime().block_on(app.desk.resident_detail(GetResidentDetailRequest {
        hostel_id,
        resident_id,
    })) {
        Ok(detail) => {
            app.detail_name.clone_from(&detail.resident.full_name);
            app.detail_phone = detail.resident.phone.unwrap_or_default();
            app.detail_email = detail.resident.email.unwrap_or_default();
            app.detail_id = detail.resident.id_number.unwrap_or_default();
            app.detail_kin = detail.resident.next_of_kin.unwrap_or_default();
            app.detail_kin_phone = detail.resident.next_of_kin_phone.unwrap_or_default();
            app.detail_balance = detail.balance;
            app.detail_allocation = detail
                .allocations
                .iter()
                .find(|allocation| allocation.status == "active")
                .map(|allocation| allocation.id);
        }
        Err(e) => app.status = e,
    }
}

fn save_resident(app: &mut HostelApp) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    let Some(resident_id) = app.selected_resident else {
        return;
    };
    let result = runtime().block_on(app.desk.update_resident(UpdateResidentRequest {
        hostel_id,
        resident_id,
        full_name: app.detail_name.clone(),
        phone: blank(&app.detail_phone),
        email: blank(&app.detail_email),
        id_number: blank(&app.detail_id),
        next_of_kin: blank(&app.detail_kin),
        next_of_kin_phone: blank(&app.detail_kin_phone),
    }));
    app.note(result.map(|()| String::new()), "Resident saved.");
    app.refresh();
}

fn check_out(app: &mut HostelApp) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    let Some(allocation_id) = app.detail_allocation else {
        return;
    };
    let result = runtime().block_on(app.desk.check_out(CheckOutRequest {
        hostel_id,
        allocation_id,
    }));
    app.note(result.map(|()| String::new()), "Checked out.");
    app.detail_allocation = None;
    app.refresh();
}

fn check_in(app: &mut HostelApp) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    let Some(room_id) = app.checkin_room else {
        app.status = "Choose a room.".to_string();
        return;
    };
    let result = runtime().block_on(app.desk.check_in(CheckInRequest {
        hostel_id,
        room_id,
        resident_id: None,
        full_name: blank(&app.checkin_name),
        phone: blank(&app.checkin_phone),
        email: blank(&app.checkin_email),
        id_number: blank(&app.checkin_id),
        next_of_kin: blank(&app.checkin_kin),
        next_of_kin_phone: blank(&app.checkin_kin_phone),
        photo_path: None,
        rate_agreed: app.checkin_rate.clone(),
        term: app.checkin_term.clone(),
        academic_year: app.checkin_year.clone(),
    }));
    app.note(result, "Checked in. Allocation");
    app.refresh();
}

fn payments(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Payments");
    for payment in &app.payments {
        ui.label(format!(
            "{} · {} · {} {} · {} · receipt {}",
            payment.payment_date.format("%Y-%m-%d"),
            payment.resident_name,
            app.currency,
            payment.amount,
            payment.payment_method,
            payment.receipt_number.as_deref().unwrap_or("—")
        ));
    }
    ui.separator();
    ui.label("Record a payment");
    allocation_picker(ui, &app.allocations, &mut app.pay_allocation);
    labeled(ui, "Amount", &mut app.pay_amount);
    combo(
        ui,
        "Method",
        &mut app.pay_method,
        &["cash", "mobile_money", "bank_transfer", "card"],
    );
    labeled(ui, "Notes", &mut app.pay_notes);
    if ui.button("Record payment and open receipt").clicked() {
        record_payment(app);
    }
}

fn record_payment(app: &mut HostelApp) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    let Some(allocation_id) = app.pay_allocation else {
        app.status = "Choose an active stay.".to_string();
        return;
    };
    let Some(stay) = app
        .allocations
        .iter()
        .find(|allocation| allocation.allocation_id == allocation_id)
        .cloned()
    else {
        app.status = "That stay is no longer active.".to_string();
        return;
    };
    let recorded = runtime().block_on(app.desk.record_payment_and_generate_receipt(
        RecordPaymentRequest {
            hostel_id,
            allocation_id,
            resident_id: stay.resident_id,
            amount: app.pay_amount.clone(),
            payment_method: app.pay_method.clone(),
            period_start: None,
            period_end: None,
            notes: blank(&app.pay_notes),
        },
    ));
    let (_payment_id, receipt_number) = match recorded {
        Ok(pair) => pair,
        Err(e) => {
            app.status = e;
            return;
        }
    };
    let opened = runtime().block_on(app.desk.generate_and_open_receipt(GenerateReceiptRequest {
        hostel_id,
        hostel_name: app.hostel_name.clone(),
        hostel_address: blank(&app.hostel_address),
        hostel_phone: blank(&app.hostel_phone),
        receipt_number: receipt_number.clone(),
        resident_name: stay.resident_name,
        room_number: stay.room_number,
        amount: app.pay_amount.clone(),
        date: chrono::Local::now().date_naive(),
        balance: stay.balance,
        payment_method: app.pay_method.clone(),
        notes: blank(&app.pay_notes),
        currency_code: app.currency.clone(),
        footer_text: blank(&app.receipt_footer),
        logo_path: app.logo_path.clone(),
    }));
    match opened {
        Ok(path) => app.status = format!("Receipt {receipt_number} opened: {path}"),
        Err(e) => app.status = format!("Payment recorded. Receipt file failed: {e}"),
    }
    app.refresh();
}

fn settings(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.heading("Settings");
    ui.horizontal(|ui| {
        tab(ui, app, "Business", SettingsTab::Business);
        tab(ui, app, "Team", SettingsTab::Team);
        tab(ui, app, "Preferences", SettingsTab::Preferences);
        tab(ui, app, "Backup", SettingsTab::Backup);
        tab(ui, app, "Account", SettingsTab::Account);
        tab(ui, app, "Cloud", SettingsTab::Cloud);
    });
    ui.separator();
    match app.settings {
        SettingsTab::Business => business(app, ui),
        SettingsTab::Team => team(app, ui),
        SettingsTab::Preferences => preferences(app, ui),
        SettingsTab::Backup => backup(app, ui),
        SettingsTab::Account => account(app, ui),
        SettingsTab::Cloud => cloud(app, ui),
    }
}

fn cloud(app: &mut HostelApp, ui: &mut egui::Ui) {
    ui.label(&app.cloud_summary);
    if ui.button("Sync now").clicked() {
        app.last_sync = None;
        match runtime().block_on(app.desk.sync_once()) {
            Ok(report) => {
                app.status = if report.summary.is_empty() {
                    "Already up to date.".to_string()
                } else {
                    report.summary
                };
                app.loaded = false;
            }
            Err(error) => app.status = format!("Sync failed: {error}"),
        }
    }
    if !app.cloud_conflicts.is_empty() {
        ui.separator();
        ui.label("Needs attention");
        for conflict in &app.cloud_conflicts {
            ui.label(conflict);
        }
    }
}

fn tab(ui: &mut egui::Ui, app: &mut HostelApp, label: &str, tab: SettingsTab) {
    if ui.selectable_label(app.settings == tab, label).clicked() {
        app.settings = tab;
    }
}

fn business(app: &mut HostelApp, ui: &mut egui::Ui) {
    labeled(ui, "Name", &mut app.hostel_name);
    labeled(ui, "Address", &mut app.hostel_address);
    labeled(ui, "Phone", &mut app.hostel_phone);
    labeled(ui, "Currency", &mut app.currency);
    if let Some(path) = &app.logo_path {
        ui.label(format!("Logo: {path}"));
    }
    if ui.button("Choose logo").clicked() {
        upload_logo(app);
    }
    if ui.button("Save business profile").clicked() {
        save_business(app);
    }
    if let Some(license) = app.desk.license_status() {
        ui.label(format!(
            "License {} · {} · expires {}",
            license.tier, license.hostel_name, license.expiry_date
        ));
    }
}

fn save_business(app: &mut HostelApp) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    let result = runtime().block_on(app.desk.update_hostel(UpdateHostelRequest {
        hostel_id,
        name: app.hostel_name.clone(),
        address: blank(&app.hostel_address),
        phone: blank(&app.hostel_phone),
        currency: app.currency.trim().to_string(),
        branding_logo_path: app.logo_path.clone(),
    }));
    app.note(result.map(|hostel| hostel.name), "Saved");
    app.refresh();
}

fn upload_logo(app: &mut HostelApp) {
    let Some(hostel_id) = app.hostel_id() else {
        return;
    };
    let Some(path) = rfd::FileDialog::new()
        .set_title("Hostel logo")
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
    else {
        return;
    };
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(e) => {
            app.status = format!("could not read logo: {e}");
            return;
        }
    };
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("logo.png")
        .to_string();
    match runtime().block_on(app.desk.upload_hostel_logo(UploadHostelLogoRequest {
        hostel_id,
        data,
        filename,
    })) {
        Ok(stored) => {
            app.logo_path = Some(stored);
            save_business(app);
        }
        Err(e) => app.status = e,
    }
}

fn team(app: &mut HostelApp, ui: &mut egui::Ui) {
    for user in &app.users {
        ui.label(format!(
            "{} · {} · {}",
            user.full_name, user.email, user.role
        ));
        if ui.button(format!("Remove {}", user.email)).clicked() {
            let result =
                runtime().block_on(app.desk.delete_user(DeleteUserRequest { user_id: user.id }));
            app.note(result.map(|()| String::new()), "Removed.");
            app.refresh();
            return;
        }
    }
    ui.separator();
    ui.label("Add staff");
    labeled(ui, "Name", &mut app.new_user_name);
    labeled(ui, "Email", &mut app.new_user_email);
    labeled_password(ui, "Password", &mut app.new_user_password);
    combo(ui, "Role", &mut app.new_user_role, &["admin", "manager"]);
    if ui.button("Create staff account").clicked() {
        let Some(hostel_id) = app.hostel_id() else {
            return;
        };
        let result = runtime().block_on(app.desk.create_user(CreateUserRequest {
            hostel_id,
            full_name: app.new_user_name.clone(),
            email: app.new_user_email.clone(),
            password: app.new_user_password.clone(),
            role: app.new_user_role.clone(),
        }));
        app.note(result, "Staff id");
        app.new_user_password.clear();
        app.refresh();
    }
}

fn preferences(app: &mut HostelApp, ui: &mut egui::Ui) {
    combo(
        ui,
        "Date format",
        &mut app.date_format,
        &["dd/MM/yyyy", "MM/dd/yyyy", "yyyy-MM-dd"],
    );
    labeled(ui, "Receipt footer", &mut app.receipt_footer);
    if ui.button("Save preferences").clicked() {
        let Some(hostel_id) = app.hostel_id() else {
            return;
        };
        let result = runtime().block_on(app.desk.update_app_preferences(
            UpsertAppPreferencesRequest {
                hostel_id,
                date_format: app.date_format.clone(),
                language: "en".to_string(),
                receipt_footer: blank(&app.receipt_footer),
            },
        ));
        app.note(result.map(|_| String::new()), "Preferences saved.");
    }
}

fn backup(app: &mut HostelApp, ui: &mut egui::Ui) {
    if ui.button("Back up database").clicked() {
        let result = runtime().block_on(app.desk.backup_database());
        app.note(result, "Backup written:");
        app.refresh();
    }
    for path in app.backups.clone() {
        ui.label(&path);
        if ui.button(format!("Restore {path}")).clicked() {
            let result = runtime().block_on(
                app.desk
                    .restore_backup(RestoreBackupRequest { backup_path: path }),
            );
            if let Err(e) = result {
                app.status = e;
            }
            return;
        }
    }
}

fn account(app: &mut HostelApp, ui: &mut egui::Ui) {
    labeled(ui, "Name", &mut app.account_name);
    labeled(ui, "Email", &mut app.account_email);
    labeled_password(ui, "New password", &mut app.account_password);
    if ui.button("Save account").clicked() {
        let Some(user) = app.user.clone() else {
            return;
        };
        let password = blank(&app.account_password);
        let result = runtime().block_on(app.desk.update_user(UpdateUserRequest {
            user_id: user.id,
            full_name: app.account_name.clone(),
            email: app.account_email.clone(),
            role: user.role.clone(),
            password,
        }));
        match result {
            Ok(updated) => {
                app.account_password.clear();
                app.user = Some(updated);
                app.status = "Account saved.".to_string();
            }
            Err(e) => app.status = e,
        }
    }
}

fn room_picker(ui: &mut egui::Ui, rooms: &[db::Room], selected: &mut Option<i64>) {
    let current = rooms
        .iter()
        .find(|room| Some(room.id) == *selected)
        .map(|room| room.room_number.as_str())
        .unwrap_or("Choose a room");
    egui::ComboBox::from_label("Room")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for room in rooms {
                ui.selectable_value(selected, Some(room.id), &room.room_number);
            }
        });
}

fn allocation_picker(ui: &mut egui::Ui, rows: &[db::ActiveAllocation], selected: &mut Option<i64>) {
    let current = rows
        .iter()
        .find(|row| Some(row.allocation_id) == *selected)
        .map(|row| format!("{} · room {}", row.resident_name, row.room_number))
        .unwrap_or_else(|| "Choose a stay".to_string());
    egui::ComboBox::from_label("Stay")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for row in rows {
                ui.selectable_value(
                    selected,
                    Some(row.allocation_id),
                    format!("{} · {}", row.resident_name, row.room_number),
                );
            }
        });
}

fn labeled(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
    });
}

fn labeled_password(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).password(true));
    });
}

fn combo(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) {
    egui::ComboBox::from_label(label)
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for option in options {
                ui.selectable_value(value, (*option).to_string(), *option);
            }
        });
}

fn blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn default_academic_year() -> String {
    let today = chrono::Local::now().date_naive();
    let year = today.year();
    format!("{year}/{}", year + 1)
}
