use crate::{Draft, Store, billing::BillingState, quote::parse_tags};
use anyhow::{Result, ensure};
use rusqlite::params;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Contact {
    /// Local address book row; `None` for contacts read from the imported archive.
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    pub email: String,
    /// One line per address line.
    pub address: String,
    #[serde(default)]
    pub country: String,
    /// Company number (SIREN or equivalent).
    #[serde(default)]
    pub registration: String,
    #[serde(default)]
    pub phone: String,
    /// Private CRM notes; never printed on documents.
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Next follow-up date `YYYY-MM-DD`, or empty when none.
    #[serde(default)]
    pub follow_up: String,
}

/// A local document addressed to a client.
pub struct ClientDocument {
    pub draft: Draft,
    pub state: BillingState,
    /// Issue date `YYYY-MM-DD`; empty for a draft.
    pub date: String,
    pub gross: String,
}

/// A read-only Facture.net archive document addressed to a client.
pub struct ArchivedDocument {
    pub kind: &'static str,
    pub number: String,
    pub date: String,
    pub currency: String,
    /// Amount before tax, as recorded by the source.
    pub net: String,
}

/// Revenue and outstanding totals for one currency on a client card.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ClientSummary {
    pub currency: String,
    /// Invoices minus credit notes before tax, including Facture.net archive. Deposits excluded.
    pub revenue: String,
    /// Local invoices and deposits only; archived payment history can be incomplete.
    pub outstanding: String,
}

#[derive(Default)]
pub struct ClientHistory {
    pub documents: Vec<ClientDocument>,
    pub archived: Vec<ArchivedDocument>,
    pub summaries: Vec<ClientSummary>,
}

fn same_client(a: &str, b: &str) -> bool {
    let a = a.trim();
    !a.is_empty() && a.to_lowercase() == b.trim().to_lowercase()
}

fn format_amount(value: Decimal) -> String {
    format!("{:.2}", value)
}

impl Store {
    /// Documents whose recipient name equals `name` (case-insensitive, trimmed), newest first.
    pub fn client_history(&self, name: &str) -> Result<ClientHistory> {
        let mut history = ClientHistory::default();
        let mut revenue = BTreeMap::<String, Decimal>::new();
        let mut outstanding = BTreeMap::<String, Decimal>::new();
        for draft in self.list()? {
            let quote = self.quote(draft.id)?;
            if !same_client(name, &quote.recipient) {
                continue;
            }
            let date: Option<String> = self.0.query_row(
                "SELECT date(issued_at,'localtime') FROM billing WHERE id=?1",
                [draft.id],
                |row| row.get(0),
            )?;
            let state = self.billing_state(draft.id)?;
            let net = Decimal::from_str_exact(&quote.totals(&draft.currency)?.0)?;
            if state
                .number
                .as_deref()
                .is_some_and(|number| !number.starts_with("TEST-"))
            {
                match state.kind.as_str() {
                    "invoice" => *revenue.entry(draft.currency.clone()).or_default() += net,
                    "credit" => *revenue.entry(draft.currency.clone()).or_default() -= net,
                    _ => {}
                }
                if matches!(state.kind.as_str(), "invoice" | "deposit") {
                    let due = Decimal::from_str_exact(&state.outstanding)?.max(Decimal::ZERO);
                    *outstanding.entry(draft.currency.clone()).or_default() += due;
                }
            }
            history.documents.push(ClientDocument {
                state,
                gross: quote.totals(&draft.currency)?.2,
                date: date.unwrap_or_default(),
                draft,
            });
        }
        if self.has_archive()? {
            let mut query = self.0.prepare(
                "SELECT category, coalesce(date(json_extract(c,'$.createdAt'),'unixepoch','localtime'),''),
                upper(coalesce(json_extract(c,'$.currency'),'EUR')), json_extract(c,'$.type'),
                coalesce(json_extract(c,'$.uid'),''),
                coalesce(json_extract(c,'$.recipient.company'),''),
                trim(coalesce(json_extract(c,'$.recipient.firstName'),'')||' '||coalesce(json_extract(c,'$.recipient.lastName'),'')),
                CAST(coalesce(json_extract(c,'$.pricePretax'),0) AS TEXT)
            FROM (SELECT category, CAST(content AS TEXT) AS c FROM facture_net_archive
                  WHERE category IN ('quotations','invoices'))
            ORDER BY 2 DESC, 5 DESC",
            )?;
            let rows = query.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ArchivedDocument {
                        date: row.get(1)?,
                        currency: row.get(2)?,
                        kind: "",
                        number: row.get(4)?,
                        net: row.get(7)?,
                    },
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (category, mut document, kind, company, person) = row?;
                if same_client(name, &company) || same_client(name, &person) {
                    document.kind = crate::dashboard::archive_kind(&category, kind.as_deref());
                    let net = Decimal::from_str_exact(&document.net).unwrap_or_default();
                    match document.kind {
                        "invoice" => *revenue.entry(document.currency.clone()).or_default() += net,
                        "credit" => *revenue.entry(document.currency.clone()).or_default() -= net,
                        _ => {}
                    }
                    history.archived.push(document);
                }
            }
        }
        let mut currencies: Vec<_> = revenue.keys().chain(outstanding.keys()).cloned().collect();
        currencies.sort();
        currencies.dedup();
        history.summaries = currencies
            .into_iter()
            .map(|currency| ClientSummary {
                revenue: format_amount(*revenue.get(&currency).unwrap_or(&Decimal::ZERO)),
                outstanding: format_amount(*outstanding.get(&currency).unwrap_or(&Decimal::ZERO)),
                currency,
            })
            .collect();
        Ok(history)
    }

    /// Local contacts whose follow-up date is today or earlier, soonest first.
    pub fn follow_ups_due(&self) -> Result<Vec<Contact>> {
        let today = self.today()?;
        let mut records = Vec::new();
        let mut query = self.0.prepare(
            "SELECT rowid,name,email,address,country,registration,phone,notes,tags,follow_up
             FROM contacts
             WHERE follow_up != '' AND follow_up <= ?1
             ORDER BY follow_up, name COLLATE NOCASE",
        )?;
        for row in query.query_map([&today], read_contact)? {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn contacts(&self) -> Result<Vec<Contact>> {
        let mut records = Vec::new();
        let mut query = self.0.prepare(
            "SELECT rowid,name,email,address,country,registration,phone,notes,tags,follow_up
             FROM contacts ORDER BY name COLLATE NOCASE",
        )?;
        for row in query.query_map([], read_contact)? {
            records.push(row?);
        }
        let imported: bool = self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='facture_net_archive')",
            [],
            |row| row.get(0),
        )?;
        if imported {
            let mut query = self.0.prepare(
                "SELECT content FROM facture_net_archive WHERE category IN ('clients','companies') ORDER BY source,category,source_key",
            )?;
            let mut rows = query.query([])?;
            while let Some(row) = rows.next()? {
                let record: serde_json::Value = serde_json::from_slice(&row.get::<_, Vec<u8>>(0)?)?;
                let field = |key: &str| record[key].as_str().unwrap_or("").trim();
                let name = if field("name").is_empty() {
                    format!("{} {}", field("firstName"), field("lastName"))
                        .trim()
                        .to_owned()
                } else {
                    field("name").to_owned()
                };
                if name.is_empty() {
                    continue;
                }
                let city = format!("{} {}", field("zipCode"), field("city"));
                let mut address = vec![field("street")];
                if let Some(extra) = record["additional_address_lines"].as_array() {
                    address.extend(extra.iter().filter_map(serde_json::Value::as_str));
                }
                address.push(city.trim());
                records.push(Contact {
                    id: None,
                    name,
                    email: field("email").into(),
                    address: address
                        .into_iter()
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    country: field("country").into(),
                    registration: field("company_number").into(),
                    phone: field("phone").into(),
                    ..Default::default()
                });
            }
        }
        records.sort_by_key(|record| record.name.to_lowercase());
        // A local entry wins over the archived one it duplicates.
        records.sort_by_key(|record| (record.name.to_lowercase(), record.id.is_none()));
        records.dedup_by(|a, b| a.name == b.name && a.email == b.email && a.address == b.address);
        Ok(records)
    }

    pub fn save_contact(&self, contact: &Contact) -> Result<()> {
        let name = contact.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 200,
            "Contact name must contain 1–200 characters"
        );
        let follow_up = contact.follow_up.trim();
        if !follow_up.is_empty() {
            self.validate_date(follow_up)?;
        }
        let tags = parse_tags(&contact.tags.join(",")).join(", ");
        let values = params![
            name,
            contact.email.trim(),
            contact.address.trim(),
            contact.country.trim(),
            contact.registration.trim(),
            contact.phone.trim(),
            contact.notes.trim(),
            tags,
            follow_up,
            contact.id
        ];
        if contact.id.is_some() {
            ensure!(
                self.0.execute(
                    "UPDATE contacts SET name=?1,email=?2,address=?3,country=?4,registration=?5,
                     phone=?6,notes=?7,tags=?8,follow_up=?9 WHERE rowid=?10",
                    values,
                )? == 1,
                "Contact not found"
            );
        } else {
            // Keep existing CRM fields when the insert path sends blanks (e.g. “Save to contacts”).
            self.0.execute(
                "INSERT INTO contacts(name,email,address,country,registration,phone,notes,tags,follow_up)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                ON CONFLICT(name,email,address) DO UPDATE SET
                    country=excluded.country, registration=excluded.registration,
                    phone=CASE WHEN excluded.phone='' THEN contacts.phone ELSE excluded.phone END,
                    notes=CASE WHEN excluded.notes='' THEN contacts.notes ELSE excluded.notes END,
                    tags=CASE WHEN excluded.tags='' THEN contacts.tags ELSE excluded.tags END,
                    follow_up=CASE WHEN excluded.follow_up='' THEN contacts.follow_up ELSE excluded.follow_up END",
                &values[..9],
            )?;
        }
        Ok(())
    }

    /// Removes an address book entry; documents keep their own copy of the recipient.
    pub fn delete_contact(&self, id: i64) -> Result<()> {
        ensure!(
            self.0
                .execute("DELETE FROM contacts WHERE rowid=?1", [id])?
                == 1,
            "Contact not found"
        );
        Ok(())
    }
}

fn read_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
    let tags: String = row.get(8)?;
    Ok(Contact {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        email: row.get(2)?,
        address: row.get(3)?,
        country: row.get(4)?,
        registration: row.get(5)?,
        phone: row.get(6)?,
        notes: row.get(7)?,
        tags: parse_tags(&tags),
        follow_up: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::{Line, Quote};

    fn quote_for(recipient: &str) -> Quote {
        Quote {
            recipient: recipient.into(),
            lines: vec![Line {
                description: "A".into(),
                price: "100".into(),
                vat: "20".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn client_history_matches_recipient_names_and_archive() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        store.save_quote(None, "Un", "EUR", "fr", &quote_for("Client Exemple"))?;
        store.save_quote(None, "Deux", "EUR", "fr", &quote_for("  client EXEMPLE "))?;
        store.save_quote(None, "Autre", "EUR", "fr", &quote_for("Autre client"))?;
        store.save_quote(None, "Vide", "EUR", "fr", &quote_for(""))?;
        store.0.execute_batch(
            "CREATE TABLE IF NOT EXISTS facture_net_archive (source TEXT NOT NULL, category TEXT NOT NULL, source_key TEXT NOT NULL,
                content BLOB NOT NULL, record_link TEXT, PRIMARY KEY (source, category, source_key));",
        )?;
        for (category, key, json) in [
            (
                "invoices",
                "1",
                r#"{"uid":"F012600001","type":"facture","createdAt":1767225600,"currency":"eur","pricePretax":120.5,"recipient":{"company":"CLIENT exemple"}}"#,
            ),
            (
                "quotations",
                "2",
                r#"{"uid":"D012600002","createdAt":1767225600,"pricePretax":10,"recipient":{"firstName":"Client","lastName":"Exemple"}}"#,
            ),
            (
                "invoices",
                "3",
                r#"{"uid":"F012600003","type":"facture","createdAt":1767225600,"pricePretax":5,"recipient":{"company":"Autre client"}}"#,
            ),
        ] {
            store.0.execute(
                "INSERT INTO facture_net_archive VALUES ('s',?1,?2,?3,NULL)",
                params![category, key, json.as_bytes()],
            )?;
        }
        let history = store.client_history("Client exemple")?;
        assert_eq!(history.documents.len(), 2);
        assert_eq!(history.documents[0].gross, "120.00");
        assert_eq!(history.documents[0].date, "");
        assert_eq!(history.archived.len(), 2);
        assert!(
            history
                .archived
                .iter()
                .any(|d| d.number == "F012600001" && d.kind == "invoice" && d.net == "120.5")
        );
        assert!(history.archived.iter().any(|d| d.kind == "quote"));
        assert_eq!(history.summaries.len(), 1);
        assert_eq!(history.summaries[0].currency, "EUR");
        assert_eq!(history.summaries[0].revenue, "120.50");
        assert_eq!(history.summaries[0].outstanding, "0.00");
        assert!(store.client_history("")?.documents.is_empty());
        Ok(())
    }

    #[test]
    fn contact_crm_fields_and_follow_ups_roundtrip() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let today = store.today()?;
        store.save_contact(&Contact {
            name: "Atelier".into(),
            email: "a@example.test".into(),
            address: "1 rue".into(),
            phone: "+33 1 23 45 67 89".into(),
            notes: "Préfère le matin".into(),
            tags: parse_tags("vip, chantier"),
            follow_up: today.clone(),
            ..Default::default()
        })?;
        store.save_contact(&Contact {
            name: "Plus tard".into(),
            email: "b@example.test".into(),
            address: "2 rue".into(),
            follow_up: "2099-01-01".into(),
            ..Default::default()
        })?;
        let contacts = store.contacts()?;
        let atelier = contacts.iter().find(|c| c.name == "Atelier").unwrap();
        assert_eq!(atelier.phone, "+33 1 23 45 67 89");
        assert_eq!(atelier.notes, "Préfère le matin");
        assert_eq!(atelier.tags, ["vip", "chantier"]);
        assert_eq!(atelier.follow_up, today);
        let due = store.follow_ups_due()?;
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].name, "Atelier");
        let mut cleared = atelier.clone();
        cleared.follow_up.clear();
        store.save_contact(&cleared)?;
        assert!(store.follow_ups_due()?.is_empty());
        Ok(())
    }

    #[test]
    fn blank_insert_conflict_preserves_crm_fields() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        store.save_contact(&Contact {
            name: "Atelier".into(),
            email: "a@example.test".into(),
            address: "1 rue".into(),
            phone: "0102030405".into(),
            notes: "Privé".into(),
            tags: parse_tags("vip"),
            follow_up: "2099-06-01".into(),
            ..Default::default()
        })?;
        store.save_contact(&Contact {
            name: "Atelier".into(),
            email: "a@example.test".into(),
            address: "1 rue".into(),
            country: "FR".into(),
            ..Default::default()
        })?;
        let contact = store.contacts()?.into_iter().next().unwrap();
        assert_eq!(contact.country, "FR");
        assert_eq!(contact.phone, "0102030405");
        assert_eq!(contact.notes, "Privé");
        assert_eq!(contact.tags, ["vip"]);
        assert_eq!(contact.follow_up, "2099-06-01");
        Ok(())
    }

    #[test]
    fn client_summary_counts_local_revenue_and_outstanding() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let mut content = quote_for("Client CRM");
        content.due_date = "2099-12-31".into();
        let invoice = store.save_quote(None, "Facture", "EUR", "fr", &content)?;
        store.0.execute(
            "UPDATE billing SET kind='invoice', number='F012600099', issued_at=datetime('now') WHERE id=?1",
            [invoice.id],
        )?;
        let history = store.client_history("Client CRM")?;
        assert_eq!(history.summaries.len(), 1);
        assert_eq!(history.summaries[0].revenue, "100.00");
        assert_eq!(history.summaries[0].outstanding, "120.00");
        Ok(())
    }
}
