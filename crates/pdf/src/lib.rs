use chrono::NaiveDate;
use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PaintMode, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, RawImage, Rect, Rgb, TextItem, XObjectTransform,
};
use rust_decimal::Decimal;
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
    /// Receipt number as printed. Numeric receipts are zero-padded; a device
    /// prefix such as `AB12-0007` is shown as issued.
    pub receipt_number: String,
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

/// Numeric receipt numbers stay zero-padded. Prefixed numbers are already formatted.
fn display_receipt_number(number: &str) -> String {
    if number.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("{number:0>6}")
    } else {
        number.to_string()
    }
}

/// Generate a PDF receipt at `path` from `payload`.
///
/// Overwrites any existing file at the path. The page size is fixed at A4.
pub fn generate_receipt(payload: &ReceiptPayload, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let mut doc = PdfDocument::new(&format!("Receipt {}", payload.receipt_number));
    let regular = BuiltinFont::Helvetica;
    let bold = BuiltinFont::HelveticaBold;

    let black = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
    let mut l = vec![
        Op::SetFillColor { col: black.clone() },
        Op::SetOutlineColor { col: black },
        Op::SetOutlineThickness { pt: Pt(0.8) },
    ];

    let margin_x: f32 = 20.0;
    let right_x: f32 = 190.0;
    let page_center: f32 = 105.0;

    // Header: logo (if any) at the left, hostel identity stacked beside it.
    let logo_box = 22.0;
    let mut text_x = margin_x;
    let header_top = 277.0;
    if let Some(logo_path) = &payload.logo_path {
        if draw_logo(
            &mut doc,
            &mut l,
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

    text(
        &mut l,
        &payload.hostel_name,
        16.0,
        text_x,
        header_top - 6.0,
        bold,
    );
    let mut header_y = header_top - 12.0;
    if let Some(address) = &payload.hostel_address {
        if !address.trim().is_empty() {
            text(&mut l, address, 9.0, text_x, header_y, regular);
            header_y -= 5.0;
        }
    }
    if let Some(phone) = &payload.hostel_phone {
        if !phone.trim().is_empty() {
            text(
                &mut l,
                format!("Tel: {phone}"),
                9.0,
                text_x,
                header_y,
                regular,
            );
        }
    }

    // Divider under the header.
    let divider_y = header_top - logo_box - 4.0;
    hline(&mut l, margin_x, right_x, divider_y);

    // Title box, receipt number and date.
    let title = "PAYMENT RECEIPT";
    let title_y = divider_y - 12.0;
    let box_w = title.len() as f32 * 3.0 + 16.0;
    stroke_rect(
        &mut l,
        page_center - box_w / 2.0,
        title_y - 3.0,
        page_center + box_w / 2.0,
        title_y + 6.0,
    );
    text(
        &mut l,
        title,
        12.0,
        page_center - (title.len() as f32 * 1.55),
        title_y,
        bold,
    );
    text(
        &mut l,
        format!("No. {}", display_receipt_number(&payload.receipt_number)),
        11.0,
        margin_x,
        title_y,
        bold,
    );
    text(
        &mut l,
        format!("Date: {}", payload.date.format("%d/%m/%Y")),
        11.0,
        right_x - 35.0,
        title_y,
        regular,
    );

    // Body fields.
    let mut y = title_y - 16.0;
    let line_gap = 9.0;
    text(
        &mut l,
        format!("Received with thanks from: {}", payload.resident_name),
        11.0,
        margin_x,
        y,
        regular,
    );
    y -= line_gap;

    text(
        &mut l,
        format!(
            "The Sum of {}: {}",
            payload.currency_code,
            amount_in_words(payload.amount)
        ),
        11.0,
        margin_x,
        y,
        regular,
    );
    y -= line_gap;

    text(
        &mut l,
        format!("Being payment for: {}", payload.being_payment_for()),
        11.0,
        margin_x,
        y,
        regular,
    );
    y -= line_gap;

    text(
        &mut l,
        format!("Payment method: {}", payload.payment_method_label()),
        11.0,
        margin_x,
        y,
        regular,
    );
    text(
        &mut l,
        format!("Balance: {}", payload.format_amount(payload.balance)),
        11.0,
        right_x - 60.0,
        y,
        regular,
    );
    y -= line_gap + 4.0;

    // Boxed total amount.
    let box_top = y;
    let box_bottom = y - 12.0;
    stroke_rect(&mut l, margin_x, box_bottom, margin_x + 60.0, box_top);
    text(
        &mut l,
        payload.format_amount(payload.amount),
        13.0,
        margin_x + 4.0,
        box_bottom + 4.0,
        bold,
    );
    text(
        &mut l,
        "With thanks.",
        10.0,
        margin_x + 66.0,
        box_bottom + 4.0,
        regular,
    );

    // Footer note.
    let footer_y = box_bottom - 12.0;
    let footer_text = payload
        .footer_text
        .clone()
        .unwrap_or_else(|| "The amount paid is non-refundable.".to_string());
    text(&mut l, footer_text, 9.0, margin_x, footer_y, regular);

    // Signature.
    let sign_y = 35.0;
    hline(&mut l, right_x - 55.0, right_x, sign_y);
    text(&mut l, "Sign", 9.0, right_x - 55.0, sign_y - 5.0, regular);
    text(
        &mut l,
        format!("For: {}", payload.hostel_name),
        9.0,
        right_x - 55.0,
        sign_y + 2.0,
        regular,
    );

    let bytes = doc
        .with_pages(vec![PdfPage::new(Mm(210.0), Mm(297.0), l)])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write(path.as_ref(), bytes)?;
    Ok(())
}

/// One line of `text` in a built-in font, with its baseline starting at
/// `(x, y)` mm.
fn text(ops: &mut Vec<Op>, text: impl Into<String>, size: f32, x: f32, y: f32, font: BuiltinFont) {
    ops.extend([
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::Builtin(font),
            size: Pt(size),
        },
        Op::SetTextCursor {
            pos: Point::new(Mm(x), Mm(y)),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.into())],
        },
        Op::EndTextSection,
    ]);
}

/// Outline of the rectangle from `(x1, y1)` (bottom left) to `(x2, y2)` (top right), in mm.
fn stroke_rect(ops: &mut Vec<Op>, x1: f32, y1: f32, x2: f32, y2: f32) {
    ops.push(Op::DrawRectangle {
        rectangle: Rect {
            mode: Some(PaintMode::Stroke),
            ..Rect::from_xywh(
                Mm(x1).into(),
                Mm(y1).into(),
                Mm(x2 - x1).into(),
                Mm(y2 - y1).into(),
            )
        },
    });
}

fn hline(ops: &mut Vec<Op>, x1: f32, x2: f32, y: f32) {
    let point = |x| LinePoint {
        p: Point::new(Mm(x), Mm(y)),
        bezier: false,
    };
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![point(x1), point(x2)],
            is_closed: false,
        },
    });
}

/// Decode and place a logo image so it fits within `max_w` x `max_h` mm, with
/// its top-left corner at `(x, y + max_h)`.
fn draw_logo(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    logo_path: &str,
    x: f32,
    y: f32,
    max_w: f32,
    max_h: f32,
) -> anyhow::Result<()> {
    let bytes = std::fs::read(logo_path)?;
    let image = RawImage::decode_from_bytes(&bytes, &mut Vec::new()).map_err(anyhow::Error::msg)?;
    let (px_w, px_h) = (image.width as f32, image.height as f32);
    if px_w == 0.0 || px_h == 0.0 {
        anyhow::bail!("logo image has zero dimensions");
    }

    let dpi_w = px_w * 25.4 / max_w;
    let dpi_h = px_h * 25.4 / max_h;
    let dpi = dpi_w.max(dpi_h);
    let rendered_h = px_h * 25.4 / dpi;

    let id = doc.add_image(&image);
    ops.push(Op::UseXobject {
        id,
        transform: XObjectTransform {
            translate_x: Some(Mm(x).into()),
            translate_y: Some(Mm(y + max_h - rendered_h).into()),
            dpi: Some(dpi),
            ..Default::default()
        },
    });
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
            receipt_number: "123".to_string(),
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
    fn places_a_png_logo() {
        // A logo that fails to decode is skipped silently, so check it decodes.
        let pixel = printpdf::RawImage {
            pixels: printpdf::RawImageData::U8(vec![255, 0, 0, 0, 255, 0]),
            width: 2,
            height: 1,
            data_format: printpdf::RawImageFormat::RGB8,
            tag: Vec::new(),
        };
        let (png, _) = pixel
            .encode_to_bytes(&[printpdf::OutputImageFormat::Png])
            .expect("encode png");
        let path =
            std::env::temp_dir().join(format!("hosteldesk_logo_test_{}.png", std::process::id()));
        std::fs::write(&path, png).expect("write png");

        let mut doc = PdfDocument::new("logo");
        let mut ops = Vec::new();
        let placed = draw_logo(
            &mut doc,
            &mut ops,
            path.to_str().expect("utf-8 path"),
            20.0,
            255.0,
            22.0,
            22.0,
        );
        let _ = std::fs::remove_file(&path);

        placed.expect("logo placed");
        assert!(matches!(ops.as_slice(), [Op::UseXobject { .. }]));
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
