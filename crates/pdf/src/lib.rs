use chrono::NaiveDate;
use printpdf::path::PaintMode;
use printpdf::{BuiltinFont, Color, Line, Mm, PdfDocument, Point, Rect, Rgb};
use rust_decimal::Decimal;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Data required to render a payment receipt.
#[derive(Debug, Clone)]
pub struct ReceiptPayload {
    /// Name of the hostel issuing the receipt.
    pub hostel_name: String,
    /// Optional postal/physical address, shown under the hostel name.
    pub hostel_address: Option<String>,
    /// Optional contact phone number, shown under the address.
    pub hostel_phone: Option<String>,
    /// Sequential receipt number, zero-padded in the rendered output.
    pub receipt_number: u64,
    /// Full name of the resident who made the payment.
    pub resident_name: String,
    /// Room the payment is associated with.
    pub room_number: String,
    /// Payment amount.
    pub amount: Decimal,
    /// Date the payment was recorded.
    pub date: NaiveDate,
    /// Outstanding balance after the payment.
    pub balance: Decimal,
    /// Payment method (e.g. "cash", "bank_transfer").
    pub payment_method: String,
    /// Optional free-text note describing what the payment covers.
    pub notes: Option<String>,
    /// ISO currency code (e.g. "USD") used to prefix monetary amounts.
    pub currency_code: String,
    /// Optional footer text rendered at the bottom of the receipt.
    pub footer_text: Option<String>,
    /// Optional path to a logo image to render at the top of the receipt.
    pub logo_path: Option<String>,
}

impl ReceiptPayload {
    fn format_amount(&self, amount: Decimal) -> String {
        format!("{} {}", self.currency_code, format_with_commas(amount))
    }

    fn payment_method_label(&self) -> String {
        self.payment_method.replace('_', " ").to_uppercase()
    }

    fn being_payment_for(&self) -> String {
        match &self.notes {
            Some(notes) if !notes.trim().is_empty() => {
                format!("Room {} - {}", self.room_number, notes.trim())
            }
            _ => format!("Room {} - Rent payment", self.room_number),
        }
    }
}

/// Group the integer part of a decimal amount with thousands separators,
/// preserving any fractional part (e.g. `150000` -> `150,000`, `1234.5` ->
/// `1,234.5`).
fn format_with_commas(amount: Decimal) -> String {
    let s = format!("{amount:.2}");
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s.as_str(), None),
    };
    let (sign, digits) = match int_part.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", int_part),
    };
    let mut grouped = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let grouped_int: String = grouped.chars().rev().collect();
    match frac_part {
        Some(f) => format!("{sign}{grouped_int}.{f}"),
        None => format!("{sign}{grouped_int}"),
    }
}

/// Spell out a non-negative whole number in English words (e.g. `100000` ->
/// `ONE HUNDRED THOUSAND`).
///
/// ponytail: supports up to 999 trillion, far beyond any realistic hostel
/// payment; larger values are truncated to the highest supported scale.
fn number_to_words(mut n: u64) -> String {
    const ONES: [&str; 20] = [
        "ZERO",
        "ONE",
        "TWO",
        "THREE",
        "FOUR",
        "FIVE",
        "SIX",
        "SEVEN",
        "EIGHT",
        "NINE",
        "TEN",
        "ELEVEN",
        "TWELVE",
        "THIRTEEN",
        "FOURTEEN",
        "FIFTEEN",
        "SIXTEEN",
        "SEVENTEEN",
        "EIGHTEEN",
        "NINETEEN",
    ];
    const TENS: [&str; 10] = [
        "", "", "TWENTY", "THIRTY", "FORTY", "FIFTY", "SIXTY", "SEVENTY", "EIGHTY", "NINETY",
    ];
    const SCALES: [&str; 5] = ["", "THOUSAND", "MILLION", "BILLION", "TRILLION"];

    fn three_digits(n: u64, ones: &[&str; 20], tens: &[&str; 10]) -> String {
        let mut words = Vec::new();
        let hundreds = n / 100;
        let rest = n % 100;
        if hundreds > 0 {
            words.push(format!("{} HUNDRED", ones[hundreds as usize]));
        }
        if rest > 0 {
            if rest < 20 {
                words.push(ones[rest as usize].to_string());
            } else {
                let t = rest / 10;
                let o = rest % 10;
                if o > 0 {
                    words.push(format!("{}-{}", tens[t as usize], ones[o as usize]));
                } else {
                    words.push(tens[t as usize].to_string());
                }
            }
        }
        words.join(" ")
    }

    if n == 0 {
        return ONES[0].to_string();
    }

    let mut groups = Vec::new();
    while n > 0 {
        groups.push(n % 1000);
        n /= 1000;
    }

    let mut parts = Vec::new();
    for (i, group) in groups.iter().enumerate().rev() {
        if *group == 0 {
            continue;
        }
        let words = three_digits(*group, &ONES, &TENS);
        let scale = SCALES.get(i).copied().unwrap_or("");
        if scale.is_empty() {
            parts.push(words);
        } else {
            parts.push(format!("{words} {scale}"));
        }
    }

    parts.join(" ")
}

/// Spell out a monetary amount in words, e.g. `150000.75` ->
/// `ONE HUNDRED FIFTY THOUSAND AND CENTS SEVENTY FIVE ONLY`.
///
/// Rounds with the same `round_dp(2)` as [`format_with_commas`] so the words
/// line always agrees with the printed figures — the words are what a bank
/// or auditor treats as authoritative on a receipt, so they cannot come from
/// a different rounding than the number shown next to them.
fn amount_in_words(amount: Decimal) -> String {
    let rounded = format!("{amount:.2}");
    let (whole_str, cents_str) = rounded.split_once('.').unwrap_or((rounded.as_str(), "00"));
    let whole_amount: u64 = whole_str.trim_start_matches('-').parse().unwrap_or(0);
    let cents: u64 = cents_str.parse().unwrap_or(0);

    if cents > 0 {
        format!(
            "{} AND CENTS {} ONLY",
            number_to_words(whole_amount),
            number_to_words(cents)
        )
    } else {
        format!("{} ONLY", number_to_words(whole_amount))
    }
}

/// Generate a PDF receipt at `path` from `payload`.
///
/// Overwrites any existing file at the path. The page size is fixed at A4.
pub fn generate_receipt(payload: &ReceiptPayload, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let file = File::create(path.as_ref())?;
    let mut writer = BufWriter::new(file);

    let (doc, page, layer) = PdfDocument::new(
        format!("Receipt {}", payload.receipt_number),
        Mm(210.0),
        Mm(297.0),
        "Layer 1",
    );
    let l = doc.get_page(page).get_layer(layer);
    let regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;

    let black = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
    l.set_fill_color(black.clone());
    l.set_outline_color(black);
    l.set_outline_thickness(0.8);

    let margin_x: f32 = 20.0;
    let right_x: f32 = 190.0;
    let page_center: f32 = 105.0;

    // Header: logo (if any) at the left, hostel identity stacked beside it.
    let logo_box = 22.0;
    let mut text_x = margin_x;
    let header_top = 277.0;
    if let Some(logo_path) = &payload.logo_path {
        if draw_logo(
            &l,
            logo_path,
            margin_x,
            header_top - logo_box,
            logo_box,
            logo_box,
        )
        .is_ok()
        {
            text_x = margin_x + logo_box + 6.0;
        }
    }

    l.use_text(
        &payload.hostel_name,
        16.0,
        Mm(text_x),
        Mm(header_top - 6.0),
        &bold,
    );
    let mut header_y = header_top - 12.0;
    if let Some(address) = &payload.hostel_address {
        if !address.trim().is_empty() {
            l.use_text(address, 9.0, Mm(text_x), Mm(header_y), &regular);
            header_y -= 5.0;
        }
    }
    if let Some(phone) = &payload.hostel_phone {
        if !phone.trim().is_empty() {
            l.use_text(
                format!("Tel: {phone}"),
                9.0,
                Mm(text_x),
                Mm(header_y),
                &regular,
            );
        }
    }

    // Divider under the header.
    let divider_y = header_top - logo_box - 4.0;
    hline(&l, margin_x, right_x, divider_y);

    // Title box, receipt number and date.
    let title = "PAYMENT RECEIPT";
    let title_y = divider_y - 12.0;
    let box_w = title.len() as f32 * 3.0 + 16.0;
    l.add_rect(
        Rect::new(
            Mm(page_center - box_w / 2.0),
            Mm(title_y - 3.0),
            Mm(page_center + box_w / 2.0),
            Mm(title_y + 6.0),
        )
        .with_mode(PaintMode::Stroke),
    );
    l.use_text(
        title,
        12.0,
        Mm(page_center - (title.len() as f32 * 1.55)),
        Mm(title_y),
        &bold,
    );
    l.use_text(
        format!("No. {:06}", payload.receipt_number),
        11.0,
        Mm(margin_x),
        Mm(title_y),
        &bold,
    );
    l.use_text(
        format!("Date: {}", payload.date.format("%d/%m/%Y")),
        11.0,
        Mm(right_x - 35.0),
        Mm(title_y),
        &regular,
    );

    // Body fields.
    let mut y = title_y - 16.0;
    let line_gap = 9.0;
    l.use_text(
        format!("Received with thanks from: {}", payload.resident_name),
        11.0,
        Mm(margin_x),
        Mm(y),
        &regular,
    );
    y -= line_gap;

    l.use_text(
        format!(
            "The Sum of {}: {}",
            payload.currency_code,
            amount_in_words(payload.amount)
        ),
        11.0,
        Mm(margin_x),
        Mm(y),
        &regular,
    );
    y -= line_gap;

    l.use_text(
        format!("Being payment for: {}", payload.being_payment_for()),
        11.0,
        Mm(margin_x),
        Mm(y),
        &regular,
    );
    y -= line_gap;

    l.use_text(
        format!("Payment method: {}", payload.payment_method_label()),
        11.0,
        Mm(margin_x),
        Mm(y),
        &regular,
    );
    l.use_text(
        format!("Balance: {}", payload.format_amount(payload.balance)),
        11.0,
        Mm(right_x - 60.0),
        Mm(y),
        &regular,
    );
    y -= line_gap + 4.0;

    // Boxed total amount.
    let box_top = y;
    let box_bottom = y - 12.0;
    l.add_rect(
        Rect::new(
            Mm(margin_x),
            Mm(box_bottom),
            Mm(margin_x + 60.0),
            Mm(box_top),
        )
        .with_mode(PaintMode::Stroke),
    );
    l.use_text(
        payload.format_amount(payload.amount),
        13.0,
        Mm(margin_x + 4.0),
        Mm(box_bottom + 4.0),
        &bold,
    );
    l.use_text(
        "With thanks.",
        10.0,
        Mm(margin_x + 66.0),
        Mm(box_bottom + 4.0),
        &regular,
    );

    // Footer note.
    let footer_y = box_bottom - 12.0;
    let footer_text = payload
        .footer_text
        .clone()
        .unwrap_or_else(|| "The amount paid is non-refundable.".to_string());
    l.use_text(footer_text, 9.0, Mm(margin_x), Mm(footer_y), &regular);

    // Signature.
    let sign_y = 35.0;
    hline(&l, right_x - 55.0, right_x, sign_y);
    l.use_text("Sign", 9.0, Mm(right_x - 55.0), Mm(sign_y - 5.0), &regular);
    l.use_text(
        format!("For: {}", payload.hostel_name),
        9.0,
        Mm(right_x - 55.0),
        Mm(sign_y + 2.0),
        &regular,
    );

    doc.save(&mut writer)?;
    Ok(())
}

fn hline(layer: &printpdf::PdfLayerReference, x1: f32, x2: f32, y: f32) {
    let line = Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y)), false),
            (Point::new(Mm(x2), Mm(y)), false),
        ],
        is_closed: false,
    };
    layer.add_line(line);
}

/// Decode and place a logo image so it fits within `max_w` x `max_h` mm, with
/// its top-left corner at `(x, y + max_h)`.
fn draw_logo(
    layer: &printpdf::PdfLayerReference,
    logo_path: &str,
    x: f32,
    y: f32,
    max_w: f32,
    max_h: f32,
) -> anyhow::Result<()> {
    let bytes = std::fs::read(logo_path)?;
    let dynamic_image = image::load_from_memory(&bytes)?;
    let (px_w, px_h) = (dynamic_image.width() as f32, dynamic_image.height() as f32);
    if px_w == 0.0 || px_h == 0.0 {
        anyhow::bail!("logo image has zero dimensions");
    }

    let dpi_w = px_w * 25.4 / max_w;
    let dpi_h = px_h * 25.4 / max_h;
    let dpi = dpi_w.max(dpi_h);
    let rendered_h = px_h * 25.4 / dpi;

    let image = printpdf::Image::from_dynamic_image(&dynamic_image);
    image.add_to_layer(
        layer.clone(),
        printpdf::ImageTransform {
            translate_x: Some(Mm(x)),
            translate_y: Some(Mm(y + max_h - rendered_h)),
            dpi: Some(dpi),
            ..Default::default()
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn sample_payload() -> ReceiptPayload {
        ReceiptPayload {
            hostel_name: "Sunny Hostel".to_string(),
            hostel_address: Some("Plot 1, Main Street".to_string()),
            hostel_phone: Some("0700000000".to_string()),
            receipt_number: 123,
            resident_name: "Jane Doe".to_string(),
            room_number: "A01".to_string(),
            amount: Decimal::from_str("150000").unwrap(),
            date: NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
            balance: Decimal::from_str("49500.50").unwrap(),
            payment_method: "bank_transfer".to_string(),
            notes: None,
            currency_code: "UGX".to_string(),
            footer_text: None,
            logo_path: None,
        }
    }

    #[test]
    fn formats_amounts_with_thousands_separators() {
        // Always exactly 2 decimal places: a receipt figure that drops
        // trailing zeros (49,500.5 instead of 49,500.50) reads as a typo.
        assert_eq!(
            format_with_commas(Decimal::from_str("150000").unwrap()),
            "150,000.00"
        );
        assert_eq!(
            format_with_commas(Decimal::from_str("49500.50").unwrap()),
            "49,500.50"
        );
        assert_eq!(
            format_with_commas(Decimal::from_str("500").unwrap()),
            "500.00"
        );
    }

    #[test]
    fn amount_in_words_matches_the_same_rounding_as_figures() {
        // C6: words must agree with figures on the exact same rounded value.
        assert_eq!(
            amount_in_words(Decimal::from_str("150000").unwrap()),
            "ONE HUNDRED FIFTY THOUSAND ONLY"
        );
        assert_eq!(
            amount_in_words(Decimal::from_str("150000.75").unwrap()),
            "ONE HUNDRED FIFTY THOUSAND AND CENTS SEVENTY-FIVE ONLY"
        );
        assert_eq!(
            amount_in_words(Decimal::from_str("49500.50").unwrap()),
            "FORTY-NINE THOUSAND FIVE HUNDRED AND CENTS FIFTY ONLY"
        );
    }

    #[test]
    fn spells_out_numbers_in_words() {
        assert_eq!(number_to_words(0), "ZERO");
        assert_eq!(number_to_words(100_000), "ONE HUNDRED THOUSAND");
        assert_eq!(number_to_words(21), "TWENTY-ONE");
        assert_eq!(
            number_to_words(1_234_567),
            "ONE MILLION TWO HUNDRED THIRTY-FOUR THOUSAND FIVE HUNDRED SIXTY-SEVEN"
        );
    }

    #[test]
    fn being_payment_for_falls_back_without_notes() {
        let payload = sample_payload();
        assert_eq!(payload.being_payment_for(), "Room A01 - Rent payment");
    }

    #[test]
    fn being_payment_for_uses_notes_when_present() {
        let mut payload = sample_payload();
        payload.notes = Some("June rent".to_string());
        assert_eq!(payload.being_payment_for(), "Room A01 - June rent");
    }

    #[test]
    fn generates_valid_pdf_file() {
        let payload = sample_payload();
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
