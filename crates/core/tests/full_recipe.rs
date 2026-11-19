//! Public-API business recipe. Synthetic data only; PDF bytes are deterministic test doubles.
//! Real renderer/container acceptance is exercised by the app's independent tests.
use anyhow::Result;
use facturecraft_core::{
    Draft, Store,
    backup::restore_snapshot,
    billing::Share,
    contacts::Contact,
    issuer::Issuer,
    quote::{Line, Quote},
};
use rust_decimal::Decimal;
use std::path::PathBuf;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "facturecraft-recipe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn amount(value: &str) -> Decimal {
    Decimal::from_str_exact(value).unwrap()
}
fn render(_: &Draft, quote: &Quote, number: &str, date: &str) -> Result<Vec<u8>> {
    Ok(format!(
        "%PDF-1.4\nsynthetic {number} {date} {}\n%%EOF",
        quote.totals("EUR")?.2
    )
    .into_bytes())
}

#[test]
fn client_quote_acceptance_deposit_invoice_payment_credit_backup_restore() -> Result<()> {
    let workspace = Workspace::new()?;
    let store = Store::open(&workspace.0.join("source.sqlite3"))?;
    let issuer = Issuer {
        name: "Studio Synthétique".into(),
        address: "1 rue Exemple\n75001 Paris".into(),
        country: "France".into(),
        registration: "123456789".into(),
        vat_id: "FR12123456789".into(),
        payment_method: "Espèces".into(),
        ..Default::default()
    };
    store.save_issuer(&issuer)?;
    store.save_contact(&Contact {
        name: "Client Synthétique".into(),
        email: "client@example.test".into(),
        address: "2 rue Exemple\n69001 Lyon".into(),
        country: "France".into(),
        registration: "987654321".into(),
        ..Default::default()
    })?;
    let contact = store.contacts()?.remove(0);
    let content = Quote {
        recipient: contact.name.clone(),
        recipient_email: contact.email.clone(),
        recipient_address: contact.address.clone(),
        recipient_country: contact.country.clone(),
        recipient_registration: contact.registration.clone(),
        due_date: "2099-12-31".into(),
        lines: vec![Line {
            description: "Prestation synthétique".into(),
            price: "1000".into(),
            vat: "20".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let quote =
        store.create_quote_idempotent("recipe-quote", "Devis exemple", "EUR", "fr", &content)?;
    assert!(
        store
            .derive_document(quote.id, "invoice", "Invalid")
            .is_err()
    );
    assert_eq!(store.finalize_document(quote.id, render)?, "TEST-D-000001");
    assert!(
        store
            .derive_document(quote.id, "invoice", "Unsigned")
            .is_err()
    );
    store.accept_quote(quote.id, "2026-11-18")?;
    assert!(store.billing_state(quote.id)?.accepted);

    let deposit = store.derive_deposit(
        quote.id,
        store.billing_state(quote.id)?.version,
        &Share::Percent("30".into()),
        "Acompte 30 %",
    )?;
    assert_eq!(store.quote(deposit.id)?.totals("EUR")?.2, "360.00");
    assert!(
        store
            .record_payment(deposit.id, "360", "2026-11-18", "cash", "before issuance")
            .is_err()
    );
    assert_eq!(
        store.finalize_document(deposit.id, render)?,
        "TEST-FA-000001"
    );
    store.record_payment(deposit.id, "360", "2026-11-18", "cash", "deposit paid")?;
    let invoice = store.derive_document(quote.id, "invoice", "Facture finale")?;
    store.allocate_deposit(invoice.id, deposit.id)?;
    assert!(store.allocate_deposit(invoice.id, deposit.id).is_err());
    let state = store.billing_state(invoice.id)?;
    assert_eq!(amount(&state.deposits), amount("360"));
    assert_eq!(amount(&state.paid), Decimal::ZERO);
    assert_eq!(amount(&state.outstanding), amount("840"));
    assert_eq!(
        store.finalize_document_versioned(invoice.id, state.version, render)?,
        "TEST-F-000001"
    );
    assert!(store.allocate_deposit(invoice.id, deposit.id).is_err());
    assert!(
        store
            .record_payment(invoice.id, "840.01", "2026-11-18", "cash", "overpayment")
            .is_err()
    );
    store.record_payment(
        invoice.id,
        "600",
        "2026-11-18",
        "cash",
        "partial invoice payment",
    )?;
    assert_eq!(
        amount(&store.billing_state(invoice.id)?.outstanding),
        amount("240")
    );

    let credit = store.derive_document(invoice.id, "credit", "Avoir partiel")?;
    let mut credit_content = store.quote(credit.id)?;
    credit_content.lines[0].price = "200".into();
    store.save_quote_versioned(
        credit.id,
        store.billing_state(credit.id)?.version,
        "Avoir partiel",
        "EUR",
        "fr",
        &credit_content,
    )?;
    assert_eq!(store.finalize_document(credit.id, render)?, "TEST-A-000001");
    let state = store.billing_state(invoice.id)?;
    assert_eq!(amount(&state.credited), amount("240"));
    assert_eq!(amount(&state.outstanding), Decimal::ZERO);
    assert_eq!(store.payments(invoice.id)?.len(), 1);
    assert_eq!(store.payments(deposit.id)?.len(), 1);
    assert!(
        store
            .record_payment(invoice.id, "0.01", "2026-11-18", "cash", "settled")
            .is_err()
    );
    let allocated_deposit_credit =
        store.derive_document(deposit.id, "credit", "Invalid deposit credit")?;
    assert!(
        store
            .finalize_document(allocated_deposit_credit.id, render)
            .is_err()
    );
    store.delete_draft(allocated_deposit_credit.id)?;

    let ids = [quote.id, deposit.id, invoice.id, credit.id];
    let frozen: Vec<_> = ids
        .iter()
        .map(|id| {
            Ok((
                *id,
                store.finalized_pdf(*id)?,
                serde_json::to_value(store.billing_state(*id)?)?,
            ))
        })
        .collect::<Result<_>>()?;
    let xml: Vec<_> = [deposit.id, invoice.id, credit.id]
        .iter()
        .map(|id| Ok((*id, store.facturx_xml(*id)?)))
        .collect::<Result<_>>()?;
    assert!(
        xml[1]
            .1
            .contains("<ram:TotalPrepaidAmount>360.00</ram:TotalPrepaidAmount>")
    );
    assert!(xml[2].1.contains("<ram:TypeCode>381</ram:TypeCode>"));
    assert!(xml[2].1.contains("TEST-F-000001"));
    let mut changed_contact = contact.clone();
    changed_contact.name = "Nom modifié".into();
    store.save_contact(&changed_contact)?;
    let mut changed_issuer = issuer.clone();
    changed_issuer.name = "Émetteur modifié".into();
    store.save_issuer(&changed_issuer)?;
    assert_eq!(store.quote(invoice.id)?.recipient, contact.name);
    for (id, expected_xml) in &xml {
        assert_eq!(store.facturx_xml(*id)?, *expected_xml);
    }

    let payments_before: Vec<_> = [deposit.id, invoice.id]
        .iter()
        .map(|id| Ok((*id, serde_json::to_value(store.payments(*id)?)?)))
        .collect::<Result<_>>()?;
    let snapshot = workspace.0.join("backup.sqlite3");
    store.snapshot(&snapshot)?;
    let snapshot_bytes = std::fs::read(&snapshot)?;
    let destination = workspace.0.join("restored.sqlite3");
    restore_snapshot(&snapshot, &destination)?;
    assert!(restore_snapshot(&snapshot, &destination).is_err());
    assert_eq!(std::fs::read(&snapshot)?, snapshot_bytes);
    let restored = Store::open(&destination)?;
    assert_eq!(restored.list()?.len(), 4);
    assert_eq!(restored.contacts()?.len(), 1);
    assert_eq!(restored.contacts()?[0].name, changed_contact.name);
    assert_eq!(restored.issuer()?.name, changed_issuer.name);
    assert_eq!(restored.client_history(&contact.name)?.documents.len(), 4);
    for (id, pdf, state) in &frozen {
        assert_eq!(restored.finalized_pdf(*id)?, *pdf);
        assert_eq!(serde_json::to_value(restored.billing_state(*id)?)?, *state);
        assert!(
            restored
                .save_quote(Some(*id), "Mutation interdite", "EUR", "fr", &content)
                .is_err()
        );
    }
    for (id, expected_xml) in xml {
        assert_eq!(restored.facturx_xml(id)?, expected_xml);
    }
    for (id, expected_payments) in payments_before {
        assert_eq!(
            serde_json::to_value(restored.payments(id)?)?,
            expected_payments
        );
    }
    assert_eq!(restored.payments(invoice.id)?.len(), 1);
    assert_eq!(restored.payments(deposit.id)?.len(), 1);
    let next = restored.save_quote(None, "Après restauration", "EUR", "fr", &content)?;
    assert_eq!(
        restored.finalize_document(next.id, render)?,
        "TEST-D-000002"
    );
    Ok(())
}

#[test]
fn corrupt_snapshot_cannot_replace_existing_database() -> Result<()> {
    let workspace = Workspace::new()?;
    let destination = workspace.0.join("existing.sqlite3");
    let store = Store::open(&destination)?;
    let draft = store.create("Existing synthetic draft", "EUR", "fr")?;
    let corrupt = workspace.0.join("corrupt.sqlite3");
    std::fs::write(&corrupt, b"this is not a SQLite snapshot")?;
    assert!(restore_snapshot(&corrupt, &destination).is_err());
    assert_eq!(
        store.get(draft.id)?.unwrap().title,
        "Existing synthetic draft"
    );
    let absent = workspace.0.join("absent.sqlite3");
    assert!(restore_snapshot(&corrupt, &absent).is_err());
    assert!(!absent.exists());
    Ok(())
}
