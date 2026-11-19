//! Factur-X XML: UN/CEFACT Cross Industry Invoice (D16B) for the EN 16931 profile,
//! built from a finalized invoice, deposit invoice or credit note.
//!
//! Business terms (BT/BG) refer to EN 16931-1. The XML follows the element order of
//! the Factur-X EN 16931 schema; no schema or Schematron validation runs here.
use crate::{Store, issuer::Issuer, quote::Quote};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use std::fmt::Write;

pub const GUIDELINE: &str = "urn:cen.eu:en16931:2017";
/// Name of the embedded file, fixed by the Factur-X specification.
pub const FILE_NAME: &str = "factur-x.xml";

/// A preceding invoice (BG-3).
pub struct Reference {
    pub number: String,
    /// `YYYY-MM-DD`.
    pub date: String,
}

pub struct Invoice<'a> {
    /// `invoice`, `deposit` or `credit`.
    pub kind: &'a str,
    pub number: &'a str,
    /// `YYYY-MM-DD`.
    pub issue_date: &'a str,
    pub currency: &'a str,
    pub language: &'a str,
    pub quote: &'a Quote,
    pub issuer: &'a Issuer,
    /// Deposits already invoiced and deducted from this invoice (BT-113).
    pub prepaid: &'a str,
    /// Source invoice of a credit note, or a deposit deducted from a final invoice.
    pub preceding: Option<Reference>,
}

impl Store {
    /// Factur-X XML of a finalized invoice, deposit or credit note, from the stored
    /// document, its frozen issuer and its deposit allocations.
    pub fn facturx_xml(&self, id: i64) -> Result<String> {
        let snapshot = self.document_snapshot(id, false)?;
        let state = &snapshot.billing;
        ensure!(
            matches!(state.kind.as_str(), "invoice" | "deposit" | "credit"),
            "Factur-X is only available for invoices, deposits and credit notes"
        );
        let number = state
            .number
            .as_deref()
            .context("Factur-X is only available for finalized documents")?;
        let issue_date = self.issue_date(id)?;
        let preceding_id: Option<i64> = if state.kind == "credit" {
            state.source_id
        } else {
            // ponytail: BG-3 carries one reference; further deposits only reduce BT-113.
            self.0
                .query_row(
                    "SELECT deposit_id FROM billing_allocation WHERE invoice_id=?1 ORDER BY deposit_id",
                    [id],
                    |r| r.get(0),
                )
                .optional()?
        };
        let preceding = match preceding_id {
            Some(source) => Some(Reference {
                number: self
                    .billing_state(source)?
                    .number
                    .context("Preceding invoice is not finalized")?,
                date: self.issue_date(source)?,
            }),
            None => None,
        };
        let prepaid = if state.kind == "invoice" {
            state.deposits.as_str()
        } else {
            "0"
        };
        cii_xml(&Invoice {
            kind: &state.kind,
            number,
            issue_date: &issue_date,
            currency: &snapshot.draft.currency,
            language: &snapshot.draft.language,
            quote: &snapshot.quote,
            issuer: &self.document_issuer(id)?,
            prepaid,
            preceding,
        })
    }

    /// Local issue date, as printed on the finalized PDF.
    fn issue_date(&self, id: i64) -> Result<String> {
        self.0
            .query_row(
                "SELECT date(issued_at,'localtime') FROM billing WHERE id=?1 AND number IS NOT NULL",
                [id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .context("Issue date unavailable")
    }
}

struct Group {
    category: &'static str,
    rate: Decimal,
    reasons: Vec<String>,
    /// Line nets before the global discount.
    lines: Decimal,
    basis: Decimal,
    tax: Decimal,
}

pub fn cii_xml(invoice: &Invoice<'_>) -> Result<String> {
    let quote = invoice.quote;
    let issuer = invoice.issuer;
    let currency = invoice.currency.trim().to_ascii_uppercase();
    let fr = invoice.language != "en";
    let type_code = match invoice.kind {
        "invoice" => "380",
        "deposit" => "386",
        "credit" => "381",
        _ => bail!("Factur-X is only available for invoices, deposits and credit notes"),
    };
    let (net, vat, gross) = quote.totals(&currency)?;
    let scale = decimal(&gross)?.scale();
    let amount = |value: Decimal| format!("{:.*}", scale as usize, value.round_dp(scale));
    let line_nets = quote.line_nets(&currency)?;

    // VAT breakdown, one group per category and rate (BR-E-01, BR-Z-01, BR-S-08).
    let mut groups: Vec<Group> = Vec::new();
    let mut line_categories = Vec::new();
    for (line, line_net) in quote.lines.iter().zip(&line_nets) {
        let rate = decimal(&line.vat)?.normalize();
        let index = group(&mut groups, category(rate, &line.exemption_reason), rate);
        line_categories.push(index);
        groups[index].lines += decimal(line_net)?;
    }
    for bucket in quote.tax_breakdown(&currency)? {
        let rate = decimal(&bucket.rate)?.normalize();
        let index = group(&mut groups, category(rate, &bucket.exemption_reason), rate);
        let target = &mut groups[index];
        target.basis += decimal(&bucket.net)?;
        target.tax += decimal(&bucket.tax)?;
        if !bucket.exemption_reason.is_empty() {
            target.reasons.push(bucket.exemption_reason);
        }
    }

    let line_total: Decimal = groups.iter().map(|g| g.lines).sum();
    let allowance_total = line_total - decimal(&net)?;
    let prepaid = decimal(invoice.prepaid)?;
    let due = decimal(&gross)? - prepaid;

    let mut x = String::with_capacity(8192);
    x.push_str(concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "\n",
        r#"<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100" xmlns:qdt="urn:un:unece:uncefact:data:standard:QualifiedDataType:100" xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100" xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">"#,
        "\n"
    ));
    writeln!(
        x,
        "<rsm:ExchangedDocumentContext><ram:GuidelineSpecifiedDocumentContextParameter><ram:ID>{GUIDELINE}</ram:ID></ram:GuidelineSpecifiedDocumentContextParameter></rsm:ExchangedDocumentContext>"
    )?;
    // BT-1, BT-3, BT-2, BG-1.
    write!(
        x,
        "<rsm:ExchangedDocument><ram:ID>{}</ram:ID><ram:TypeCode>{type_code}</ram:TypeCode><ram:IssueDateTime><udt:DateTimeString format=\"102\">{}</udt:DateTimeString></ram:IssueDateTime>",
        escape(invoice.number),
        date102(invoice.issue_date)?
    )?;
    let mut note = |content: &str, subject: Option<&str>| {
        if !content.trim().is_empty() {
            x.push_str("<ram:IncludedNote><ram:Content>");
            x.push_str(&escape(&plain(content)));
            x.push_str("</ram:Content>");
            if let Some(code) = subject {
                x.push_str("<ram:SubjectCode>");
                x.push_str(code);
                x.push_str("</ram:SubjectCode>");
            }
            x.push_str("</ram:IncludedNote>");
        }
    };
    note(&quote.terms, None);
    let service_date = quote.service_date.trim();
    let service_date_iso = date102(service_date).ok();
    if service_date_iso.is_none() && !service_date.is_empty() {
        let label = if fr {
            "Date de prestation"
        } else {
            "Service date"
        };
        note(&format!("{label} : {service_date}"), None);
    }
    if invoice.kind != "credit" {
        note(issuer.late_penalties(invoice.language), Some("PMD"));
        note(issuer.late_penalties(invoice.language), Some("PMT"));
        note(issuer.late_penalties(invoice.language), Some("AAB"));
    }
    x.push_str("</rsm:ExchangedDocument>\n<rsm:SupplyChainTradeTransaction>\n");

    // BG-25 invoice lines.
    for (index, ((line, line_net), group)) in quote
        .lines
        .iter()
        .zip(&line_nets)
        .zip(&line_categories)
        .enumerate()
    {
        let mut description = line.description.lines();
        let name = plain(description.next().unwrap_or_default());
        let details = description
            .map(plain)
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let quantity = decimal(&line.quantity)?;
        let price = decimal(&line.price)?;
        let group = &groups[*group];
        write!(
            x,
            "<ram:IncludedSupplyChainTradeLineItem><ram:AssociatedDocumentLineDocument><ram:LineID>{}</ram:LineID></ram:AssociatedDocumentLineDocument><ram:SpecifiedTradeProduct><ram:Name>{}</ram:Name>",
            index + 1,
            escape(&name)
        )?;
        if !details.is_empty() {
            write!(x, "<ram:Description>{}</ram:Description>", escape(&details))?;
        }
        write!(
            x,
            "</ram:SpecifiedTradeProduct><ram:SpecifiedLineTradeAgreement><ram:NetPriceProductTradePrice><ram:ChargeAmount>{}</ram:ChargeAmount></ram:NetPriceProductTradePrice></ram:SpecifiedLineTradeAgreement><ram:SpecifiedLineTradeDelivery><ram:BilledQuantity unitCode=\"{}\">{}</ram:BilledQuantity></ram:SpecifiedLineTradeDelivery><ram:SpecifiedLineTradeSettlement><ram:ApplicableTradeTax><ram:TypeCode>VAT</ram:TypeCode><ram:CategoryCode>{}</ram:CategoryCode><ram:RateApplicablePercent>{}</ram:RateApplicablePercent></ram:ApplicableTradeTax>",
            price.normalize(),
            unit_code(&line.unit),
            quantity.normalize(),
            group.category,
            group.rate,
        )?;
        // Line discount as a line allowance (BG-27), so that BT-131 = BT-129 × BT-146 − BT-136.
        let line_net = decimal(line_net)?;
        let undiscounted = (quantity * price).round_dp(scale);
        if undiscounted > line_net {
            let reason = if fr { "Remise" } else { "Discount" };
            write!(
                x,
                "<ram:SpecifiedTradeAllowanceCharge><ram:ChargeIndicator><udt:Indicator>false</udt:Indicator></ram:ChargeIndicator><ram:ActualAmount>{}</ram:ActualAmount><ram:ReasonCode>95</ram:ReasonCode><ram:Reason>{reason} {} %</ram:Reason></ram:SpecifiedTradeAllowanceCharge>",
                amount(undiscounted - line_net),
                decimal(&line.discount)?.normalize()
            )?;
        }
        writeln!(
            x,
            "<ram:SpecifiedTradeSettlementLineMonetarySummation><ram:LineTotalAmount>{}</ram:LineTotalAmount></ram:SpecifiedTradeSettlementLineMonetarySummation></ram:SpecifiedLineTradeSettlement></ram:IncludedSupplyChainTradeLineItem>",
            amount(line_net)
        )?;
    }

    // BG-4 seller, BG-7 buyer.
    x.push_str("<ram:ApplicableHeaderTradeAgreement>");
    party(
        &mut x,
        "SellerTradeParty",
        &Party {
            name: &issuer.name,
            registration: &issuer.registration,
            contact: &issuer.contact,
            phone: &issuer.phone,
            email: &issuer.email,
            address: &issuer.address,
            country: &issuer.country,
            vat_id: &issuer.vat_id,
        },
    )?;
    party(
        &mut x,
        "BuyerTradeParty",
        &Party {
            name: &quote.recipient,
            registration: &quote.recipient_registration,
            contact: "",
            phone: "",
            email: &quote.recipient_email,
            address: &quote.recipient_address,
            country: &quote.recipient_country,
            vat_id: "",
        },
    )?;
    x.push_str("</ram:ApplicableHeaderTradeAgreement>\n<ram:ApplicableHeaderTradeDelivery>");
    if let Some(date) = service_date_iso {
        // BT-72.
        write!(
            x,
            "<ram:ActualDeliverySupplyChainEvent><ram:OccurrenceDateTime><udt:DateTimeString format=\"102\">{date}</udt:DateTimeString></ram:OccurrenceDateTime></ram:ActualDeliverySupplyChainEvent>"
        )?;
    }
    x.push_str("</ram:ApplicableHeaderTradeDelivery>\n<ram:ApplicableHeaderTradeSettlement>");
    // BT-5.
    write!(
        x,
        "<ram:InvoiceCurrencyCode>{}</ram:InvoiceCurrencyCode>",
        escape(&currency)
    )?;
    let method = first_filled(&quote.payment_method, &issuer.payment_method);
    if let Some(code) = payment_means(method) {
        // BG-16: BT-81, BT-82.
        write!(
            x,
            "<ram:SpecifiedTradeSettlementPaymentMeans><ram:TypeCode>{code}</ram:TypeCode><ram:Information>{}</ram:Information>",
            escape(method.trim())
        )?;
        if matches!(code, "30" | "58") {
            let iban: String = issuer
                .iban
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_uppercase();
            ensure!(
                valid_iban(&iban),
                "Factur-X bank transfers require a valid creditor IBAN in issuer settings"
            );
            write!(
                x,
                "<ram:PayeePartyCreditorFinancialAccount><ram:IBANID>{iban}</ram:IBANID></ram:PayeePartyCreditorFinancialAccount>"
            )?;
        }
        x.push_str("</ram:SpecifiedTradeSettlementPaymentMeans>");
    }
    // BG-23: BT-117, BT-120, BT-116, BT-118, BT-121, BT-119.
    for group in &groups {
        write!(
            x,
            "<ram:ApplicableTradeTax><ram:CalculatedAmount>{}</ram:CalculatedAmount><ram:TypeCode>VAT</ram:TypeCode>",
            amount(group.tax)
        )?;
        let reason = group.reasons.join(" ; ");
        if group.category == "E" {
            write!(
                x,
                "<ram:ExemptionReason>{}</ram:ExemptionReason>",
                escape(&reason)
            )?;
        }
        write!(
            x,
            "<ram:BasisAmount>{}</ram:BasisAmount><ram:CategoryCode>{}</ram:CategoryCode>",
            amount(group.basis),
            group.category
        )?;
        if group.category == "E" && group.reasons.len() == 1 && is_franchise(&reason) {
            x.push_str("<ram:ExemptionReasonCode>VATEX-FR-FRANCHISE</ram:ExemptionReasonCode>");
        }
        write!(
            x,
            "<ram:RateApplicablePercent>{}</ram:RateApplicablePercent></ram:ApplicableTradeTax>",
            group.rate
        )?;
    }
    // BG-20: the global discount, split by VAT category as allocated on the lines.
    let discount = quote.global_discount.trim();
    for group in &groups {
        let value = group.lines - group.basis;
        if value > Decimal::ZERO {
            let reason = if fr {
                "Remise globale"
            } else {
                "Global discount"
            };
            write!(
                x,
                "<ram:SpecifiedTradeAllowanceCharge><ram:ChargeIndicator><udt:Indicator>false</udt:Indicator></ram:ChargeIndicator><ram:ActualAmount>{}</ram:ActualAmount><ram:ReasonCode>95</ram:ReasonCode><ram:Reason>{reason} {} %</ram:Reason><ram:CategoryTradeTax><ram:TypeCode>VAT</ram:TypeCode><ram:CategoryCode>{}</ram:CategoryCode><ram:RateApplicablePercent>{}</ram:RateApplicablePercent></ram:CategoryTradeTax></ram:SpecifiedTradeAllowanceCharge>",
                amount(value),
                decimal(discount)?.normalize(),
                group.category,
                group.rate
            )?;
        }
    }
    // BT-20, BT-9; BR-CO-25 needs one of them when an amount is due.
    let mut terms = first_filled(&quote.payment_terms, &issuer.payment_terms)
        .trim()
        .to_string();
    let due_date = date102(quote.due_date.trim()).ok();
    if terms.is_empty() && due_date.is_none() && invoice.kind == "credit" {
        terms = if fr {
            "Avoir à déduire ou à rembourser"
        } else {
            "Credit to be deducted or refunded"
        }
        .into();
    }
    if !terms.is_empty() || due_date.is_some() {
        x.push_str("<ram:SpecifiedTradePaymentTerms>");
        if !terms.is_empty() {
            write!(x, "<ram:Description>{}</ram:Description>", escape(&terms))?;
        }
        if let Some(date) = due_date {
            write!(
                x,
                "<ram:DueDateDateTime><udt:DateTimeString format=\"102\">{date}</udt:DateTimeString></ram:DueDateDateTime>"
            )?;
        }
        x.push_str("</ram:SpecifiedTradePaymentTerms>");
    }
    // BG-22: BT-106, BT-107, BT-109, BT-110, BT-112, BT-113, BT-115.
    write!(
        x,
        "<ram:SpecifiedTradeSettlementHeaderMonetarySummation><ram:LineTotalAmount>{}</ram:LineTotalAmount>",
        amount(line_total)
    )?;
    if allowance_total > Decimal::ZERO {
        write!(
            x,
            "<ram:AllowanceTotalAmount>{}</ram:AllowanceTotalAmount>",
            amount(allowance_total)
        )?;
    }
    write!(
        x,
        "<ram:TaxBasisTotalAmount>{net}</ram:TaxBasisTotalAmount><ram:TaxTotalAmount currencyID=\"{}\">{vat}</ram:TaxTotalAmount><ram:GrandTotalAmount>{gross}</ram:GrandTotalAmount>",
        escape(&currency)
    )?;
    if prepaid > Decimal::ZERO {
        write!(
            x,
            "<ram:TotalPrepaidAmount>{}</ram:TotalPrepaidAmount>",
            amount(prepaid)
        )?;
    }
    write!(
        x,
        "<ram:DuePayableAmount>{}</ram:DuePayableAmount></ram:SpecifiedTradeSettlementHeaderMonetarySummation>",
        amount(due)
    )?;
    if let Some(reference) = &invoice.preceding {
        // BT-25, BT-26.
        write!(
            x,
            "<ram:InvoiceReferencedDocument><ram:IssuerAssignedID>{}</ram:IssuerAssignedID><ram:FormattedIssueDateTime><qdt:DateTimeString format=\"102\">{}</qdt:DateTimeString></ram:FormattedIssueDateTime></ram:InvoiceReferencedDocument>",
            escape(&reference.number),
            date102(&reference.date)?
        )?;
    }
    x.push_str("</ram:ApplicableHeaderTradeSettlement>\n</rsm:SupplyChainTradeTransaction>\n</rsm:CrossIndustryInvoice>\n");
    Ok(x)
}

fn group(groups: &mut Vec<Group>, category: &'static str, rate: Decimal) -> usize {
    if let Some(index) = groups
        .iter()
        .position(|g| g.category == category && g.rate == rate)
    {
        return index;
    }
    groups.push(Group {
        category,
        rate,
        reasons: Vec::new(),
        lines: Decimal::ZERO,
        basis: Decimal::ZERO,
        tax: Decimal::ZERO,
    });
    groups.len() - 1
}

struct Party<'a> {
    name: &'a str,
    registration: &'a str,
    contact: &'a str,
    phone: &'a str,
    email: &'a str,
    address: &'a str,
    country: &'a str,
    vat_id: &'a str,
}

fn valid_iban(iban: &str) -> bool {
    let bytes = iban.as_bytes();
    if !(15..=34).contains(&bytes.len())
        || !bytes[..2].iter().all(u8::is_ascii_uppercase)
        || !bytes[2..4].iter().all(u8::is_ascii_digit)
        || !bytes.iter().all(u8::is_ascii_alphanumeric)
    {
        return false;
    }
    let mut remainder: u32 = 0;
    for &byte in bytes[4..].iter().chain(&bytes[..4]) {
        if byte.is_ascii_digit() {
            remainder = (remainder * 10 + (byte - b'0') as u32) % 97;
        } else {
            remainder = (remainder * 100 + (byte - b'A' + 10) as u32) % 97;
        }
    }
    remainder == 1
}

fn party(x: &mut String, tag: &str, party: &Party<'_>) -> Result<()> {
    ensure!(!party.name.trim().is_empty(), "{tag} needs a name");
    write!(
        x,
        "<ram:{tag}><ram:Name>{}</ram:Name>",
        escape(party.name.trim())
    )?;
    // BT-30 / BT-47: SIREN (scheme 0002) or SIRET (0009).
    let registration: String = party
        .registration
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if !registration.is_empty() {
        let scheme = match registration.len() {
            9 if registration.bytes().all(|b| b.is_ascii_digit()) => " schemeID=\"0002\"",
            14 if registration.bytes().all(|b| b.is_ascii_digit()) => " schemeID=\"0009\"",
            _ => "",
        };
        write!(
            x,
            "<ram:SpecifiedLegalOrganization><ram:ID{scheme}>{}</ram:ID></ram:SpecifiedLegalOrganization>",
            escape(&registration)
        )?;
    }
    // BG-6: BT-41, BT-42.
    if !party.contact.trim().is_empty() || !party.phone.trim().is_empty() {
        x.push_str("<ram:DefinedTradeContact>");
        if !party.contact.trim().is_empty() {
            write!(
                x,
                "<ram:PersonName>{}</ram:PersonName>",
                escape(party.contact.trim())
            )?;
        }
        if !party.phone.trim().is_empty() {
            write!(
                x,
                "<ram:TelephoneUniversalCommunication><ram:CompleteNumber>{}</ram:CompleteNumber></ram:TelephoneUniversalCommunication>",
                escape(party.phone.trim())
            )?;
        }
        x.push_str("</ram:DefinedTradeContact>");
    }
    // BG-5 / BG-8.
    let address = Address::parse(party.address, party.country);
    x.push_str("<ram:PostalTradeAddress>");
    if !address.postcode.is_empty() {
        write!(
            x,
            "<ram:PostcodeCode>{}</ram:PostcodeCode>",
            escape(&address.postcode)
        )?;
    }
    for (line, tag) in address
        .lines
        .iter()
        .zip(["LineOne", "LineTwo", "LineThree"])
    {
        write!(x, "<ram:{tag}>{}</ram:{tag}>", escape(line))?;
    }
    if !address.city.is_empty() {
        write!(x, "<ram:CityName>{}</ram:CityName>", escape(&address.city))?;
    }
    write!(
        x,
        "<ram:CountryID>{}</ram:CountryID></ram:PostalTradeAddress>",
        address.country
    )?;
    // BT-34 / BT-49.
    if !party.email.trim().is_empty() {
        write!(
            x,
            "<ram:URIUniversalCommunication><ram:URIID schemeID=\"EM\">{}</ram:URIID></ram:URIUniversalCommunication>",
            escape(party.email.trim())
        )?;
    }
    // BT-31.
    let vat_id: String = party
        .vat_id
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if !vat_id.is_empty() {
        write!(
            x,
            "<ram:SpecifiedTaxRegistration><ram:ID schemeID=\"VA\">{}</ram:ID></ram:SpecifiedTaxRegistration>",
            escape(&vat_id)
        )?;
    }
    // BT-32: French fiscal registration is the SIREN (first nine digits of SIRET).
    if tag == "SellerTradeParty"
        && country_code(party.country) == "FR"
        && matches!(registration.len(), 9 | 14)
        && registration.bytes().all(|b| b.is_ascii_digit())
    {
        write!(
            x,
            "<ram:SpecifiedTaxRegistration><ram:ID schemeID=\"FC\">{}</ram:ID></ram:SpecifiedTaxRegistration>",
            &registration[..9]
        )?;
    }
    write!(x, "</ram:{tag}>")?;
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Address {
    postcode: String,
    /// Street lines, at most three (BT-35, BT-36, BT-162).
    lines: Vec<String>,
    city: String,
    country: &'static str,
}

impl Address {
    /// ponytail: heuristic, the last line is "<postcode> <city>" when its first word has
    /// a digit (e.g. "75001 PARIS"); a structured address form would remove the guess.
    fn parse(text: &str, country: &str) -> Self {
        let country_code = country_code(country);
        let mut lines: Vec<String> = text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        // A trailing country line duplicates BT-40 / BT-55.
        if lines.len() > 1
            && lines.last().is_some_and(|l| {
                l.eq_ignore_ascii_case(country.trim()) || known_country(l).is_some()
            })
        {
            lines.pop();
        }
        let (postcode, city) = match lines.pop() {
            Some(last) => match last.split_once(' ') {
                Some((code, city)) if code.chars().any(|c| c.is_ascii_digit()) => {
                    (code.to_string(), city.trim().to_string())
                }
                _ => (String::new(), last),
            },
            None => (String::new(), String::new()),
        };
        if lines.len() > 3 {
            let rest = lines.split_off(2).join(", ");
            lines.push(rest);
        }
        Self {
            postcode,
            lines,
            city,
            country: country_code,
        }
    }
}

/// ISO 3166-1 alpha-2 code from a country name in French or English; defaults to FR.
fn country_code(name: &str) -> &'static str {
    known_country(name).unwrap_or("FR")
}

fn known_country(name: &str) -> Option<&'static str> {
    let key: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ü' => 'u',
            'ç' => 'c',
            '-' | '’' | '\'' => ' ',
            c => c,
        })
        .collect();
    const TABLE: &[(&str, &[&str])] = &[
        ("FR", &["fr", "france", "republique francaise"]),
        ("BE", &["be", "belgique", "belgium"]),
        ("CH", &["ch", "suisse", "switzerland"]),
        ("LU", &["lu", "luxembourg"]),
        ("MC", &["mc", "monaco"]),
        ("DE", &["de", "allemagne", "germany"]),
        ("ES", &["es", "espagne", "spain"]),
        ("IT", &["it", "italie", "italy"]),
        ("PT", &["pt", "portugal"]),
        ("NL", &["nl", "pays bas", "netherlands"]),
        ("AT", &["at", "autriche", "austria"]),
        ("IE", &["ie", "irlande", "ireland"]),
        (
            "GB",
            &["gb", "uk", "royaume uni", "united kingdom", "angleterre"],
        ),
        (
            "US",
            &[
                "us",
                "usa",
                "etats unis",
                "united states",
                "etats unis d amerique",
            ],
        ),
        ("CA", &["ca", "canada"]),
    ];
    TABLE
        .iter()
        .find(|(_, names)| names.contains(&key.as_str()))
        .map(|(code, _)| *code)
}

/// UN/ECE Recommendation 20 unit code; C62 ("one") when the unit is not recognised.
fn unit_code(unit: &str) -> &'static str {
    let unit = unit.trim().to_lowercase();
    match unit.trim_end_matches('s') {
        "jour" | "j" | "day" | "journee" | "journée" => "DAY",
        "heure" | "h" | "hour" => "HUR",
        "minute" | "min" => "MIN",
        "semaine" | "week" => "WEE",
        "moi" | "mois" | "month" => "MON",
        "an" | "annee" | "année" | "year" => "ANN",
        "forfait" | "lump sum" => "LS",
        "kg" => "KGM",
        "km" => "KMT",
        "m" => "MTR",
        "m2" | "m²" => "MTK",
        _ => "C62",
    }
}

/// UNCL 4461 payment means code, when the method is recognisable.
fn payment_means(method: &str) -> Option<&'static str> {
    let method = method.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| method.contains(w));
    if has(&["prélèvement", "prelevement", "direct debit"]) {
        Some("59")
    } else if has(&["virement", "transfer", "iban"]) {
        Some("30")
    } else if has(&["chèque", "cheque", "check"]) {
        Some("20")
    } else if has(&["carte", "card"]) {
        Some("48")
    } else if has(&["espèces", "especes", "cash"]) {
        Some("10")
    } else {
        None
    }
}

fn category(rate: Decimal, reason: &str) -> &'static str {
    if rate > Decimal::ZERO {
        "S"
    } else if !reason.trim().is_empty() {
        "E"
    } else {
        "Z"
    }
}

/// VAT franchise of small businesses (art. 293 B du CGI).
fn is_franchise(reason: &str) -> bool {
    reason.to_lowercase().replace(' ', "").contains("293b")
}

fn first_filled<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn decimal(value: &str) -> Result<Decimal> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(Decimal::ZERO);
    }
    Ok(Decimal::from_str_exact(&value.replace(',', "."))?)
}

/// `YYYY-MM-DD` to the `102` format `YYYYMMDD`.
fn date102(iso: &str) -> Result<String> {
    let bytes = iso.as_bytes();
    ensure!(
        bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && iso.replace('-', "").bytes().all(|b| b.is_ascii_digit()),
        "Expected a YYYY-MM-DD date"
    );
    Ok(iso.replace('-', ""))
}

/// Removes the light markup of descriptions (`**bold**`, `# heading`).
fn plain(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = line.trim();
            line.strip_prefix("# ").unwrap_or(line).replace("**", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters other than tab and newlines are not allowed in XML 1.0.
            c if c.is_control() && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::Line;

    /// Text of every `<tag …>text</tag>` element, in document order.
    fn values(xml: &str, tag: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = xml;
        while let Some(start) = rest.find(&format!("<{tag}")) {
            let after = &rest[start + tag.len() + 1..];
            if !after.starts_with(['>', ' ']) {
                rest = after;
                continue;
            }
            let open = after.find('>').unwrap() + 1;
            let close = after.find(&format!("</{tag}>")).unwrap();
            out.push(after[open..close].to_string());
            rest = &after[close..];
        }
        out
    }

    fn value(xml: &str, tag: &str) -> String {
        values(xml, tag).into_iter().next().unwrap_or_default()
    }

    fn sum(xml: &str, tag: &str) -> Decimal {
        values(xml, tag)
            .iter()
            .map(|v| Decimal::from_str_exact(v).unwrap())
            .sum()
    }

    fn issuer() -> Issuer {
        Issuer {
            name: "Studio Exemple".into(),
            address: "12 chemin des Essais\n75001 PARIS".into(),
            country: "France".into(),
            registration: "123 456 789".into(),
            email: "contact@example.test".into(),
            payment_terms: "30 jours".into(),
            payment_method: "Virement bancaire".into(),
            iban: "FR1420041010050500013M02606".into(),
            ..Default::default()
        }
    }

    fn invoice<'a>(quote: &'a Quote, issuer: &'a Issuer) -> Invoice<'a> {
        Invoice {
            kind: "invoice",
            number: "F112600004",
            issue_date: "2026-11-18",
            currency: "EUR",
            language: "fr",
            quote,
            issuer,
            prepaid: "0",
            preceding: None,
        }
    }

    #[test]
    fn bank_transfer_requires_a_real_checksum_valid_account() -> Result<()> {
        assert!(valid_iban("FR1420041010050500013M02606"));
        assert!(!valid_iban("FR1420041010050500013M02607"));
        assert!(!valid_iban("FR"));
        let mut issuer = issuer();
        issuer.iban.clear();
        let quote = Quote {
            recipient: "Client".into(),
            recipient_country: "France".into(),
            lines: vec![Line {
                description: "Service".into(),
                price: "100".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(
            cii_xml(&invoice(&quote, &issuer))
                .unwrap_err()
                .to_string()
                .contains("creditor IBAN")
        );
        Ok(())
    }

    fn reconciles(xml: &str, quote: &Quote) {
        let (net, vat, gross) = quote.totals("EUR").unwrap();
        let line_total = sum(xml, "ram:LineTotalAmount");
        // Header BT-106 is the last LineTotalAmount; the others are the BT-131 line nets.
        let header =
            Decimal::from_str_exact(&values(xml, "ram:LineTotalAmount").pop().unwrap()).unwrap();
        assert_eq!(line_total - header, header, "BT-106 = Σ BT-131");
        let allowances =
            Decimal::from_str_exact(&value(xml, "ram:AllowanceTotalAmount")).unwrap_or_default();
        assert_eq!(value(xml, "ram:TaxBasisTotalAmount"), net, "BT-109");
        assert_eq!(header - allowances, Decimal::from_str_exact(&net).unwrap());
        assert_eq!(value(xml, "ram:TaxTotalAmount"), vat, "BT-110");
        assert!(xml.contains("<ram:TaxTotalAmount currencyID=\"EUR\">"));
        assert_eq!(value(xml, "ram:GrandTotalAmount"), gross, "BT-112");
        assert_eq!(
            sum(xml, "ram:BasisAmount"),
            Decimal::from_str_exact(&net).unwrap(),
            "Σ BT-116"
        );
        assert_eq!(
            sum(xml, "ram:CalculatedAmount"),
            Decimal::from_str_exact(&vat).unwrap(),
            "Σ BT-117"
        );
    }

    #[test]
    fn franchise_invoice_uses_exempt_category_and_parsed_address() -> Result<()> {
        let issuer = issuer();
        let quote = Quote {
            recipient: "Client & Fils <SARL>".into(),
            recipient_address: "1 rue Exemple\nBâtiment B\n75001 Paris\nFrance".into(),
            recipient_country: "France".into(),
            recipient_registration: "987654321".into(),
            due_date: "2026-12-18".into(),
            service_date: "2026-11-10".into(),
            lines: vec![Line {
                description:
                    "# Mission **conseil**\nAtelier de cadrage\n\n**Livrable** : compte rendu"
                        .into(),
                quantity: "2,5".into(),
                price: "400".into(),
                unit: "jours".into(),
                exemption_reason: "TVA non applicable, art. 293 B du CGI".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let xml = cii_xml(&invoice(&quote, &issuer))?;
        assert_eq!(value(&xml, "ram:ID"), GUIDELINE);
        assert!(xml.contains("<ram:ID>F112600004</ram:ID>"), "BT-1");
        assert_eq!(value(&xml, "ram:TypeCode"), "380", "BT-3");
        assert_eq!(value(&xml, "udt:DateTimeString"), "20261118", "BT-2");
        assert_eq!(value(&xml, "ram:InvoiceCurrencyCode"), "EUR", "BT-5");
        assert_eq!(value(&xml, "ram:Name"), "Mission conseil", "BT-153");
        assert_eq!(
            value(&xml, "ram:Description"),
            "Atelier de cadrage\nLivrable : compte rendu",
            "BT-154"
        );
        assert!(xml.contains("<ram:BilledQuantity unitCode=\"DAY\">2.5</ram:BilledQuantity>"));
        let seller = value(&xml, "ram:SellerTradeParty");
        assert_eq!(value(&seller, "ram:Name"), "Studio Exemple", "BT-27");
        assert!(
            seller.contains("<ram:ID schemeID=\"0002\">123456789</ram:ID>"),
            "BT-30"
        );
        assert_eq!(value(&seller, "ram:PostcodeCode"), "75001");
        assert_eq!(value(&seller, "ram:CityName"), "PARIS");
        assert_eq!(value(&seller, "ram:LineOne"), "12 chemin des Essais");
        assert_eq!(value(&seller, "ram:CountryID"), "FR");
        assert!(seller.contains("schemeID=\"FC\">123456789"));
        let buyer = value(&xml, "ram:BuyerTradeParty");
        assert_eq!(
            value(&buyer, "ram:Name"),
            "Client &amp; Fils &lt;SARL&gt;",
            "BT-44"
        );
        assert_eq!(value(&buyer, "ram:LineTwo"), "Bâtiment B");
        assert_eq!(value(&buyer, "ram:CityName"), "Paris");
        assert!(!buyer.contains("ram:LineThree"));
        let tax = values(&xml, "ram:ApplicableTradeTax").pop().unwrap();
        assert_eq!(value(&tax, "ram:CategoryCode"), "E", "BT-118");
        assert_eq!(value(&tax, "ram:ExemptionReasonCode"), "VATEX-FR-FRANCHISE");
        assert_eq!(
            value(&tax, "ram:ExemptionReason"),
            "TVA non applicable, art. 293 B du CGI"
        );
        assert_eq!(value(&tax, "ram:RateApplicablePercent"), "0");
        assert!(xml.contains("<udt:DateTimeString format=\"102\">20261110</udt:DateTimeString>"));
        let terms = value(&xml, "ram:SpecifiedTradePaymentTerms");
        assert_eq!(value(&terms, "ram:Description"), "30 jours", "BT-20");
        assert!(terms.contains("20261218"), "BT-9");
        assert_eq!(value(&xml, "ram:TypeCode").as_str(), "380");
        assert!(xml.contains("<ram:SubjectCode>PMD</ram:SubjectCode>"));
        assert_eq!(value(&xml, "ram:DuePayableAmount"), "1000.00", "BT-115");
        assert!(!xml.contains("TotalPrepaidAmount"));
        reconciles(&xml, &quote);
        Ok(())
    }

    #[test]
    fn vat_invoice_with_line_and_global_discounts_reconciles_to_the_cent() -> Result<()> {
        let issuer = Issuer {
            vat_id: "FR 12 123456789".into(),
            ..issuer()
        };
        let quote = Quote {
            recipient: "Client TVA".into(),
            recipient_country: "Belgique".into(),
            recipient_address: "Rue du Test 5\n1000 Bruxelles".into(),
            due_date: "2026-12-18".into(),
            global_discount: "7.5".into(),
            lines: vec![
                Line {
                    description: "Développement".into(),
                    quantity: "3".into(),
                    price: "333.33".into(),
                    discount: "10".into(),
                    vat: "20".into(),
                    unit: "heure".into(),
                    ..Default::default()
                },
                Line {
                    description: "Livre".into(),
                    quantity: "7".into(),
                    price: "12.99".into(),
                    vat: "5,5".into(),
                    unit: "forfait".into(),
                    ..Default::default()
                },
                Line {
                    description: "Débours".into(),
                    price: "19.99".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let xml = cii_xml(&invoice(&quote, &issuer))?;
        assert!(
            xml.contains("<ram:ID schemeID=\"VA\">FR12123456789</ram:ID>"),
            "BT-31"
        );
        let taxes = values(&xml, "ram:ApplicableTradeTax");
        let breakdown = &taxes[3..];
        assert_eq!(breakdown.len(), 3, "BG-23 per category and rate");
        assert_eq!(value(&breakdown[0], "ram:CategoryCode"), "S");
        assert_eq!(value(&breakdown[0], "ram:RateApplicablePercent"), "20");
        assert_eq!(value(&breakdown[1], "ram:RateApplicablePercent"), "5.5");
        assert_eq!(value(&breakdown[2], "ram:CategoryCode"), "Z");
        assert!(!breakdown[2].contains("ExemptionReason"));
        assert!(xml.contains("unitCode=\"HUR\"") && xml.contains("unitCode=\"LS\""));
        assert_eq!(
            value(&value(&xml, "ram:BuyerTradeParty"), "ram:CountryID"),
            "BE"
        );
        // Line discount: 3 × 333.33 = 999.99, net 899.99.
        assert!(xml.contains("<ram:ActualAmount>100.00</ram:ActualAmount><ram:ReasonCode>95</ram:ReasonCode><ram:Reason>Remise 10 %</ram:Reason>"));
        let header_allowances: Decimal = values(&xml, "ram:SpecifiedTradeAllowanceCharge")
            .iter()
            .filter(|a| a.contains("CategoryTradeTax"))
            .map(|a| Decimal::from_str_exact(&value(a, "ram:ActualAmount")).unwrap())
            .sum();
        assert_eq!(
            header_allowances.to_string(),
            value(&xml, "ram:AllowanceTotalAmount"),
            "BT-107 = Σ BT-92"
        );
        assert!(xml.contains("<ram:TypeCode>30</ram:TypeCode>"), "BT-81");
        reconciles(&xml, &quote);
        Ok(())
    }

    fn render(_: &crate::Draft, _: &Quote, number: &str, _: &str) -> Result<Vec<u8>> {
        Ok(format!("%PDF-1.4 {number}").into_bytes())
    }

    #[test]
    fn store_export_covers_deposits_credits_and_rejects_quotes_and_drafts() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        store.save_issuer(&issuer())?;
        let example = |price: &str| Quote {
            recipient: "Client de test".into(),
            due_date: "2026-12-31".into(),
            lines: vec![Line {
                description: "Prestation".into(),
                price: price.into(),
                vat: "20".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let quote = store.save_quote(None, "Devis", "EUR", "fr", &example("1000"))?;
        store.finalize_document(quote.id, render)?;
        assert!(store.facturx_xml(quote.id).is_err(), "quotes are rejected");
        store.accept_quote(quote.id, "2026-11-18")?;
        let deposit = store.derive_document(quote.id, "deposit", "Acompte")?;
        store.save_quote(Some(deposit.id), "Acompte", "EUR", "fr", &example("300"))?;
        let invoice = store.derive_document(quote.id, "invoice", "Solde")?;
        assert!(
            store.facturx_xml(invoice.id).is_err(),
            "drafts are rejected"
        );
        let deposit_number = store.finalize_document(deposit.id, render)?;
        let deposit_xml = store.facturx_xml(deposit.id)?;
        assert_eq!(value(&deposit_xml, "ram:TypeCode"), "386");
        assert_eq!(value(&deposit_xml, "ram:GrandTotalAmount"), "360.00");
        store.allocate_deposit(invoice.id, deposit.id)?;
        let number = store.finalize_document(invoice.id, render)?;
        let xml = store.facturx_xml(invoice.id)?;
        assert!(xml.contains(&format!("<ram:ID>{number}</ram:ID>")));
        assert_eq!(value(&xml, "ram:GrandTotalAmount"), "1200.00");
        assert_eq!(value(&xml, "ram:TotalPrepaidAmount"), "360.00", "BT-113");
        assert_eq!(value(&xml, "ram:DuePayableAmount"), "840.00", "BT-115");
        let reference = value(&xml, "ram:InvoiceReferencedDocument");
        assert_eq!(
            value(&reference, "ram:IssuerAssignedID"),
            deposit_number,
            "BT-25"
        );
        assert_eq!(
            value(&reference, "qdt:DateTimeString"),
            store.today()?.replace('-', ""),
            "BT-26"
        );
        let seller = value(&xml, "ram:SellerTradeParty");
        assert_eq!(value(&seller, "ram:Name"), "Studio Exemple");
        // The frozen issuer is used, not the current settings.
        store.save_issuer(&Issuer {
            name: "Nouveau nom".into(),
            ..issuer()
        })?;
        assert_eq!(store.facturx_xml(invoice.id)?, xml);
        let credit = store.derive_document(invoice.id, "credit", "Avoir")?;
        store.save_quote(
            Some(credit.id),
            "Avoir",
            "EUR",
            "fr",
            &Quote {
                due_date: String::new(),
                payment_terms: String::new(),
                ..example("100")
            },
        )?;
        store.finalize_document(credit.id, render)?;
        let credit_xml = store.facturx_xml(credit.id)?;
        assert_eq!(value(&credit_xml, "ram:TypeCode"), "381");
        assert_eq!(
            value(
                &value(&credit_xml, "ram:InvoiceReferencedDocument"),
                "ram:IssuerAssignedID"
            ),
            number
        );
        assert!(!credit_xml.contains("TotalPrepaidAmount"));
        assert_eq!(value(&credit_xml, "ram:DuePayableAmount"), "120.00");
        Ok(())
    }
}
