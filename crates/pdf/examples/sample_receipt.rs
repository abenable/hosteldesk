//! Writes a sample receipt, for checking the layout by eye:
//! `cargo run -p pdf --example sample_receipt -- receipt.pdf`

use chrono::NaiveDate;
use rust_decimal::Decimal;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sample-receipt.pdf".into());
    let payload = pdf::ReceiptPayload {
        hostel_name: "Makerere Heights Hostel".into(),
        hostel_address: Some("Plot 14, Kikoni Road, Kampala".into()),
        hostel_phone: Some("+256 700 123 456".into()),
        receipt_number: "A-000142".into(),
        resident_name: "Sarah Namubiru".into(),
        resident_phone: Some("+256 772 418 905".into()),
        resident_email: Some("sarah.namubiru@gmail.com".into()),
        room_number: "B12".into(),
        room_type: Some("double".into()),
        term: Some("semester_1".into()),
        academic_year: Some("2026/2027".into()),
        amount: Decimal::new(500_000, 0),
        date: NaiveDate::from_ymd_opt(2026, 11, 28).unwrap_or_default(),
        balance: Decimal::new(400_000, 0),
        notes: None,
        currency_code: "UGX".into(),
        footer_text: None,
        logo_path: None,
        received_by: Some("Grace Akello".into()),
        received_by_contact: Some("grace@makerereheights.ug".into()),
    };
    pdf::generate_receipt(&payload, &out)?;
    println!("wrote {out}");
    Ok(())
}
