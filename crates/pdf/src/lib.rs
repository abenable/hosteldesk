//! PDF payment receipts, laid out like the HostelDesk receipt template: the
//! hostel's logo and details at the top, who paid, what for, the invoice, the
//! amount paid and the balance, then who received it, the date and a line to
//! sign, with the hostel's details along the bottom edge.

use chrono::NaiveDate;
use printpdf::{
    Color, FontId, Line, LinePoint, Mm, Op, PaintMode, ParsedFont, PdfDocument, PdfFontHandle,
    PdfPage, PdfSaveOptions, Point, Pt, RawImage, Rect, Rgb, TextItem, TextRenderingMode,
    XObjectTransform,
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
    /// The resident's phone, if recorded.
    pub resident_phone: Option<String>,
    /// The resident's email, if recorded.
    pub resident_email: Option<String>,
    /// Room the payment is associated with.
    pub room_number: String,
    /// The room's type (`single`, `double` or `dorm`), from its class.
    pub room_type: Option<String>,
    /// The stay's term: `semester_1`, `semester_2` or `recess`.
    pub term: Option<String>,
    /// The stay's academic year, e.g. `2026/2027`.
    pub academic_year: Option<String>,
    /// Payment amount.
    pub amount: Decimal,
    /// Date the payment was recorded.
    pub date: NaiveDate,
    /// Outstanding balance after the payment.
    pub balance: Decimal,
    /// Optional free-text note describing what the payment covers; replaces
    /// "Rent" in the reason.
    pub notes: Option<String>,
    /// ISO currency code (e.g. "UGX") used to label monetary amounts.
    pub currency_code: String,
    /// Optional line from Settings → Preferences, printed above the footer.
    pub footer_text: Option<String>,
    /// Optional path to the hostel's logo; the HostelDesk logo is used without one.
    pub logo_path: Option<String>,
    /// The staff member who took the payment, when known. A receipt printed
    /// again on another computer leaves it blank for a handwritten name.
    pub received_by: Option<String>,
    /// How to reach that staff member (their email).
    pub received_by_contact: Option<String>,
}

impl ReceiptPayload {
    /// What was owed before this payment: the payment plus what is left.
    fn invoice(&self) -> Decimal {
        self.amount + self.balance
    }

    /// `"Rent, Room B12 (Double)"`, with the note in place of "Rent" when there is one.
    fn reason(&self) -> String {
        let what = self
            .notes
            .as_deref()
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .unwrap_or("Rent");
        match self.room_type.as_deref().map(room_type_label) {
            Some(kind) => format!("{what}, Room {} ({kind})", self.room_number),
            None => format!("{what}, Room {}", self.room_number),
        }
    }

    /// `"Semester 1, 2026/27"`; `None` without a term.
    fn term_line(&self) -> Option<String> {
        let term = match self.term.as_deref()? {
            "semester_1" => "Semester 1".to_string(),
            "semester_2" => "Semester 2".to_string(),
            "recess" => "Recess".to_string(),
            other => other.to_string(),
        };
        Some(match self.academic_year.as_deref().map(short_year) {
            Some(year) => format!("{term}, {year}"),
            None => term,
        })
    }

    /// `"UGX 500,000"`.
    fn money(&self, amount: Decimal) -> String {
        format!("{} {}", self.currency_code, format_money(amount))
    }
}

/// `"double"` → `"Double"`; anything else as given.
fn room_type_label(kind: &str) -> String {
    let mut chars = kind.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// `"2026/2027"` → `"2026/27"`, as the template prints it; other forms as given.
fn short_year(year: &str) -> String {
    match year.split_once('/') {
        Some((from, to)) if from.len() == 4 && to.len() == 4 => format!("{from}/{}", &to[2..]),
        _ => year.to_string(),
    }
}

/// Thousands separators, rounded to cents; whole amounts drop the `.00`
/// (`500000` → `500,000`, `49500.5` → `49,500.50`).
fn format_money(amount: Decimal) -> String {
    let rounded = format!("{:.2}", amount.round_dp(2));
    let (whole, cents) = rounded.split_once('.').unwrap_or((rounded.as_str(), "00"));
    let (sign, digits) = match whole.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", whole),
    };
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if cents == "00" {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{cents}")
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

/// The HostelDesk logo (transparent background), printed when a hostel has no logo of its own.
const DEFAULT_LOGO: &[u8] = include_bytes!("../assets/hosteldesk-logo.png");

// Latin subsets of Lato and Italiana, renamed as the OFL asks (see assets/OFL.txt).
const SANS_REGULAR: &[u8] = include_bytes!("../assets/HostelDeskSans-Regular.ttf");
const SANS_BOLD: &[u8] = include_bytes!("../assets/HostelDeskSans-Bold.ttf");
const DISPLAY: &[u8] = include_bytes!("../assets/HostelDeskDisplay-Regular.ttf");

/// The three faces of the template.
#[derive(Debug, Clone, Copy)]
enum Face {
    Regular,
    Bold,
    Display,
}

/// The parsed faces, for measuring text before it is placed.
struct Fonts {
    regular: ParsedFont,
    bold: ParsedFont,
    display: ParsedFont,
}

impl Fonts {
    fn load() -> anyhow::Result<Self> {
        let parse = |bytes: &[u8], name: &str| {
            ParsedFont::from_bytes(bytes, 0, &mut Vec::new())
                .ok_or_else(|| anyhow::anyhow!("the {name} receipt font did not load"))
        };
        Ok(Self {
            regular: parse(SANS_REGULAR, "regular")?,
            bold: parse(SANS_BOLD, "bold")?,
            display: parse(DISPLAY, "display")?,
        })
    }

    fn get(&self, face: Face) -> &ParsedFont {
        match face {
            Face::Regular => &self.regular,
            Face::Bold => &self.bold,
            Face::Display => &self.display,
        }
    }

    /// The face for the hostel name: the display face when it has every
    /// letter of the name, else bold.
    fn name_face(&self, name: &str) -> Face {
        let display = self.get(Face::Display);
        if name
            .chars()
            .all(|c| display.lookup_glyph_index(c as u32).is_some())
        {
            Face::Display
        } else {
            Face::Bold
        }
    }

    /// Width of `text` at `size` pt, in mm (advance widths, no kerning).
    fn width(&self, face: Face, text: &str, size: f32) -> f32 {
        let font = self.get(face);
        let units: u32 = text
            .chars()
            .map(|c| {
                let glyph = font.lookup_glyph_index(c as u32).unwrap_or(0);
                u32::from(font.get_glyph_width(glyph).unwrap_or(0))
            })
            .sum();
        let em = f32::from(font.units_per_em.max(1));
        units as f32 / em * size * 25.4 / 72.0
    }

    /// `text` at `size` pt if it fits `max_w` mm; otherwise smaller, down to
    /// `min` pt, and then cut with an ellipsis.
    fn fit(&self, face: Face, text: &str, size: f32, min: f32, max_w: f32) -> (String, f32) {
        let mut size = size;
        while size > min && self.width(face, text, size) > max_w {
            size = (size - 0.5).max(min);
        }
        if self.width(face, text, size) <= max_w {
            return (text.to_string(), size);
        }
        let mut chars: Vec<char> = text.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let cut = format!("{}…", chars.iter().collect::<String>().trim_end());
            if self.width(face, &cut, size) <= max_w {
                return (cut, size);
            }
        }
        ("…".to_string(), size)
    }
}

// Page geometry, in mm: the template's margins on A4.
const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const LEFT: f32 = 27.4;
const RIGHT: f32 = 182.6;
/// Cell padding inside the table (7 pt).
const PAD: f32 = 2.5;
/// Indent of the contact details under "Received from:" / "Received by:".
const INDENT: f32 = 5.1;
/// Left edge of the amount column; the totals' labels end just before it.
const AMOUNT_LEFT: f32 = RIGHT - PAD - 53.3;
const LABEL_RIGHT: f32 = AMOUNT_LEFT - 4.2;
/// Left edge of the Date block (the template's 0.38-wide right column).
const DATE_LEFT: f32 = RIGHT - 0.38 * (RIGHT - LEFT);

/// Grey levels of the template: paper #F1F1EF, ink #161616, ink at 80% and 70%.
fn paper() -> Color {
    Color::Rgb(Rgb::new(241.0 / 255.0, 241.0 / 255.0, 239.0 / 255.0, None))
}
fn grey(level: u8) -> Color {
    let v = f32::from(level) / 255.0;
    Color::Rgb(Rgb::new(v, v, v, None))
}
const INK: u8 = 0x16;
const INK_80: u8 = 0x45;
const INK_70: u8 = 0x5C;

/// The page being drawn: its operations and the fonts they use.
struct Sheet {
    fonts: Fonts,
    ids: [FontId; 3],
    ops: Vec<Op>,
}

impl Sheet {
    fn id(&self, face: Face) -> FontId {
        self.ids[face as usize].clone()
    }

    /// One line of text with its baseline at `y` mm from the top of the page.
    fn text(&mut self, face: Face, size: f32, text: &str, x: f32, y: f32, level: u8) {
        self.ops.push(Op::StartTextSection);
        if let Face::Display = face {
            // The template thickens Italiana slightly (`embolden`): fill and outline.
            self.ops.extend([
                Op::SetTextRenderingMode {
                    mode: TextRenderingMode::FillStroke,
                },
                Op::SetOutlineColor { col: grey(level) },
                Op::SetOutlineThickness { pt: Pt(0.35) },
            ]);
        }
        self.ops.extend([
            Op::SetFillColor { col: grey(level) },
            Op::SetFont {
                font: PdfFontHandle::External(self.id(face)),
                size: Pt(size),
            },
            Op::SetTextCursor {
                pos: Point::new(Mm(x), Mm(PAGE_H - y)),
            },
            Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            },
        ]);
        if let Face::Display = face {
            self.ops.push(Op::SetTextRenderingMode {
                mode: TextRenderingMode::Fill,
            });
        }
        self.ops.push(Op::EndTextSection);
    }

    /// Text ending at `right` mm.
    fn text_right(&mut self, face: Face, size: f32, text: &str, right: f32, y: f32, level: u8) {
        let x = right - self.fonts.width(face, text, size);
        self.text(face, size, text, x, y, level);
    }

    /// Text fitted into `max_w` mm (smaller, then cut) starting at `x`.
    #[allow(clippy::too_many_arguments)]
    fn text_fit(
        &mut self,
        face: Face,
        size: f32,
        text: &str,
        x: f32,
        y: f32,
        max_w: f32,
        level: u8,
    ) {
        let (text, size) = self.fonts.fit(face, text, size, size * 0.75, max_w);
        self.text(face, size, &text, x, y, level);
    }

    /// A horizontal rule `y` mm from the top.
    fn rule(&mut self, x1: f32, x2: f32, y: f32, level: u8) {
        let point = |x| LinePoint {
            p: Point::new(Mm(x), Mm(PAGE_H - y)),
            bezier: false,
        };
        self.ops.extend([
            Op::SetOutlineColor { col: grey(level) },
            Op::SetOutlineThickness { pt: Pt(0.6) },
            Op::DrawLine {
                line: Line {
                    points: vec![point(x1), point(x2)],
                    is_closed: false,
                },
            },
        ]);
    }
}

/// Lay out the receipt (A4, one page).
fn build_receipt(payload: &ReceiptPayload) -> anyhow::Result<PdfDocument> {
    let mut doc = PdfDocument::new(&format!("Receipt {}", payload.receipt_number));
    let fonts = Fonts::load()?;
    let ids = [
        doc.add_font(&fonts.regular),
        doc.add_font(&fonts.bold),
        doc.add_font(&fonts.display),
    ];
    let mut page = Sheet {
        fonts,
        ids,
        ops: vec![
            Op::SetFillColor { col: paper() },
            Op::DrawRectangle {
                rectangle: Rect {
                    mode: Some(PaintMode::Fill),
                    ..Rect::from_xywh(
                        Mm(0.0).into(),
                        Mm(0.0).into(),
                        Mm(PAGE_W).into(),
                        Mm(PAGE_H).into(),
                    )
                },
            },
        ],
    };
    let present = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    // Header: the logo (the hostel's, else HostelDesk's) and the hostel's details.
    let (logo_w, logo_h) = (45.0, 33.0);
    let custom = payload
        .logo_path
        .as_deref()
        .and_then(|path| std::fs::read(path).ok());
    let placed = custom.as_deref().is_some_and(|bytes| {
        draw_logo(
            &mut doc,
            &mut page.ops,
            bytes,
            LEFT,
            PAGE_H - 22.9 - logo_h,
            logo_w,
            logo_h,
        )
        .is_ok()
    });
    if !placed {
        draw_logo(
            &mut doc,
            &mut page.ops,
            DEFAULT_LOGO,
            LEFT,
            PAGE_H - 22.9 - logo_h,
            logo_w,
            logo_h,
        )?;
    }
    let name_room = RIGHT - (LEFT + logo_w + 6.0);
    let face = page.fonts.name_face(&payload.hostel_name);
    let (name, size) = page
        .fonts
        .fit(face, &payload.hostel_name, 30.0, 16.0, name_room);
    page.text_right(face, size, &name, RIGHT, 35.9, INK);
    let mut y = 43.5;
    for line in [
        present(&payload.hostel_address),
        present(&payload.hostel_phone),
    ]
    .into_iter()
    .flatten()
    {
        let (line, size) = page.fonts.fit(Face::Regular, &line, 11.5, 9.0, name_room);
        page.text_right(Face::Regular, size, &line, RIGHT, y, INK);
        y += 5.4;
    }

    // Who paid (left) and the receipt number and date (right).
    let half = (RIGHT - LEFT) / 2.0 - INDENT;
    page.text(Face::Regular, 10.5, "Received from:", LEFT, 67.3, INK);
    let mut y = 74.3;
    for line in [
        Some(payload.resident_name.clone()),
        present(&payload.resident_phone),
        present(&payload.resident_email),
    ]
    .into_iter()
    .flatten()
    {
        page.text_fit(Face::Bold, 14.5, &line, LEFT + INDENT, y, half, INK);
        y += 6.7;
    }
    let number = format!(
        "Receipt No. {}",
        display_receipt_number(&payload.receipt_number)
    );
    page.text_right(Face::Regular, 13.5, &number, RIGHT, 66.7, INK);
    let date = payload.date.format("%-d %B %Y").to_string();
    page.text_right(Face::Regular, 12.0, &date, RIGHT, 72.4, INK);

    // The table: the reason, and the invoice it is paid against.
    page.rule(LEFT, RIGHT, 103.5, INK_80);
    page.text(Face::Bold, 11.5, "Reason", LEFT + PAD, 110.8, INK);
    let header = format!("Amount ({})", payload.currency_code);
    page.text_right(Face::Bold, 11.5, &header, RIGHT - PAD, 110.8, INK);
    page.rule(LEFT, RIGHT, 116.8, INK_80);
    let reason_w = AMOUNT_LEFT - PAD - (LEFT + PAD);
    page.text_fit(
        Face::Regular,
        11.5,
        &payload.reason(),
        LEFT + PAD,
        123.8,
        reason_w,
        INK,
    );
    if let Some(term) = payload.term_line() {
        page.text_fit(
            Face::Regular,
            11.5,
            &term,
            LEFT + PAD,
            129.2,
            reason_w,
            INK_70,
        );
    }
    page.text(Face::Regular, 13.5, "Invoice", AMOUNT_LEFT, 123.8, INK);
    let invoice = format_money(payload.invoice());
    page.text_right(Face::Regular, 11.5, &invoice, RIGHT - PAD, 123.8, INK);
    page.rule(LEFT, RIGHT, 135.3, INK_80);

    // The totals, as large as each other.
    page.text_right(Face::Bold, 19.0, "Amount Paid", LABEL_RIGHT, 142.5, INK);
    page.text_right(
        Face::Regular,
        19.0,
        &payload.money(payload.amount),
        RIGHT - PAD,
        142.5,
        INK,
    );
    page.text_right(Face::Bold, 19.0, "Balance", LABEL_RIGHT, 154.9, INK);
    page.text_right(
        Face::Regular,
        19.0,
        &payload.money(payload.balance),
        RIGHT - PAD,
        154.9,
        INK,
    );

    // Bottom, measured from the foot of the page: who received it, the date,
    // a line to sign, then the hostel's details.
    let from_bottom = |mm: f32| PAGE_H - mm;
    page.text(
        Face::Regular,
        10.5,
        "Received by:",
        LEFT,
        from_bottom(76.8),
        INK,
    );
    let mut y = from_bottom(70.5);
    for line in [
        present(&payload.received_by),
        present(&payload.received_by_contact),
    ]
    .into_iter()
    .flatten()
    {
        page.text_fit(
            Face::Bold,
            14.5,
            &line,
            LEFT + INDENT,
            y,
            DATE_LEFT - LEFT - INDENT - 4.0,
            INK,
        );
        y += 6.7;
    }
    let sign_w = 0.4 * (RIGHT - LEFT);
    page.rule(
        LEFT + INDENT,
        LEFT + INDENT + sign_w,
        from_bottom(46.4),
        INK,
    );
    page.text(
        Face::Regular,
        10.5,
        "Signature",
        LEFT + INDENT,
        from_bottom(41.0),
        INK,
    );
    page.text(
        Face::Regular,
        10.5,
        "Date:",
        DATE_LEFT,
        from_bottom(76.8),
        INK,
    );
    page.text(Face::Bold, 14.5, &date, DATE_LEFT, from_bottom(70.5), INK);

    if let Some(note) = present(&payload.footer_text) {
        let (note, size) = page
            .fonts
            .fit(Face::Regular, &note, 10.0, 8.0, RIGHT - LEFT);
        let x = (PAGE_W - page.fonts.width(Face::Regular, &note, size)) / 2.0;
        page.text(Face::Regular, size, &note, x, from_bottom(33.5), INK_80);
    }
    page.rule(LEFT, RIGHT, from_bottom(28.9), INK_80);
    let footer = [
        Some(payload.hostel_name.clone()),
        present(&payload.hostel_address),
        present(&payload.hostel_phone),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let (footer, size) = page
        .fonts
        .fit(Face::Regular, &footer, 10.0, 8.0, RIGHT - LEFT);
    let x = (PAGE_W - page.fonts.width(Face::Regular, &footer, size)) / 2.0;
    page.text(Face::Regular, size, &footer, x, from_bottom(22.9), INK_80);

    doc.with_pages(vec![PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), page.ops)]);
    Ok(doc)
}

/// Generate a PDF receipt at `path` from `payload`.
///
/// Overwrites any existing file at the path. The page size is fixed at A4.
pub fn generate_receipt(payload: &ReceiptPayload, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let bytes = build_receipt(payload)?.save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write(path.as_ref(), bytes)?;
    Ok(())
}

/// Decode and place a logo image so it fits within `max_w` x `max_h` mm, with
/// its top-left corner at `(x, y + max_h)` (PDF coordinates, from the bottom).
fn draw_logo(
    doc: &mut PdfDocument,
    ops: &mut Vec<Op>,
    bytes: &[u8],
    x: f32,
    y: f32,
    max_w: f32,
    max_h: f32,
) -> anyhow::Result<()> {
    let image = RawImage::decode_from_bytes(bytes, &mut Vec::new()).map_err(anyhow::Error::msg)?;
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

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn sample_payload() -> ReceiptPayload {
        ReceiptPayload {
            hostel_name: "Makerere Heights Hostel".to_string(),
            hostel_address: Some("Plot 14, Kikoni Road, Kampala".to_string()),
            hostel_phone: Some("+256 700 123 456".to_string()),
            receipt_number: "A-000142".to_string(),
            resident_name: "Sarah Namubiru".to_string(),
            resident_phone: Some("+256 772 418 905".to_string()),
            resident_email: Some("sarah.namubiru@gmail.com".to_string()),
            room_number: "B12".to_string(),
            room_type: Some("double".to_string()),
            term: Some("semester_1".to_string()),
            academic_year: Some("2026/2027".to_string()),
            amount: dec("500000"),
            date: NaiveDate::from_ymd_opt(2026, 11, 28).unwrap(),
            balance: dec("400000"),
            notes: None,
            currency_code: "UGX".to_string(),
            footer_text: None,
            logo_path: None,
            received_by: Some("Grace Akello".to_string()),
            received_by_contact: Some("grace@makerereheights.ug".to_string()),
        }
    }

    /// Whole amounts print as in the template (500,000); real cents keep two places.
    #[test]
    fn money_drops_zero_cents_and_keeps_real_ones() {
        assert_eq!(format_money(dec("500000")), "500,000");
        assert_eq!(format_money(dec("49500.50")), "49,500.50");
        assert_eq!(format_money(dec("1234.5")), "1,234.50");
        assert_eq!(format_money(dec("999.999")), "1,000");
        assert_eq!(format_money(dec("0")), "0");
    }

    /// The invoice line is what was owed before this payment.
    #[test]
    fn invoice_is_this_payment_plus_what_is_left() {
        assert_eq!(sample_payload().invoice(), dec("900000"));
    }

    #[test]
    fn reason_names_the_room_and_its_type_or_the_note() {
        let mut payload = sample_payload();
        assert_eq!(payload.reason(), "Rent, Room B12 (Double)");
        payload.room_type = None;
        payload.notes = Some("  June rent ".to_string());
        assert_eq!(payload.reason(), "June rent, Room B12");
    }

    #[test]
    fn term_line_reads_like_the_template() {
        let mut payload = sample_payload();
        assert_eq!(payload.term_line().as_deref(), Some("Semester 1, 2026/27"));
        payload.term = Some("recess".to_string());
        payload.academic_year = Some("2025/26".to_string());
        assert_eq!(payload.term_line().as_deref(), Some("Recess, 2025/26"));
        payload.term = None;
        assert_eq!(payload.term_line(), None);
    }

    /// Everything the template shows is on the page, worded as in the template.
    #[test]
    fn receipt_carries_the_template_text() {
        let doc = build_receipt(&sample_payload()).expect("build");
        let text = doc.extract_text().concat().join("\n");
        for expected in [
            "Makerere Heights Hostel",
            "Plot 14, Kikoni Road, Kampala",
            "Received from:",
            "Sarah Namubiru",
            "+256 772 418 905",
            "sarah.namubiru@gmail.com",
            "Receipt No. A-000142",
            "28 November 2026",
            "Reason",
            "Amount (UGX)",
            "Rent, Room B12 (Double)",
            "Semester 1, 2026/27",
            "Invoice",
            "900,000",
            "Amount Paid",
            "UGX 500,000",
            "Balance",
            "UGX 400,000",
            "Received by:",
            "Grace Akello",
            "Date:",
            "Signature",
        ] {
            assert!(
                text.contains(expected),
                "{expected:?} missing from:\n{text}"
            );
        }
    }

    /// The fonts are trimmed subsets, so a receipt stays small on disk.
    #[test]
    fn receipt_stays_small() {
        let path = std::env::temp_dir().join(format!("hosteldesk_size_{}.pdf", std::process::id()));
        generate_receipt(&sample_payload(), &path).expect("generate");
        let size = std::fs::metadata(&path).expect("pdf").len();
        let _ = std::fs::remove_file(&path);
        assert!(size < 300_000, "receipt is {size} bytes");
    }

    /// Names with accents (José, Müller, Łukasz) have glyphs in every face.
    #[test]
    fn fonts_cover_latin_names() {
        let fonts = Fonts::load().expect("fonts");
        // Italiana (the display face, used for the hostel name) has no euro sign.
        for (face, chars) in [
            (Face::Regular, "AZaz09·é€Łü"),
            (Face::Bold, "AZaz09·é€Łü"),
            (Face::Display, "AZaz09éüñ"),
        ] {
            for c in chars.chars() {
                assert!(
                    fonts.get(face).lookup_glyph_index(c as u32).is_some(),
                    "{face:?} has no {c}"
                );
            }
        }
    }

    /// Italiana covers Western European letters only; a hostel name it cannot
    /// draw in full is set in the bold face instead of showing empty boxes.
    #[test]
    fn hostel_name_falls_back_when_the_display_face_lacks_a_letter() {
        let fonts = Fonts::load().expect("fonts");
        assert!(matches!(
            fonts.name_face("Café Müller Hostel"),
            Face::Display
        ));
        assert!(matches!(fonts.name_face("Łódź Hostel"), Face::Bold));
    }

    /// Text too wide for its space shrinks, then ends in an ellipsis; it never overflows.
    #[test]
    fn long_text_is_fitted_to_its_space() {
        let fonts = Fonts::load().expect("fonts");
        let long = "The Very Long Name Of A Hostel That Goes On And On And On";
        let (text, size) = fonts.fit(Face::Display, long, 30.0, 16.0, 80.0);
        assert!(size < 30.0);
        assert!(fonts.width(Face::Display, &text, size) <= 80.0);
        assert!(text.ends_with('…'), "{text}");
        let (short, size) = fonts.fit(Face::Display, "Sunny", 30.0, 16.0, 80.0);
        assert_eq!((short.as_str(), size), ("Sunny", 30.0));
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
        let mut doc = PdfDocument::new("logo");
        let mut ops = Vec::new();
        draw_logo(&mut doc, &mut ops, &png, 20.0, 255.0, 22.0, 22.0).expect("logo placed");
        assert!(matches!(ops.as_slice(), [Op::UseXobject { .. }]));
    }

    /// A hostel without its own logo (or whose logo file has gone) gets the
    /// HostelDesk logo at the top of the receipt instead of none.
    #[test]
    fn receipts_fall_back_to_the_hostel_desk_logo() {
        let has_image = |logo_path: Option<&str>| {
            let mut payload = sample_payload();
            payload.logo_path = logo_path.map(str::to_string);
            let path = std::env::temp_dir().join(format!(
                "hosteldesk_receipt_logo_{}_{}.pdf",
                std::process::id(),
                logo_path.is_some()
            ));
            generate_receipt(&payload, &path).expect("generate receipt");
            let bytes = std::fs::read(&path).expect("read pdf");
            let _ = std::fs::remove_file(&path);
            bytes.windows(6).any(|w| w == b"/Image")
        };
        assert!(has_image(None), "no custom logo");
        assert!(
            has_image(Some("/nonexistent/logo.png")),
            "custom logo file missing"
        );
    }

    #[test]
    fn generates_valid_pdf_file() {
        let path = std::env::temp_dir().join(format!(
            "hosteldesk_receipt_test_{}.pdf",
            std::process::id()
        ));
        generate_receipt(&sample_payload(), &path).expect("generate receipt");
        let bytes = std::fs::read(&path).expect("read generated pdf");
        let _ = std::fs::remove_file(&path);
        assert!(bytes.starts_with(b"%PDF"), "not a PDF");
    }
}
