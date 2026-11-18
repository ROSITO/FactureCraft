use crate::Store;
use anyhow::Result;
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Activity {
    /// `YYYY-MM-DD`
    pub date: String,
    pub kind: String,
    pub number: String,
    pub detail: String,
    pub amount: String,
}

/// Activity summary for one year and one currency. Revenue follows Facture.net:
/// invoices minus credit notes, excluding deposits, before tax. Demonstration
/// (`TEST-`) documents are never counted.
#[derive(Debug, Serialize)]
pub struct Dashboard {
    pub year: i32,
    pub currency: String,
    pub currencies: Vec<String>,
    pub quotes: usize,
    pub invoices: usize,
    pub revenue_year: String,
    pub revenue_month: String,
    /// Local documents only: archived payment history can be incomplete.
    pub outstanding: String,
    pub monthly: Vec<String>,
    pub activity: Vec<Activity>,
}

struct Event {
    date: String,
    currency: String,
    kind: String,
    number: String,
    recipient: String,
    net: Decimal,
}

/// Document kind of an archived Facture.net record.
pub(crate) fn archive_kind(category: &str, kind: Option<&str>) -> &'static str {
    match (category, kind) {
        ("quotations", _) => "quote",
        (_, Some(kind)) if kind.contains("acompte") => "deposit",
        (_, Some(kind)) if kind.to_lowercase().contains("avoir") => "credit",
        _ => "invoice",
    }
}

impl Store {
    pub fn dashboard(&self, year: i32, currency: &str) -> Result<Dashboard> {
        let currency = currency.trim().to_ascii_uppercase();
        let (this_year, this_month): (i32, i64) = self.0.query_row(
            "SELECT CAST(strftime('%Y','now','localtime') AS INTEGER), CAST(strftime('%m','now','localtime') AS INTEGER)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // 0 selects the current year.
        let year = if year == 0 { this_year } else { year };
        let events = self.events()?;
        let mut currencies: Vec<String> = events.iter().map(|e| e.currency.clone()).collect();
        currencies.push(currency.clone());
        currencies.sort();
        currencies.dedup();

        let mut monthly = [Decimal::ZERO; 12];
        let (mut quotes, mut invoices) = (0, 0);
        for event in events
            .iter()
            .filter(|e| e.currency == currency && e.date.starts_with(&year.to_string()))
        {
            let month: usize = event.date[5..7].parse().unwrap_or(1);
            match event.kind.as_str() {
                "quote" => quotes += 1,
                "invoice" => {
                    invoices += 1;
                    monthly[month - 1] += event.net;
                }
                "credit" => monthly[month - 1] -= event.net,
                _ => {}
            }
        }
        let mut outstanding = Decimal::ZERO;
        let mut payments = Vec::new();
        let mut query = self.0.prepare(
            "SELECT b.id FROM billing b JOIN drafts d ON d.id=b.id
            WHERE b.kind IN ('invoice','deposit') AND b.number NOT LIKE 'TEST-%' AND d.currency=?1",
        )?;
        for id in query.query_map([&currency], |row| row.get::<_, i64>(0))? {
            let id = id?;
            let state = self.billing_state(id)?;
            outstanding += Decimal::from_str_exact(&state.outstanding)?.max(Decimal::ZERO);
            let mut paid = self
                .0
                .prepare("SELECT date, amount FROM billing_payment WHERE document_id=?1")?;
            for row in paid.query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })? {
                let (date, amount) = row?;
                payments.push(Activity {
                    date,
                    kind: "payment".into(),
                    number: state.number.clone().unwrap_or_default(),
                    detail: String::new(),
                    amount,
                });
            }
        }

        let mut activity: Vec<Activity> = events
            .into_iter()
            .filter(|e| e.currency == currency)
            .map(|e| Activity {
                date: e.date,
                kind: e.kind,
                number: e.number,
                detail: e.recipient,
                amount: e.net.to_string(),
            })
            .chain(payments)
            .collect();
        activity.sort_by(|a, b| b.date.cmp(&a.date));
        activity.truncate(6);

        let format = |value: Decimal| format!("{:.2}", value);
        Ok(Dashboard {
            year,
            currency,
            currencies,
            quotes,
            invoices,
            revenue_year: format(monthly.iter().sum()),
            revenue_month: format(if year == this_year {
                monthly[this_month as usize - 1]
            } else {
                Decimal::ZERO
            }),
            outstanding: format(outstanding),
            monthly: monthly.into_iter().map(format).collect(),
            activity,
        })
    }

    /// Currency to open the dashboard on: the preferred one when it has activity,
    /// otherwise the currency with the most documents.
    pub fn dashboard_currency(&self, preferred: &str) -> Result<String> {
        let preferred = preferred.trim().to_ascii_uppercase();
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for event in self.events()? {
            *counts.entry(event.currency).or_default() += 1;
        }
        if counts.contains_key(&preferred) || counts.is_empty() {
            return Ok(preferred);
        }
        Ok(counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(currency, _)| currency)
            .unwrap_or(preferred))
    }

    /// Finalized local documents and imported Facture.net quotes and invoices.
    fn events(&self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        let mut query = self.0.prepare(
            "SELECT b.id, date(b.issued_at,'localtime'), d.currency, b.kind, b.number
            FROM billing b JOIN drafts d ON d.id=b.id
            WHERE b.number IS NOT NULL AND b.number NOT LIKE 'TEST-%'",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (id, date, currency, kind, number) = row?;
            let quote = self.quote(id)?;
            events.push(Event {
                net: Decimal::from_str_exact(&quote.totals(&currency)?.0)?,
                date,
                currency,
                kind,
                number,
                recipient: quote.recipient,
            });
        }
        if !self.has_archive()? {
            return Ok(events);
        }
        // Archive amounts are JSON numbers; keep them as text to avoid binary rounding.
        let mut query = self.0.prepare(
            "SELECT category, date(json_extract(c,'$.createdAt'),'unixepoch','localtime'),
                upper(coalesce(json_extract(c,'$.currency'),'EUR')), json_extract(c,'$.type'),
                coalesce(json_extract(c,'$.uid'),''), coalesce(json_extract(c,'$.recipient.company'),''),
                CAST(coalesce(json_extract(c,'$.pricePretax'),0) AS TEXT)
            FROM (SELECT category, CAST(content AS TEXT) AS c FROM facture_net_archive
                  WHERE category IN ('quotations','invoices'))
            WHERE coalesce(json_extract(c,'$.createdAt'),0) > 0",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;
        for row in rows {
            let (category, date, currency, kind, number, recipient, net) = row?;
            let kind = archive_kind(&category, kind.as_deref());
            events.push(Event {
                date,
                currency,
                kind: kind.into(),
                number,
                recipient,
                net: Decimal::from_str_exact(&net).unwrap_or_default(),
            });
        }
        Ok(events)
    }
}
