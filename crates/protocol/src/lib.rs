//! Sync commands shared by the desktop cache and the hosted server.
//!
//! Money crosses the wire as decimal strings. Callers validate those strings
//! with `rust_decimal` before they build a [`SyncCommand`].

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    },
    /// Check an allocation out.
    CheckOut {
        /// Hostel.
        hostel_public_id: String,
        /// Allocation to close.
        allocation_public_id: String,
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
        /// Optional note.
        notes: Option<String>,
    },
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
            }),
            SyncCommand::CheckOut {
                hostel_public_id,
                allocation_public_id,
            } => self.check_out(hostel_public_id, allocation_public_id),
            SyncCommand::RecordPayment {
                payment_public_id,
                receipt_public_id,
                hostel_public_id,
                allocation_public_id,
                resident_public_id,
                amount,
                payment_method,
                receipt_number,
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
                notes: notes.clone(),
            }),
        };
        if matches!(outcome, ApplyOutcome::Ack { .. }) {
            self.applied.insert(idempotency_key.to_string());
        }
        outcome
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
        let mut snap = incoming;
        snap.revision = if base == 0 { 1 } else { base + 1 };
        self.rooms.insert(snap.public_id.clone(), snap.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Room(snap)],
            replay: false,
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
        let mut snap = incoming;
        snap.revision = if base == 0 { 1 } else { base + 1 };
        self.preferences = Some(snap.clone());
        ApplyOutcome::Ack {
            changed: vec![Entity::Preferences(snap)],
            replay: false,
        }
    }

    fn check_in(&mut self, input: CheckInInput<'_>) -> ApplyOutcome {
        let Some(room) = self.rooms.get(input.room_public_id).cloned() else {
            return conflict("room not found");
        };
        if room.hostel_public_id != input.hostel_public_id {
            return conflict("room belongs to another hostel");
        }
        if room.status == "maintenance" {
            return conflict("room is under maintenance");
        }
        let active = self
            .allocations
            .values()
            .filter(|allocation| {
                allocation.room_public_id == input.room_public_id && allocation.status == "active"
            })
            .count();
        if active >= room.capacity as usize {
            return conflict("room has no available beds");
        }
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
        };
        self.allocations
            .insert(allocation.public_id.clone(), allocation.clone());
        let mut room = room;
        if room.status == "vacant" {
            room.status = "occupied".to_string();
            room.revision += 1;
            self.rooms.insert(room.public_id.clone(), room.clone());
        }
        let mut changed = vec![Entity::Resident(resident), Entity::Room(room)];
        changed.push(Entity::Allocation(allocation));
        ApplyOutcome::Ack {
            changed,
            replay: false,
        }
    }

    fn check_out(&mut self, hostel_public_id: &str, allocation_public_id: &str) -> ApplyOutcome {
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
                room.revision += 1;
                self.rooms.insert(room.public_id.clone(), room.clone());
                changed.push(Entity::Room(room));
            }
        }
        ApplyOutcome::Ack {
            changed,
            replay: false,
        }
    }

    fn record_payment(&mut self, input: PaymentInput<'_>) -> ApplyOutcome {
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
        }
    }

    #[test]
    fn overlapping_check_in_conflicts_and_replay_does_not_double_book() {
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
            }
        );
        let second = ledger.apply("in-2", &check_in("a2", "p2"));
        match second {
            ApplyOutcome::Conflict { message } => {
                assert!(message.contains("no available beds"), "{message}");
            }
            other => panic!("expected conflict, got {other:?}"),
        }
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
        match ledger.apply("pay-2", &payment) {
            ApplyOutcome::Conflict { message } => assert!(message.contains("receipt number")),
            other => panic!("expected conflict, got {other:?}"),
        }
    }
}
