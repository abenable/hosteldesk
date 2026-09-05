use chrono::NaiveDate;
use printpdf::{BuiltinFont, Mm, PdfDocument};
use rust_decimal::Decimal;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Data required to render a payment receipt.
#[derive(Debug, Clone)]
pub struct ReceiptPayload {
    /// Name of the hostel issuing the receipt.
    pub hostel_name: String,
    /// Sequential receipt number, zero-padded in the rendered output.
    pub receipt_number: u64,
    /// Full name of the resident who made the payment.
    pub resident_name: String,
    /// Payment amount.
    pub amount: Decimal,
    /// Date the payment was recorded.
    pub date: NaiveDate,
    /// Outstanding balance after the payment.
    pub balance: Decimal,
    /// ISO currency code (e.g. "USD") used to prefix monetary amounts.
    pub currency_code: String,
    /// Optional footer text rendered at the bottom of the receipt.
    pub footer_text: Option<String>,
    /// Optional path to a logo image to render at the top of the receipt.
    pub logo_path: Option<String>,
}

impl ReceiptPayload {
    fn format_amount(&self, amount: Decimal) -> String {
        format!("{} {}", self.currency_code, amount)
    }

    /// Format the receipt as plain text for rendering or testing.
    pub fn format_text(&self) -> String {
        format!(
            "Hostel: {hostel}\nReceipt No: {receipt_no:08}\nResident: {resident}\nAmount Paid: {amount}\nDate: {date}\nBalance: {balance}",
            hostel = self.hostel_name,
            receipt_no = self.receipt_number,
            resident = self.resident_name,
            amount = self.format_amount(self.amount),
            date = self.date.format("%Y-%m-%d"),
            balance = self.format_amount(self.balance),
        )
    }
}

/// Generate a PDF receipt at `path` from `payload`.
///
/// Overwrites any existing file at the path. The page size is fixed at A4.
pub fn generate_receipt(payload: &ReceiptPayload, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let text = payload.format_text();
    let file = File::create(path.as_ref())?;
    let mut writer = BufWriter::new(file);

    let (doc, page, layer) = PdfDocument::new(
        format!("Receipt {}", payload.receipt_number),
        Mm(210.0),
        Mm(297.0),
        "Layer 1",
    );
    let current_layer = doc.get_page(page).get_layer(layer);
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;

    let start_y: f32 = 270.0;
    let line_height: f32 = 7.0;
    let x: f32 = 20.0;

    // Leave room at the top when a logo is configured.
    let logo_offset: f32 = if payload.logo_path.is_some() {
        30.0
    } else {
        0.0
    };
    let mut y = start_y - logo_offset;

    for line in text.lines() {
        current_layer.use_text(line, 12.0, Mm(x), Mm(y), &font);
        y -= line_height;
    }

    if let Some(footer) = &payload.footer_text {
        current_layer.use_text(footer.as_str(), 10.0, Mm(x), Mm(20.0), &font);
    }

    // ponytail: embedding a logo needs printpdf's `image` feature. Enabling it
    // pulls in image v0.24.* plus decoder crates that are not currently in
    // Cargo.lock, so an existing logo path is ignored rather than blocking
    // receipt generation.
    if let Some(logo_path) = &payload.logo_path {
        if Path::new(logo_path).is_file() {
            eprintln!("pdf: ignoring logo at {logo_path:?}; image feature not enabled");
        }
    }

    doc.save(&mut writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn payload_formats_all_required_fields() {
        let payload = ReceiptPayload {
            hostel_name: "Sunny Hostel".to_string(),
            receipt_number: 123,
            resident_name: "Jane Doe".to_string(),
            amount: Decimal::from_str("150.50").unwrap(),
            date: NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
            balance: Decimal::from_str("49.50").unwrap(),
            currency_code: "USD".to_string(),
            footer_text: None,
            logo_path: None,
        };

        let formatted = payload.format_text();

        assert!(
            formatted.contains("Hostel: Sunny Hostel"),
            "missing hostel name: {formatted}"
        );
        assert!(
            formatted.contains("Receipt No: 00000123"),
            "missing or malformed receipt number: {formatted}"
        );
        assert!(
            formatted.contains("Resident: Jane Doe"),
            "missing resident name: {formatted}"
        );
        assert!(
            formatted.contains("Amount Paid: USD 150.50"),
            "missing or malformed amount: {formatted}"
        );
        assert!(
            formatted.contains("Date: 2024-03-15"),
            "missing date: {formatted}"
        );
        assert!(
            formatted.contains("Balance: USD 49.50"),
            "missing or malformed balance: {formatted}"
        );
    }

    #[test]
    fn generates_valid_pdf_file() {
        let payload = ReceiptPayload {
            hostel_name: "Test Hostel".to_string(),
            receipt_number: 999,
            resident_name: "John Smith".to_string(),
            amount: Decimal::from_str("200.00").unwrap(),
            date: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            balance: Decimal::from_str("0.00").unwrap(),
            currency_code: "USD".to_string(),
            footer_text: Some("Thank you for your stay!".to_string()),
            logo_path: Some("/nonexistent/logo.png".to_string()),
        };
        let path = std::env::temp_dir().join(format!(
            "hosteldesk_receipt_test_{}.pdf",
            std::process::id()
        ));

        generate_receipt(&payload, &path).expect("generate receipt");

        let bytes = std::fs::read(&path).expect("read generated pdf");
        assert!(
            bytes.starts_with(b"%PDF"),
            "generated file is not a PDF: {bytes:?}"
        );
        assert!(bytes.len() > 100, "generated PDF is unexpectedly small");

        let _ = std::fs::remove_file(&path);
    }
}
