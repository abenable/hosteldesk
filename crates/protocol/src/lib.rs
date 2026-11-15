//! Sync commands shared by the desktop cache and the hosted server.
//!
//! Money crosses the wire as decimal strings. [`Ledger::apply`] checks every
//! amount and rate with [`validate_money`], which matches the desk's SQLite
//! CHECK constraints, so a row the server accepts can always be stored on a desk.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

mod wire;
pub use wire::*;

/// One idempotent change a desk wants the server to accept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandEnvelope {
    /// Client-generated id. A retry with the same key is not a second payment.
    pub idempotency_key: String,
    /// The change itself.
    pub command: SyncCommand,
}

/// Ledger mutations. Creates use `base_revision = 0`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncCommand {
    /// Create or edit the hostel profile.
    UpsertHostel {
        /// Stable hostel id.
        public_id: String,
        /// Revision the edit was based on. `0` creates the hostel.
        base_revision: i64,
        /// Display name.
        name: String,
        /// Optional address.
        address: Option<String>,
        /// Optional phone.
        phone: Option<String>,
        /// ISO currency code.
        currency: String,
    },
    /// Create or edit a room.
    UpsertRoom {
        /// Stable room id.
        public_id: String,
        /// Hostel that owns the room.
        hostel_public_id: String,
        /// Revision the edit was based on. `0` creates the room.
        base_revision: i64,
        /// Room label unique within the hostel.
        room_number: String,
        /// `single`, `double`, or `dorm`.
        room_type: String,
        /// Bed count.
        capacity: i32,
        /// Optional floor.
        floor: Option<i32>,
        /// Semester rate, decimal string.
        semester_rate: String,
        /// Recess rate, decimal string.
        recess_rate: String,
        /// `vacant`, `occupied`, or `maintenance`.
        status: String,
    },
    /// Create or edit a resident.
    UpsertResident {
        /// Stable resident id.
        public_id: String,
        /// Hostel that owns the resident.
        hostel_public_id: String,
        /// Revision the edit was based on. `0` creates the resident.
        base_revision: i64,
        /// Full name.
        full_name: String,
        /// Optional phone.
        phone: Option<String>,
        /// Optional email.
        email: Option<String>,
        /// Optional id document number.
        id_number: Option<String>,
        /// Optional next of kin.
        next_of_kin: Option<String>,
        /// Optional next-of-kin phone.
        next_of_kin_phone: Option<String>,
    },
    /// Create or edit receipt preferences.
    UpsertPreferences {
        /// Hostel the preferences belong to.
        hostel_public_id: String,
        /// Revision the edit was based on. `0` creates the row.
        base_revision: i64,
        /// One of the supported date formats.
        date_format: String,
        /// Language code. Currently `en`.
        language: String,
        /// Optional receipt footer.
        receipt_footer: Option<String>,
    },
    /// Check a resident into a room. The allocation id is chosen by the desk.
    CheckIn {
        /// Stable allocation id.
        allocation_public_id: String,
        /// Hostel.
        hostel_public_id: String,
        /// Resident id. Created when the server has not seen it yet.
        resident_public_id: String,
        /// Room id.
        room_public_id: String,
        /// Resident name, used when the resident is new.
        full_name: String,
        /// Optional phone.
        phone: Option<String>,
        /// Optional email.
        email: Option<String>,
        /// Optional id number.
        id_number: Option<String>,
        /// Optional next of kin.
        next_of_kin: Option<String>,
        /// Optional next-of-kin phone.
        next_of_kin_phone: Option<String>,
        /// Agreed rate, decimal string.
        rate_agreed: String,
        /// `semester_1`, `semester_2`, or `recess`.
        term: String,
        /// Academic year label such as `2026/2027`.
        academic_year: String,
        /// When the desk checked the resident in, [`TIMESTAMP_FORMAT`] (UTC).
        checked_in_at: String,
    },
    /// Check an allocation out.
    CheckOut {
        /// Hostel.
        hostel_public_id: String,
        /// Allocation to close.
        allocation_public_id: String,
        /// When the desk checked the resident out, [`TIMESTAMP_FORMAT`] (UTC).
        checked_out_at: String,
    },
    /// Record a payment. The receipt number was already printed on the desk.
    RecordPayment {
        /// Stable payment id.
        payment_public_id: String,
        /// Stable receipt id.
        receipt_public_id: String,
        /// Hostel.
        hostel_public_id: String,
        /// Allocation the payment applies to.
        allocation_public_id: String,
        /// Resident who paid.
        resident_public_id: String,
        /// Amount, decimal string.
        amount: String,
        /// `cash`, `mobile_money`, `bank_transfer`, or `card`.
        payment_method: String,
        /// Receipt number, including a device prefix when the desk is enrolled.
        receipt_number: String,
        /// When the desk took the money, `YYYY-MM-DDTHH:MM:SSZ` (UTC).
        payment_date: String,
        /// Optional note.
        notes: Option<String>,
    },
}

impl SyncCommand {
    /// Hostel this command writes to.
    pub fn hostel_public_id(&self) -> &str {
        match self {
            SyncCommand::UpsertHostel { public_id, .. } => public_id,
            SyncCommand::UpsertRoom {
                hostel_public_id, ..
            }
            | SyncCommand::UpsertResident {
                hostel_public_id, ..
            }
            | SyncCommand::UpsertPreferences {
                hostel_public_id, ..
            }
            | SyncCommand::CheckIn {
                hostel_public_id, ..
            }
            | SyncCommand::CheckOut {
                hostel_public_id, ..
            }
            | SyncCommand::RecordPayment {
                hostel_public_id, ..
            } => hostel_public_id,
        }
    }
}

/// Validate a money string the way the desk's SQLite CHECK constraints do:
/// ASCII digits with at most one `.`, strictly greater than zero.
///
/// `rust_decimal` alone would also accept `+5`, `1_000`, and surrounding
/// whitespace, which the desk's schema rejects, so a pulled row carrying one
/// would fail every later pull.
pub fn validate_money(field: &str, amount: &str) -> Result<rust_decimal::Decimal, String> {
    if amount.is_empty() || !amount.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(format!("{field} must be a plain decimal amount"));
    }
    hosteldesk_core::parse_positive_money(amount).map_err(|e| format!("{field}: {e}"))
}

/// Format of [`PaymentSnap::payment_date`] and `RecordPayment::payment_date`
/// (the desk's `payments.payment_date` column default).
pub const PAYMENT_DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

/// Format of check-in and check-out times (SQLite `CURRENT_TIMESTAMP`, UTC).
pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

fn valid_time(value: &str, format: &str) -> bool {
    chrono::NaiveDateTime::parse_from_str(value, format).is_ok()
}

/// Ids a batch of commands reads, so a store can load only those rows into a
/// [`Ledger`] instead of the whole hostel.
///
/// The store must load the hostel, its preferences, every allocation, resident,
/// and payment listed here, the rooms listed here plus the rooms of those
/// allocations plus any room whose number is in `room_numbers`, every *active*
/// allocation in any of those rooms (for bed counts), receipts whose number is
/// in `receipt_numbers`, and which of `idempotency_keys` were already applied.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BatchRefs {
    /// Room ids named by the commands.
    pub rooms: HashSet<String>,
    /// Resident ids named by the commands.
    pub residents: HashSet<String>,
    /// Allocation ids named by the commands.
    pub allocations: HashSet<String>,
    /// Payment ids named by the commands.
    pub payments: HashSet<String>,
    /// Receipt numbers named by the commands.
    pub receipt_numbers: HashSet<String>,
    /// Room numbers named by room upserts, for the uniqueness check.
    pub room_numbers: HashSet<String>,
    /// Idempotency keys in the batch.
    pub idempotency_keys: HashSet<String>,
}

impl BatchRefs {
    /// Collect the ids `commands` refer to.
    pub fn of(commands: &[CommandEnvelope]) -> Self {
        let mut refs = Self::default();
        for envelope in commands {
            refs.idempotency_keys
                .insert(envelope.idempotency_key.clone());
            match &envelope.command {
                SyncCommand::UpsertHostel { .. } | SyncCommand::UpsertPreferences { .. } => {}
                SyncCommand::UpsertRoom {
                    public_id,
                    room_number,
                    ..
                } => {
                    refs.rooms.insert(public_id.clone());
                    refs.room_numbers.insert(room_number.clone());
                }
                SyncCommand::UpsertResident { public_id, .. } => {
                    refs.residents.insert(public_id.clone());
                }
                SyncCommand::CheckIn {
                    allocation_public_id,
                    resident_public_id,
                    room_public_id,
                    ..
                } => {
                    refs.allocations.insert(allocation_public_id.clone());
                    refs.residents.insert(resident_public_id.clone());
                    refs.rooms.insert(room_public_id.clone());
                }
                SyncCommand::CheckOut {
                    allocation_public_id,
                    ..
                } => {
                    refs.allocations.insert(allocation_public_id.clone());
                }
                SyncCommand::RecordPayment {
                    payment_public_id,
                    allocation_public_id,
                    receipt_number,
                    ..
                } => {
                    refs.payments.insert(payment_public_id.clone());
                    refs.allocations.insert(allocation_public_id.clone());
                    refs.receipt_numbers.insert(receipt_number.clone());
                }
            }
        }
        refs
    }
}

/// A row the server accepted, or a row the desk should apply from a pull.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "entity", rename_all = "snake_case")]
pub enum Entity {
    /// Hostel profile.
    Hostel(HostelSnap),
    /// Room.
    Room(RoomSnap),
    /// Resident.
    Resident(ResidentSnap),
    /// Stay.
    Allocation(AllocationSnap),
    /// Payment.
    Payment(PaymentSnap),
    /// Receipt.
    Receipt(ReceiptSnap),
    /// Preferences.
    Preferences(PrefsSnap),
}

/// Hostel profile snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostelSnap {
    /// Stable id.
    pub public_id: String,
    /// Current revision.
    pub revision: i64,
    /// Name.
    pub name: String,
    /// Address.
    pub address: Option<String>,
    /// Phone.
    pub phone: Option<String>,
    /// Currency.
    pub currency: String,
}

/// Room snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomSnap {
    /// Stable id.
    pub public_id: String,
    /// Owning hostel.
    pub hostel_public_id: String,
    /// Current revision.
    pub revision: i64,
    /// Room number.
    pub room_number: String,
    /// Room type.
    pub room_type: String,
    /// Capacity.
    pub capacity: i32,
    /// Floor.
    pub floor: Option<i32>,
    /// Semester rate.
    pub semester_rate: String,
    /// Recess rate.
    pub recess_rate: String,
    /// Status.
    pub status: String,
}

/// Resident snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResidentSnap {
    /// Stable id.
    pub public_id: String,
    /// Owning hostel.
    pub hostel_public_id: String,
    /// Current revision.
    pub revision: i64,
    /// Name.
    pub full_name: String,
    /// Phone.
    pub phone: Option<String>,
    /// Email.
    pub email: Option<String>,
    /// Id number.
    pub id_number: Option<String>,
    /// Next of kin.
    pub next_of_kin: Option<String>,
    /// Next-of-kin phone.
    pub next_of_kin_phone: Option<String>,
}

/// Allocation snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AllocationSnap {
    /// Stable id.
    pub public_id: String,
    /// Hostel.
    pub hostel_public_id: String,
    /// Resident.
    pub resident_public_id: String,
    /// Room.
    pub room_public_id: String,
    /// `active` or `checked_out`.
    pub status: String,
    /// Agreed rate.
    pub rate_agreed: String,
    /// Term.
    pub term: String,
    /// Academic year.
    pub academic_year: String,
    /// Revision.
    pub revision: i64,
    /// When the stay began, [`TIMESTAMP_FORMAT`].
    pub checked_in_at: String,
    /// When the stay ended, [`TIMESTAMP_FORMAT`]. `None` while active.
    pub checked_out_at: Option<String>,
}

/// Payment snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentSnap {
    /// Stable id.
    pub public_id: String,
    /// Hostel.
    pub hostel_public_id: String,
    /// Allocation.
    pub allocation_public_id: String,
    /// Resident.
    pub resident_public_id: String,
    /// Amount decimal string.
    pub amount: String,
    /// Method.
    pub payment_method: String,
    /// When the desk took the money, [`PAYMENT_DATE_FORMAT`].
    pub payment_date: String,
    /// Notes.
    pub notes: Option<String>,
}

/// Receipt snapshot. The number is the one printed on the desk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptSnap {
    /// Stable id.
    pub public_id: String,
    /// Hostel.
    pub hostel_public_id: String,
    /// Payment.
    pub payment_public_id: String,
    /// Printed receipt number.
    pub receipt_number: String,
}

/// Preferences snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrefsSnap {
    /// Hostel.
    pub hostel_public_id: String,
    /// Revision.
    pub revision: i64,
    /// Date format.
    pub date_format: String,
    /// Language.
    pub language: String,
    /// Footer.
    pub receipt_footer: Option<String>,
}

/// Result of applying one command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// The command is now part of the ledger. `replay` means it was already applied.
    Ack {
        /// Rows changed by this attempt. Empty on replay.
        changed: Vec<Entity>,
        /// True when the idempotency key was already stored.
        replay: bool,
        /// Something staff should look at, such as a room booked over capacity.
        /// The command was still accepted: it records something that happened.
        warning: Option<String>,
    },
    /// The server rejected the command. The desk should show `message`.
    Conflict {
        /// Why the command was rejected.
        message: String,
    },
}

/// Persisted form of a [`Ledger`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerDump {
    /// Hostel profile.
    pub hostel: Option<HostelSnap>,
    /// Rooms.
    pub rooms: Vec<RoomSnap>,
    /// Residents.
    pub residents: Vec<ResidentSnap>,
    /// Allocations.
    pub allocations: Vec<AllocationSnap>,
    /// Payments.
    pub payments: Vec<PaymentSnap>,
    /// Receipts.
    pub receipts: Vec<ReceiptSnap>,
    /// Preferences.
    pub preferences: Option<PrefsSnap>,
    /// Idempotency keys already applied.
    pub applied: Vec<String>,
}

/// In-memory hostel ledger used by the server and by tests.
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    /// Hostel profile, once bootstrapped.
    pub hostel: Option<HostelSnap>,
    rooms: HashMap<String, RoomSnap>,
    residents: HashMap<String, ResidentSnap>,
    allocations: HashMap<String, AllocationSnap>,
    payments: HashMap<String, PaymentSnap>,
    receipts: HashMap<String, ReceiptSnap>,
    preferences: Option<PrefsSnap>,
    applied: HashSet<String>,
}

impl Ledger {
    /// Apply one command. Replaying `idempotency_key` does not insert a second row.
    pub fn apply(&mut self, idempotency_key: &str, command: &SyncCommand) -> ApplyOutcome {
        if self.applied.contains(idempotency_key) {
            return ApplyOutcome::Ack {
                changed: Vec::new(),
                replay: true,
                warning: None,
            };
        }
        let outcome = match command {
            SyncCommand::UpsertHostel {
                public_id,
                base_revision,
                name,
                address,
                phone,
                currency,
            } => self.upsert_hostel(
                public_id,
                *base_revision,
                name,
                address.clone(),
                phone.clone(),
                currency,
            ),
            SyncCommand::UpsertRoom {
                public_id,
                hostel_public_id,
                base_revision,
                room_number,
                room_type,
                capacity,
                floor,
                semester_rate,
                recess_rate,
                status,
            } => self.upsert_room(RoomSnap {
                public_id: public_id.clone(),
                hostel_public_id: hostel_public_id.clone(),
                revision: *base_revision,
                room_number: room_number.clone(),
                room_type: room_type.clone(),
                capacity: *capacity,
                floor: *floor,
                semester_rate: semester_rate.clone(),
                recess_rate: recess_rate.clone(),
                status: status.clone(),
            }),
            SyncCommand::UpsertResident {
                public_id,
                hostel_public_id,
                base_revision,
                full_name,
                phone,
                email,
                id_number,
                next_of_kin,
                next_of_kin_phone,
            } => self.upsert_resident(ResidentSnap {
                public_id: public_id.clone(),
                hostel_public_id: hostel_public_id.clone(),
                revision: *base_revision,
                full_name: full_name.clone(),
                phone: phone.clone(),
                email: email.clone(),
                id_number: id_number.clone(),
                next_of_kin: next_of_kin.clone(),
                next_of_kin_phone: next_of_kin_phone.clone(),
            }),
            SyncCommand::UpsertPreferences {
                hostel_public_id,
                base_revision,
                date_format,
                language,
                receipt_footer,
            } => self.upsert_prefs(PrefsSnap {
                hostel_public_id: hostel_public_id.clone(),
                revision: *base_revision,
                date_format: date_format.clone(),
                language: language.clone(),
                receipt_footer: receipt_footer.clone(),
            }),
            SyncCommand::CheckIn {
                allocation_public_id,
                hostel_public_id,
                resident_public_id,
                room_public_id,
                full_name,
                phone,
                email,
                id_number,
                next_of_kin,
                next_of_kin_phone,
                rate_agreed,
                term,
                academic_year,
                checked_in_at,
            } => self.check_in(CheckInInput {
                allocation_public_id,
                hostel_public_id,
                resident_public_id,
                room_public_id,
                full_name,
                phone,
                email,
                id_number,
                next_of_kin,
                next_of_kin_phone,
                rate_agreed,
                term,
                academic_year,
                checked_in_at,
            }),
            SyncCommand::CheckOut {
                hostel_public_id,
                allocation_public_id,
                checked_out_at,
            } => self.check_out(hostel_public_id, allocation_public_id, checked_out_at),
            SyncCommand::RecordPayment {
                payment_public_id,
                receipt_public_id,
                hostel_public_id,
                allocation_public_id,
                resident_public_id,
                amount,
                payment_method,
                receipt_number,
                payment_date,
                notes,
            } => self.record_payment(PaymentInput {
                payment_public_id,
                receipt_public_id,
                hostel_public_id,
                allocation_public_id,
                resident_public_id,
                amount,
                payment_method,
                receipt_number,
                payment_date,
                notes: notes.clone(),
            }),
        };
        if matches!(outcome, ApplyOutcome::Ack { .. }) {
            self.applied.insert(idempotency_key.to_string());
        }
        outcome
    }

    /// [`Self::apply`] for a push addressed to `hostel_public_id`. A command
    /// naming any other hostel is a conflict, so one push can never write into
    /// a hostel it was not authorized for.
    pub fn apply_for_hostel(
        &mut self,
        hostel_public_id: &str,
        idempotency_key: &str,
        command: &SyncCommand,
    ) -> ApplyOutcome {
        if command.hostel_public_id() != hostel_public_id {
            return conflict("command names a different hostel than the push");
        }
        self.apply(idempotency_key, command)
    }

    /// Serializable copy of the ledger, including applied idempotency keys.
    pub fn dump(&self) -> LedgerDump {
        LedgerDump {
            hostel: self.hostel.clone(),
            rooms: self.rooms.values().cloned().collect(),
            residents: self.residents.values().cloned().collect(),
            allocations: self.allocations.values().cloned().collect(),
            payments: self.payments.values().cloned().collect(),
            receipts: self.receipts.values().cloned().collect(),
            preferences: self.preferences.clone(),
            applied: self.applied.iter().cloned().collect(),
        }
    }

    /// Restore a ledger produced by [`Self::dump`].
    pub fn from_dump(dump: LedgerDump) -> Self {
        Self {
            hostel: dump.hostel,
            rooms: dump
                .rooms
                .into_iter()
                .map(|row| (row.public_id.clone(), row))
                .collect(),
            residents: dump
                .residents
                .into_iter()
                .map(|row| (row.public_id.clone(), row))
                .collect(),
            allocations: dump
                .allocations
                .into_iter()
                .map(|row| (row.public_id.clone(), row))
                .collect(),
            payments: dump
                .payments
                .into_iter()
                .map(|row| (row.public_id.clone(), row))
                .collect(),
            receipts: dump
                .receipts
                .into_iter()
                .map(|row| (row.public_id.clone(), row))
                .collect(),
            preferences: dump.preferences,
            applied: dump.applied.into_iter().collect(),
        }
    }

    /// Every current row, for a desk pulling a hostel it has not seen.
    pub fn snapshot(&self) -> Vec<Entity> {
        let mut rows = Vec::new();
        if let Some(hostel) = &self.hostel {
            rows.push(Entity::Hostel(hostel.clone()));
        }
        rows.extend(self.rooms.values().cloned().map(Entity::Room));
        rows.extend(self.residents.values().cloned().map(Entity::Resident));
        rows.extend(self.allocations.values().cloned().map(Entity::Allocation));
        rows.extend(self.payments.values().cloned().map(Entity::Payment));
        rows.extend(self.receipts.values().cloned().map(Entity::Receipt));
        if let Some(prefs) = &self.preferences {
            rows.push(Entity::Preferences(prefs.clone()));
        }
        rows
    }

    fn upsert_hostel(
        &mut self,
        public_id: &str,
        base_revision: i64,
        name: &str,
        address: Option<String>,
        phone: Option<String>,
        currency: &str,
    ) -> ApplyOutcome {
        if let Some(current) = &self.hostel {
            if current.public_id != public_id {
                return conflict("this organization already has a different hostel");
            }
            if current.revision != base_revision {
                return conflict("hostel was saved on another desk");
            }
        } else if base_revision != 0 {
            return conflict("hostel does not exist yet");
        }
        let revision = if base_revision == 0 {
            1
        } else {
            base_revision + 1
        };
        let snap = HostelSnap {
            public_id: public_id.to_string(),
            revision,
            name: name.to_string(),
            address,
            phone,
            currency: currency.to_string(),
        };
        self.hostel = Some(snap.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Hostel(snap)],
            replay: false,
            warning: None,
        }
    }

    fn upsert_room(&mut self, incoming: RoomSnap) -> ApplyOutcome {
        let base = incoming.revision;
        if let Some(current) = self.rooms.get(&incoming.public_id) {
            if current.revision != base {
                return conflict("room was saved on another desk");
            }
        } else if base != 0 {
            return conflict("room does not exist");
        }
        if !matches!(incoming.room_type.as_str(), "single" | "double" | "dorm") {
            return conflict("room_type must be single, double, or dorm");
        }
        if !matches!(
            incoming.status.as_str(),
            "vacant" | "occupied" | "maintenance"
        ) {
            return conflict("status must be vacant, occupied, or maintenance");
        }
        if incoming.capacity < 1 {
            return conflict("capacity must be at least 1");
        }
        if incoming.room_number.trim().is_empty() {
            return conflict("room_number is required");
        }
        if let Err(message) = validate_money("semester_rate", &incoming.semester_rate)
            .and_then(|_| validate_money("recess_rate", &incoming.recess_rate))
        {
            return ApplyOutcome::Conflict { message };
        }
        if self.rooms.values().any(|room| {
            room.hostel_public_id == incoming.hostel_public_id
                && room.room_number == incoming.room_number
                && room.public_id != incoming.public_id
        }) {
            return conflict("room_number is already used by another room");
        }
        let mut snap = incoming;
        snap.revision = if base == 0 { 1 } else { base + 1 };
        self.rooms.insert(snap.public_id.clone(), snap.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Room(snap)],
            replay: false,
            warning: None,
        }
    }

    fn upsert_resident(&mut self, incoming: ResidentSnap) -> ApplyOutcome {
        let base = incoming.revision;
        if let Some(current) = self.residents.get(&incoming.public_id) {
            if current.revision != base {
                return conflict("resident was saved on another desk");
            }
            if current.hostel_public_id != incoming.hostel_public_id {
                return conflict("resident belongs to another hostel");
            }
        } else if base != 0 {
            return conflict("resident does not exist");
        }
        if incoming.full_name.trim().is_empty() {
            return conflict("full_name is required");
        }
        let mut snap = incoming;
        snap.revision = if base == 0 { 1 } else { base + 1 };
        self.residents.insert(snap.public_id.clone(), snap.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Resident(snap)],
            replay: false,
            warning: None,
        }
    }

    fn upsert_prefs(&mut self, incoming: PrefsSnap) -> ApplyOutcome {
        let base = incoming.revision;
        if let Some(current) = &self.preferences {
            if current.hostel_public_id != incoming.hostel_public_id {
                return conflict("preferences belong to another hostel");
            }
            if current.revision != base {
                return conflict("preferences were saved on another desk");
            }
        } else if base != 0 {
            return conflict("preferences do not exist");
        }
        if !matches!(
            incoming.date_format.as_str(),
            "dd/MM/yyyy" | "MM/dd/yyyy" | "yyyy-MM-dd"
        ) {
            return conflict("date_format is not supported");
        }
        if incoming.language != "en" {
            return conflict("language must be en");
        }
        let mut snap = incoming;
        snap.revision = if base == 0 { 1 } else { base + 1 };
        self.preferences = Some(snap.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Preferences(snap)],
            replay: false,
            warning: None,
        }
    }

    /// A check-in records something that already happened at the front desk, so
    /// a full room or one under maintenance is accepted with a warning for
    /// staff to resolve, not rejected (rejecting it also orphaned the stay's
    /// payments on the desk that took the money).
    fn check_in(&mut self, input: CheckInInput<'_>) -> ApplyOutcome {
        if !matches!(input.term, "semester_1" | "semester_2" | "recess") {
            return conflict("term must be semester_1, semester_2, or recess");
        }
        if let Err(message) = validate_money("rate_agreed", input.rate_agreed) {
            return ApplyOutcome::Conflict { message };
        }
        if !valid_time(input.checked_in_at, TIMESTAMP_FORMAT) {
            return conflict("checked_in_at must look like 2026-11-15 09:30:00");
        }
        if self.allocations.contains_key(input.allocation_public_id) {
            return conflict("allocation id is already used");
        }
        let Some(room) = self.rooms.get(input.room_public_id).cloned() else {
            return conflict("room not found");
        };
        if room.hostel_public_id != input.hostel_public_id {
            return conflict("room belongs to another hostel");
        }
        let active = self
            .allocations
            .values()
            .filter(|allocation| {
                allocation.room_public_id == input.room_public_id && allocation.status == "active"
            })
            .count();
        let warning = if room.status == "maintenance" {
            Some(format!(
                "room {} was checked into while under maintenance",
                room.room_number
            ))
        } else if active >= room.capacity as usize {
            Some(format!(
                "room {} is over capacity ({} beds, {} residents)",
                room.room_number,
                room.capacity,
                active + 1
            ))
        } else {
            None
        };
        let resident = if let Some(existing) = self.residents.get(input.resident_public_id).cloned()
        {
            if existing.hostel_public_id != input.hostel_public_id {
                return conflict("resident belongs to another hostel");
            }
            existing
        } else {
            if input.full_name.trim().is_empty() {
                return conflict("full_name is required for a new resident");
            }
            let created = ResidentSnap {
                public_id: input.resident_public_id.to_string(),
                hostel_public_id: input.hostel_public_id.to_string(),
                revision: 1,
                full_name: input.full_name.to_string(),
                phone: input.phone.clone(),
                email: input.email.clone(),
                id_number: input.id_number.clone(),
                next_of_kin: input.next_of_kin.clone(),
                next_of_kin_phone: input.next_of_kin_phone.clone(),
            };
            self.residents
                .insert(created.public_id.clone(), created.clone());
            created
        };
        let allocation = AllocationSnap {
            public_id: input.allocation_public_id.to_string(),
            hostel_public_id: input.hostel_public_id.to_string(),
            resident_public_id: resident.public_id.clone(),
            room_public_id: room.public_id.clone(),
            status: "active".to_string(),
            rate_agreed: input.rate_agreed.to_string(),
            term: input.term.to_string(),
            academic_year: input.academic_year.to_string(),
            revision: 1,
            checked_in_at: input.checked_in_at.to_string(),
            checked_out_at: None,
        };
        self.allocations
            .insert(allocation.public_id.clone(), allocation.clone());
        // Occupancy is derived, not an edit: it never bumps the room's
        // revision, so a desk's next room edit does not conflict with it.
        let mut room = room;
        if room.status == "vacant" {
            room.status = "occupied".to_string();
            self.rooms.insert(room.public_id.clone(), room.clone());
        }
        let mut changed = vec![Entity::Resident(resident), Entity::Room(room)];
        changed.push(Entity::Allocation(allocation));
        ApplyOutcome::Ack {
            changed,
            replay: false,
            warning,
        }
    }

    fn check_out(
        &mut self,
        hostel_public_id: &str,
        allocation_public_id: &str,
        checked_out_at: &str,
    ) -> ApplyOutcome {
        if !valid_time(checked_out_at, TIMESTAMP_FORMAT) {
            return conflict("checked_out_at must look like 2026-11-15 09:30:00");
        }
        let Some(mut allocation) = self.allocations.get(allocation_public_id).cloned() else {
            return conflict("allocation not found");
        };
        if allocation.hostel_public_id != hostel_public_id {
            return conflict("allocation belongs to another hostel");
        }
        if allocation.status != "active" {
            return conflict("allocation is not active");
        }
        allocation.status = "checked_out".to_string();
        allocation.revision += 1;
        allocation.checked_out_at = Some(checked_out_at.to_string());
        let room_id = allocation.room_public_id.clone();
        self.allocations
            .insert(allocation.public_id.clone(), allocation.clone());
        let mut changed = vec![Entity::Allocation(allocation)];
        let still_active = self
            .allocations
            .values()
            .any(|row| row.room_public_id == room_id && row.status == "active");
        if !still_active {
            if let Some(mut room) = self.rooms.get(&room_id).cloned() {
                room.status = "vacant".to_string();
                self.rooms.insert(room.public_id.clone(), room.clone());
                changed.push(Entity::Room(room));
            }
        }
        ApplyOutcome::Ack {
            changed,
            replay: false,
            warning: None,
        }
    }

    fn record_payment(&mut self, input: PaymentInput<'_>) -> ApplyOutcome {
        if let Err(message) = validate_money("amount", input.amount) {
            return ApplyOutcome::Conflict { message };
        }
        if !matches!(
            input.payment_method,
            "cash" | "mobile_money" | "bank_transfer" | "card"
        ) {
            return conflict("payment_method must be cash, mobile_money, bank_transfer, or card");
        }
        if !valid_time(input.payment_date, PAYMENT_DATE_FORMAT) {
            return conflict("payment_date must look like 2026-11-15T09:30:00Z");
        }
        if self.payments.contains_key(input.payment_public_id) {
            return conflict("payment id is already used");
        }
        let Some(allocation) = self.allocations.get(input.allocation_public_id) else {
            return conflict("allocation not found");
        };
        if allocation.hostel_public_id != input.hostel_public_id
            || allocation.resident_public_id != input.resident_public_id
        {
            return conflict("payment does not match the stay");
        }
        if self.receipts.values().any(|receipt| {
            receipt.hostel_public_id == input.hostel_public_id
                && receipt.receipt_number == input.receipt_number
        }) {
            return conflict("receipt number is already used");
        }
        let payment = PaymentSnap {
            public_id: input.payment_public_id.to_string(),
            hostel_public_id: input.hostel_public_id.to_string(),
            allocation_public_id: input.allocation_public_id.to_string(),
            resident_public_id: input.resident_public_id.to_string(),
            amount: input.amount.to_string(),
            payment_method: input.payment_method.to_string(),
            payment_date: input.payment_date.to_string(),
            notes: input.notes,
        };
        let receipt = ReceiptSnap {
            public_id: input.receipt_public_id.to_string(),
            hostel_public_id: input.hostel_public_id.to_string(),
            payment_public_id: input.payment_public_id.to_string(),
            receipt_number: input.receipt_number.to_string(),
        };
        self.payments
            .insert(payment.public_id.clone(), payment.clone());
        self.receipts
            .insert(receipt.public_id.clone(), receipt.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Payment(payment), Entity::Receipt(receipt)],
            replay: false,
            warning: None,
        }
    }
}

struct PaymentInput<'a> {
    payment_public_id: &'a str,
    receipt_public_id: &'a str,
    hostel_public_id: &'a str,
    allocation_public_id: &'a str,
    resident_public_id: &'a str,
    amount: &'a str,
    payment_method: &'a str,
    receipt_number: &'a str,
    payment_date: &'a str,
    notes: Option<String>,
}

struct CheckInInput<'a> {
    allocation_public_id: &'a str,
    hostel_public_id: &'a str,
    resident_public_id: &'a str,
    room_public_id: &'a str,
    full_name: &'a str,
    phone: &'a Option<String>,
    email: &'a Option<String>,
    id_number: &'a Option<String>,
    next_of_kin: &'a Option<String>,
    next_of_kin_phone: &'a Option<String>,
    rate_agreed: &'a str,
    term: &'a str,
    academic_year: &'a str,
    checked_in_at: &'a str,
}

fn conflict(message: &str) -> ApplyOutcome {
    ApplyOutcome::Conflict {
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(id: &str) -> SyncCommand {
        SyncCommand::UpsertRoom {
            public_id: id.to_string(),
            hostel_public_id: "h1".to_string(),
            base_revision: 0,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 1,
            floor: None,
            semester_rate: "100".to_string(),
            recess_rate: "50".to_string(),
            status: "vacant".to_string(),
        }
    }

    fn check_in(allocation: &str, resident: &str) -> SyncCommand {
        SyncCommand::CheckIn {
            allocation_public_id: allocation.to_string(),
            hostel_public_id: "h1".to_string(),
            resident_public_id: resident.to_string(),
            room_public_id: "r1".to_string(),
            full_name: "Ada".to_string(),
            phone: None,
            email: None,
            id_number: None,
            next_of_kin: None,
            next_of_kin_phone: None,
            rate_agreed: "100".to_string(),
            term: "semester_1".to_string(),
            academic_year: "2026/2027".to_string(),
            checked_in_at: "2026-11-15 09:00:00".to_string(),
        }
    }

    #[test]
    fn replay_does_not_double_book() {
        let mut ledger = Ledger::default();
        assert!(matches!(
            ledger.apply("room", &room("r1")),
            ApplyOutcome::Ack { .. }
        ));
        assert!(matches!(
            ledger.apply("in-1", &check_in("a1", "p1")),
            ApplyOutcome::Ack { replay: false, .. }
        ));
        let replay = ledger.apply("in-1", &check_in("a1", "p1"));
        assert_eq!(
            replay,
            ApplyOutcome::Ack {
                changed: Vec::new(),
                replay: true,
                warning: None,
            }
        );
        assert_eq!(ledger.allocations.len(), 1);
    }

    #[test]
    fn stale_room_edit_is_rejected() {
        let mut ledger = Ledger::default();
        ledger.apply("room", &room("r1"));
        let stale = SyncCommand::UpsertRoom {
            public_id: "r1".to_string(),
            hostel_public_id: "h1".to_string(),
            base_revision: 0,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 2,
            floor: None,
            semester_rate: "100".to_string(),
            recess_rate: "50".to_string(),
            status: "vacant".to_string(),
        };
        match ledger.apply("edit", &stale) {
            ApplyOutcome::Conflict { message } => assert!(message.contains("another desk")),
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn receipt_number_is_kept_and_not_reused() {
        let mut ledger = Ledger::default();
        ledger.apply("room", &room("r1"));
        ledger.apply("in", &check_in("a1", "p1"));
        let payment = SyncCommand::RecordPayment {
            payment_public_id: "pay1".to_string(),
            receipt_public_id: "rc1".to_string(),
            hostel_public_id: "h1".to_string(),
            allocation_public_id: "a1".to_string(),
            resident_public_id: "p1".to_string(),
            amount: "40".to_string(),
            payment_method: "cash".to_string(),
            receipt_number: "AB12-0007".to_string(),
            payment_date: "2026-11-15T09:30:00Z".to_string(),
            notes: None,
        };
        match ledger.apply("pay", &payment) {
            ApplyOutcome::Ack { changed, .. } => {
                assert!(changed.iter().any(|entity| matches!(
                    entity,
                    Entity::Receipt(receipt) if receipt.receipt_number == "AB12-0007"
                )));
            }
            other => panic!("expected ack, got {other:?}"),
        }
        let mut second = payment;
        if let SyncCommand::RecordPayment {
            payment_public_id,
            receipt_public_id,
            ..
        } = &mut second
        {
            *payment_public_id = "pay2".to_string();
            *receipt_public_id = "rc2".to_string();
        }
        match ledger.apply("pay-2", &second) {
            ApplyOutcome::Conflict { message } => assert!(message.contains("receipt number")),
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    fn payment(id: &str, amount: &str, receipt: &str) -> SyncCommand {
        SyncCommand::RecordPayment {
            payment_public_id: id.to_string(),
            receipt_public_id: format!("rc-{id}"),
            hostel_public_id: "h1".to_string(),
            allocation_public_id: "a1".to_string(),
            resident_public_id: "p1".to_string(),
            amount: amount.to_string(),
            payment_method: "cash".to_string(),
            receipt_number: receipt.to_string(),
            payment_date: "2026-11-15T09:30:00Z".to_string(),
            notes: None,
        }
    }

    fn conflict_message(outcome: ApplyOutcome) -> String {
        match outcome {
            ApplyOutcome::Conflict { message } => message,
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn malformed_or_non_positive_money_is_a_conflict_not_a_crash() {
        let mut ledger = Ledger::default();
        ledger.apply("room", &room("r1"));
        ledger.apply("in", &check_in("a1", "p1"));
        for (n, bad) in ["-5", "0", "abc", "", " 10", "1_000", "+5", "1e3"]
            .iter()
            .enumerate()
        {
            let message = conflict_message(ledger.apply(
                &format!("pay-{n}"),
                &payment(&format!("pay-{n}"), bad, &format!("R{n}")),
            ));
            assert!(message.contains("amount"), "{bad:?}: {message}");
        }
        let mut bad_rate = room("r2");
        if let SyncCommand::UpsertRoom { semester_rate, .. } = &mut bad_rate {
            *semester_rate = "-1".to_string();
        }
        assert!(conflict_message(ledger.apply("room-2", &bad_rate)).contains("semester_rate"));
    }

    #[test]
    fn enum_fields_the_desk_cannot_store_are_conflicts() {
        let mut ledger = Ledger::default();
        let mut bad_status = room("r1");
        if let SyncCommand::UpsertRoom { status, .. } = &mut bad_status {
            *status = "flooded".to_string();
        }
        assert!(conflict_message(ledger.apply("a", &bad_status)).contains("status"));
        ledger.apply("room", &room("r1"));
        let mut bad_term = check_in("a1", "p1");
        if let SyncCommand::CheckIn { term, .. } = &mut bad_term {
            *term = "summer".to_string();
        }
        assert!(conflict_message(ledger.apply("b", &bad_term)).contains("term"));
        ledger.apply("in", &check_in("a1", "p1"));
        let mut bad_method = payment("pay", "10", "R1");
        if let SyncCommand::RecordPayment { payment_method, .. } = &mut bad_method {
            *payment_method = "cheque".to_string();
        }
        assert!(conflict_message(ledger.apply("c", &bad_method)).contains("payment_method"));
        let mut bad_date = payment("pay", "10", "R1");
        if let SyncCommand::RecordPayment { payment_date, .. } = &mut bad_date {
            *payment_date = "yesterday".to_string();
        }
        assert!(conflict_message(ledger.apply("d", &bad_date)).contains("payment_date"));
    }

    #[test]
    fn command_for_another_hostel_is_rejected() {
        let mut ledger = Ledger::default();
        let outcome = ledger.apply_for_hostel("h2", "room", &room("r1"));
        assert!(conflict_message(outcome).contains("different hostel"));
        assert!(matches!(
            ledger.apply_for_hostel("h1", "room", &room("r1")),
            ApplyOutcome::Ack { .. }
        ));
    }

    #[test]
    fn over_capacity_check_in_is_accepted_with_a_warning() {
        let mut ledger = Ledger::default();
        ledger.apply("room", &room("r1"));
        ledger.apply("in-1", &check_in("a1", "p1"));
        match ledger.apply("in-2", &check_in("a2", "p2")) {
            ApplyOutcome::Ack {
                warning: Some(warning),
                ..
            } => assert!(warning.contains("over capacity"), "{warning}"),
            other => panic!("expected ack with warning, got {other:?}"),
        }
        assert_eq!(ledger.allocations.len(), 2);
        // The payment for the overbooked stay is not cascaded into a conflict.
        let mut pay = payment("pay", "10", "R1");
        if let SyncCommand::RecordPayment {
            allocation_public_id,
            resident_public_id,
            ..
        } = &mut pay
        {
            *allocation_public_id = "a2".to_string();
            *resident_public_id = "p2".to_string();
        }
        assert!(matches!(
            ledger.apply("pay", &pay),
            ApplyOutcome::Ack { warning: None, .. }
        ));
    }

    #[test]
    fn reusing_an_allocation_or_payment_id_is_a_conflict() {
        let mut ledger = Ledger::default();
        ledger.apply("room", &room("r1"));
        ledger.apply("in-1", &check_in("a1", "p1"));
        assert!(
            conflict_message(ledger.apply("in-2", &check_in("a1", "p1"))).contains("allocation")
        );
        ledger.apply("pay-1", &payment("pay", "10", "R1"));
        assert!(
            conflict_message(ledger.apply("pay-2", &payment("pay", "10", "R2")))
                .contains("payment")
        );
    }

    #[test]
    fn duplicate_room_number_is_a_conflict() {
        let mut ledger = Ledger::default();
        ledger.apply("room-1", &room("r1"));
        // Same number "101", different id: would break the desk's UNIQUE index on pull.
        assert!(conflict_message(ledger.apply("room-2", &room("r2"))).contains("room_number"));
    }

    #[test]
    fn payment_keeps_its_desk_date() {
        let mut ledger = Ledger::default();
        ledger.apply("room", &room("r1"));
        ledger.apply("in", &check_in("a1", "p1"));
        match ledger.apply("pay", &payment("pay", "10", "R1")) {
            ApplyOutcome::Ack { changed, .. } => assert!(changed.iter().any(|entity| matches!(
                entity,
                Entity::Payment(p) if p.payment_date == "2026-11-15T09:30:00Z"
            ))),
            other => panic!("expected ack, got {other:?}"),
        }
    }

    /// Keep only the rows [`BatchRefs`] says a batch needs, the way the D1 store queries them.
    fn scoped(dump: &LedgerDump, refs: &BatchRefs) -> LedgerDump {
        let allocations: Vec<_> = dump
            .allocations
            .iter()
            .filter(|a| refs.allocations.contains(&a.public_id))
            .cloned()
            .collect();
        let mut rooms = refs.rooms.clone();
        rooms.extend(allocations.iter().map(|a| a.room_public_id.clone()));
        rooms.extend(
            dump.rooms
                .iter()
                .filter(|r| refs.room_numbers.contains(&r.room_number))
                .map(|r| r.public_id.clone()),
        );
        let mut scoped_allocations = allocations;
        scoped_allocations.extend(
            dump.allocations
                .iter()
                .filter(|a| a.status == "active" && rooms.contains(&a.room_public_id))
                .filter(|a| !refs.allocations.contains(&a.public_id))
                .cloned(),
        );
        LedgerDump {
            hostel: dump.hostel.clone(),
            rooms: dump
                .rooms
                .iter()
                .filter(|r| rooms.contains(&r.public_id))
                .cloned()
                .collect(),
            residents: dump
                .residents
                .iter()
                .filter(|r| refs.residents.contains(&r.public_id))
                .cloned()
                .collect(),
            allocations: scoped_allocations,
            payments: dump
                .payments
                .iter()
                .filter(|p| refs.payments.contains(&p.public_id))
                .cloned()
                .collect(),
            receipts: dump
                .receipts
                .iter()
                .filter(|r| refs.receipt_numbers.contains(&r.receipt_number))
                .cloned()
                .collect(),
            preferences: dump.preferences.clone(),
            applied: dump
                .applied
                .iter()
                .filter(|k| refs.idempotency_keys.contains(*k))
                .cloned()
                .collect(),
        }
    }

    #[test]
    fn batch_scoped_ledger_matches_the_full_ledger() {
        let mut full = Ledger::default();
        let mut second_room = room("r2");
        if let SyncCommand::UpsertRoom { room_number, .. } = &mut second_room {
            *room_number = "102".to_string();
        }
        full.apply("room-1", &room("r1"));
        full.apply("room-2", &second_room);
        full.apply("in-1", &check_in("a1", "p1"));
        full.apply("pay-1", &payment("pay1", "10", "R1"));
        let mut other_room_check_in = check_in("a9", "p9");
        if let SyncCommand::CheckIn { room_public_id, .. } = &mut other_room_check_in {
            *room_public_id = "r2".to_string();
        }
        full.apply("in-9", &other_room_check_in);

        let check_out = SyncCommand::CheckOut {
            hostel_public_id: "h1".to_string(),
            allocation_public_id: "a1".to_string(),
            checked_out_at: "2026-11-16 10:00:00".to_string(),
        };
        let batch = vec![
            CommandEnvelope {
                idempotency_key: "pay-1".to_string(),
                command: payment("pay1", "10", "R1"),
            },
            CommandEnvelope {
                idempotency_key: "pay-2".to_string(),
                command: payment("pay2", "10", "R1"),
            },
            CommandEnvelope {
                idempotency_key: "in-2".to_string(),
                command: check_in("a2", "p1"),
            },
            CommandEnvelope {
                idempotency_key: "out-1".to_string(),
                command: check_out,
            },
        ];
        let refs = BatchRefs::of(&batch);
        let mut partial = Ledger::from_dump(scoped(&full.dump(), &refs));
        for envelope in &batch {
            let expected = full.apply(&envelope.idempotency_key, &envelope.command);
            let actual = partial.apply(&envelope.idempotency_key, &envelope.command);
            assert_eq!(actual, expected, "{}", envelope.idempotency_key);
        }
    }
}
