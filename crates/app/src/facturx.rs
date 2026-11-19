//! Factur-X container: a copy of the finalized PDF turned into PDF/A-3B, with the
//! EN 16931 CII XML attached as `factur-x.xml`.
//!
//! The stored PDF stays untouched. Page content streams, fonts and images are copied
//! as they are; this module only adds the PDF/A-3 and Factur-X structures and fixes
//! what the renderer writes that PDF/A forbids:
//! - XMP metadata (PDF/A-3B identification, Factur-X extension schema), unfiltered;
//! - an sRGB OutputIntent, because pages paint in DeviceRGB;
//! - the embedded file with /AFRelationship, /Subtype text/xml, /Params (Size, ModDate),
//!   referenced from /AF and /Names /EmbeddedFiles on the catalog;
//! - a document information dictionary matching the XMP (printpdf writes PDF/X keys);
//! - /CIDToGIDMap /Identity on CIDFontType2 fonts (glyph ids are the CIDs, see subset.rs);
//! - /Interpolate false on images;
//! - a /Name on the optional content configuration (printpdf writes one layer);
//! - the file syntax: header 1.7 with a binary comment, EOL around obj/endobj and
//!   before endstream, exact /Length, trailer /ID.
//!
//! Seven synthetic scenarios independently validated with Mustang 2.26.0 (bundled veraPDF):
//! PDF/A-3B and embedded EN16931 XML passed on 2026-11-18; see docs/validation-facturx.md.
//! Credit notes retain three nonblocking French note warnings. This evidence does not cover
//! every possible invoice or establish acceptance by a French invoicing platform.
use anyhow::{Context, Result, ensure};
use facturecraft_core::{Store, facturx::FILE_NAME};
use lopdf::{Dictionary, Document, Object, Stream, StringFormat};
use std::io::Write;

/// sRGB IEC61966-2.1 ICC v4 profile (public domain), written by moxcms 0.8 `new_srgb()`.
const SRGB: &[u8] = include_bytes!("../../../assets/sRGB-IEC61966-2.1.icc");
const PRODUCER: &str = "FactureCraft";

/// Factur-X file of a finalized invoice, deposit or credit note.
pub fn export(store: &Store, id: i64) -> Result<Vec<u8>> {
    let xml = store.facturx_xml(id)?;
    let title = store.get(id)?.context("Document not found")?.title;
    embed(&store.finalized_pdf(id)?, &xml, &title)
}

pub fn embed(pdf: &[u8], xml: &str, title: &str) -> Result<Vec<u8>> {
    let mut doc = Document::load_mem(pdf).context("Finalized PDF cannot be read")?;
    // Creation date of the finalized PDF, reused as modification date: the content is
    // unchanged and the same document always gives the same Factur-X file.
    let created = doc
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
        .and_then(|id| doc.get_dictionary(id))
        .and_then(|info| info.get(b"CreationDate"))
        .and_then(Object::as_str)
        .ok()
        .and_then(|date| xmp_date(&String::from_utf8_lossy(date)))
        .context("Finalized PDF has no creation date")?;
    let (pdf_date, xmp_date) = created;

    for object in doc.objects.values_mut() {
        fix(object);
    }

    let icc = doc.add_object(compressed(
        Dictionary::from_iter([("N", Object::Integer(3))]),
        SRGB.to_vec(),
    )?);
    let attachment = doc.add_object(compressed(
        Dictionary::from_iter([
            ("Type", Object::Name(b"EmbeddedFile".to_vec())),
            ("Subtype", Object::Name(b"text/xml".to_vec())),
            (
                "Params",
                Object::Dictionary(Dictionary::from_iter([
                    ("Size", Object::Integer(xml.len() as i64)),
                    ("ModDate", text(&pdf_date)),
                ])),
            ),
        ]),
        xml.as_bytes().to_vec(),
    )?);
    let filespec = doc.add_object(Dictionary::from_iter([
        ("Type", Object::Name(b"Filespec".to_vec())),
        ("F", text(FILE_NAME)),
        ("UF", text(FILE_NAME)),
        ("Desc", text("Factur-X invoice")),
        // Alternative: the XML carries the same invoice as the PDF (EN 16931 profile).
        ("AFRelationship", Object::Name(b"Alternative".to_vec())),
        (
            "EF",
            Object::Dictionary(Dictionary::from_iter([
                ("F", Object::Reference(attachment)),
                ("UF", Object::Reference(attachment)),
            ])),
        ),
    ]));
    let metadata = doc.add_object(Stream::new(
        Dictionary::from_iter([
            ("Type", Object::Name(b"Metadata".to_vec())),
            ("Subtype", Object::Name(b"XML".to_vec())),
        ]),
        xmp(title, &xmp_date).into_bytes(),
    ));
    let info = Dictionary::from_iter([
        ("Title", text(title)),
        ("Producer", text(PRODUCER)),
        ("Creator", text(PRODUCER)),
        ("CreationDate", text(&pdf_date)),
        ("ModDate", text(&pdf_date)),
    ]);
    // Replace printpdf's dictionary (PDF/X keys) rather than leave it orphaned.
    let info = match doc.trailer.get(b"Info").and_then(Object::as_reference) {
        Ok(id) => {
            *doc.get_object_mut(id)? = Object::Dictionary(info);
            id
        }
        Err(_) => doc.add_object(info),
    };

    let root = doc.trailer.get(b"Root")?.as_reference()?;
    let catalog = doc.get_object_mut(root)?.as_dict_mut()?;
    catalog.set("Metadata", Object::Reference(metadata));
    catalog.set(
        "OutputIntents",
        Object::Array(vec![Object::Dictionary(Dictionary::from_iter([
            ("Type", Object::Name(b"OutputIntent".to_vec())),
            ("S", Object::Name(b"GTS_PDFA1".to_vec())),
            ("OutputConditionIdentifier", text("sRGB IEC61966-2.1")),
            ("Info", text("sRGB IEC61966-2.1")),
            ("DestOutputProfile", Object::Reference(icc)),
        ]))]),
    );
    catalog.set("AF", Object::Array(vec![Object::Reference(filespec)]));
    catalog.set(
        "Names",
        Object::Dictionary(Dictionary::from_iter([(
            "EmbeddedFiles",
            Object::Dictionary(Dictionary::from_iter([(
                "Names",
                Object::Array(vec![text(FILE_NAME), Object::Reference(filespec)]),
            )])),
        )])),
    );
    if let Ok(properties) = catalog
        .get_mut(b"OCProperties")
        .and_then(Object::as_dict_mut)
        && let Ok(default) = properties.get_mut(b"D").and_then(Object::as_dict_mut)
        && !default.has(b"Name")
    {
        default.set("Name", text("FactureCraft"));
    }

    doc.trailer.set("Info", Object::Reference(info));
    if !doc.trailer.has(b"ID") {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        pdf.hash(&mut hasher);
        let id = Object::String(
            format!("{:016x}", hasher.finish()).into_bytes(),
            StringFormat::Hexadecimal,
        );
        doc.trailer.set("ID", Object::Array(vec![id.clone(), id]));
    }
    write_pdf(&doc)
}

/// Applies the PDF/A fixes that concern fonts and images, at any depth.
fn fix(object: &mut Object) {
    let dict = match object {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => &mut stream.dict,
        Object::Array(items) => {
            items.iter_mut().for_each(fix);
            return;
        }
        _ => return,
    };
    if dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"CIDFontType2")
        && !dict.has(b"CIDToGIDMap")
    {
        dict.set("CIDToGIDMap", Object::Name(b"Identity".to_vec()));
    }
    if dict.has(b"Interpolate") {
        dict.set("Interpolate", Object::Boolean(false));
    }
    for (_, value) in dict.iter_mut() {
        fix(value);
    }
}

fn compressed(dict: Dictionary, content: Vec<u8>) -> Result<Stream> {
    let mut stream = Stream::new(dict, content);
    stream.compress()?;
    Ok(stream)
}

/// PDF text string: PDFDocEncoding for ASCII, otherwise UTF-16BE with a byte order mark.
fn text(value: &str) -> Object {
    let bytes = if value.is_ascii() {
        value.as_bytes().to_vec()
    } else {
        [0xFE, 0xFF]
            .into_iter()
            .chain(value.encode_utf16().flat_map(u16::to_be_bytes))
            .collect()
    };
    Object::String(bytes, StringFormat::Literal)
}

/// `D:YYYYMMDDHHmmSS` with an optional `Z` or `±HH'mm'` offset, as the PDF date
/// (completed with its offset) and the equivalent XMP date.
fn xmp_date(pdf: &str) -> Option<(String, String)> {
    let raw = pdf.strip_prefix("D:")?;
    let digits = raw
        .get(..14)
        .filter(|d| d.bytes().all(|b| b.is_ascii_digit()))?;
    let offset = &raw[14..];
    let (sign, hours, minutes) = match offset.as_bytes().first() {
        Some(sign @ (b'+' | b'-')) => (
            *sign as char,
            offset.get(1..3)?,
            offset.get(4..6).unwrap_or("00"),
        ),
        _ => ('+', "00", "00"),
    };
    Some((
        format!("D:{digits}{sign}{hours}'{minutes}'"),
        format!(
            "{}-{}-{}T{}:{}:{}{sign}{hours}:{minutes}",
            &digits[..4],
            &digits[4..6],
            &digits[6..8],
            &digits[8..10],
            &digits[10..12],
            &digits[12..14]
        ),
    ))
}

fn xmp(title: &str, date: &str) -> String {
    let title = escape(title);
    // The Factur-X schema is not predefined by PDF/A, hence the extension schema.
    format!(
        r#"<?xpacket begin="{bom}" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
<pdfaid:part>3</pdfaid:part>
<pdfaid:conformance>B</pdfaid:conformance>
</rdf:Description>
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:format>application/pdf</dc:format>
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">{title}</rdf:li></rdf:Alt></dc:title>
</rdf:Description>
<rdf:Description rdf:about="" xmlns:pdf="http://ns.adobe.com/pdf/1.3/">
<pdf:Producer>{PRODUCER}</pdf:Producer>
</rdf:Description>
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/">
<xmp:CreatorTool>{PRODUCER}</xmp:CreatorTool>
<xmp:CreateDate>{date}</xmp:CreateDate>
<xmp:ModifyDate>{date}</xmp:ModifyDate>
</rdf:Description>
<rdf:Description rdf:about="" xmlns:fx="urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#">
<fx:DocumentType>INVOICE</fx:DocumentType>
<fx:DocumentFileName>{FILE_NAME}</fx:DocumentFileName>
<fx:Version>1.0</fx:Version>
<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>
</rdf:Description>
<rdf:Description rdf:about="" xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/" xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#" xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#">
<pdfaExtension:schemas><rdf:Bag><rdf:li rdf:parseType="Resource">
<pdfaSchema:schema>Factur-X PDFA Extension Schema</pdfaSchema:schema>
<pdfaSchema:namespaceURI>urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#</pdfaSchema:namespaceURI>
<pdfaSchema:prefix>fx</pdfaSchema:prefix>
<pdfaSchema:property><rdf:Seq>
{properties}</rdf:Seq></pdfaSchema:property>
</rdf:li></rdf:Bag></pdfaExtension:schemas>
</rdf:Description>
</rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#,
        bom = '\u{feff}',
        properties = [
            ("DocumentFileName", "name of the embedded XML invoice file"),
            ("DocumentType", "INVOICE"),
            ("Version", "The actual version of the Factur-X XML schema"),
            ("ConformanceLevel", "The conformance level of the embedded Factur-X data"),
        ]
        .iter()
        .map(|(name, description)| format!(
            "<rdf:li rdf:parseType=\"Resource\"><pdfaProperty:name>{name}</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType><pdfaProperty:category>external</pdfaProperty:category><pdfaProperty:description>{description}</pdfaProperty:description></rdf:li>\n"
        ))
        .collect::<String>(),
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Serializes the document with the syntax PDF/A requires (ISO 19005-2, 6.1), which
/// lopdf 0.26's writer does not follow: binary comment after the header, EOL after
/// `obj`, before `endobj` and before `endstream`.
fn write_pdf(doc: &Document) -> Result<Vec<u8>> {
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![None; doc.max_id as usize + 1];
    for (&(id, generation), object) in &doc.objects {
        if matches!(object.type_name(), Ok("ObjStm" | "XRef")) {
            continue;
        }
        *offsets
            .get_mut(id as usize)
            .context("Object number out of range")? = Some((out.len(), generation));
        writeln!(out, "{id} {generation} obj")?;
        write_object(&mut out, object)?;
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    write!(out, "xref\n0 {}\n", offsets.len())?;
    for entry in &offsets {
        match entry {
            Some((offset, generation)) => write!(out, "{offset:010} {generation:05} n\r\n")?,
            None => out.extend_from_slice(b"0000000000 65535 f\r\n"),
        }
    }
    let mut trailer = Dictionary::new();
    for key in ["Root", "Info", "ID"] {
        trailer.set(key, doc.trailer.get(key.as_bytes())?.clone());
    }
    trailer.set("Size", Object::Integer(offsets.len() as i64));
    out.extend_from_slice(b"trailer\n");
    write_object(&mut out, &Object::Dictionary(trailer))?;
    write!(out, "\nstartxref\n{xref}\n%%EOF\n")?;
    Ok(out)
}

fn write_object(out: &mut Vec<u8>, object: &Object) -> Result<()> {
    match object {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Boolean(value) => write!(out, "{value}")?,
        Object::Integer(value) => write!(out, "{value}")?,
        Object::Real(value) => {
            ensure!(value.is_finite(), "Invalid real number");
            write!(out, "{value}")?
        }
        Object::Name(name) => {
            out.push(b'/');
            for &byte in name {
                if b" \t\n\r\x0C()<>[]{}/%#".contains(&byte) || !(33..=126).contains(&byte) {
                    write!(out, "#{byte:02X}")?;
                } else {
                    out.push(byte);
                }
            }
        }
        // Hexadecimal strings need no escaping.
        Object::String(bytes, _) => {
            out.push(b'<');
            for byte in bytes {
                write!(out, "{byte:02X}")?;
            }
            out.push(b'>');
        }
        Object::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b' ');
                }
                write_object(out, item)?;
            }
            out.push(b']');
        }
        Object::Dictionary(dict) => {
            out.extend_from_slice(b"<<");
            for (key, value) in dict.iter() {
                write_object(out, &Object::Name(key.clone()))?;
                out.push(b' ');
                write_object(out, value)?;
            }
            out.extend_from_slice(b">>");
        }
        Object::Stream(stream) => {
            let mut dict = stream.dict.clone();
            dict.set("Length", Object::Integer(stream.content.len() as i64));
            write_object(out, &Object::Dictionary(dict))?;
            out.extend_from_slice(b"\nstream\n");
            out.extend_from_slice(&stream.content);
            out.extend_from_slice(b"\nendstream");
        }
        Object::Reference((id, generation)) => write!(out, "{id} {generation} R")?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use facturecraft_core::{issuer::Issuer, quote::Quote};

    fn sample_pdf() -> Vec<u8> {
        let mut logo = Vec::new();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]))
            .write_to(
                &mut std::io::Cursor::new(&mut logo),
                image::ImageFormat::Png,
            )
            .unwrap();
        let quote = Quote {
            recipient: "Client de démonstration".into(),
            lines: vec![facturecraft_core::quote::Line {
                description: "Prestation".into(),
                price: "100".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::pdf::render(&crate::pdf::Document {
            kind: "invoice",
            number: Some("TEST-F-000001"),
            date: "2026-11-18",
            title: "Facture de démonstration",
            currency: "EUR",
            language: "fr",
            quote: &quote,
            issuer: &Issuer {
                name: "Exemple".into(),
                ..Default::default()
            },
            logo: Some(&logo),
            applied_deposit: None,
        })
        .unwrap()
    }

    #[test]
    fn embeds_the_xml_with_pdfa3_structures_and_strict_syntax() -> Result<()> {
        let pdf = sample_pdf();
        let xml = "<?xml version=\"1.0\"?><rsm:CrossIndustryInvoice/>";
        let output = embed(&pdf, xml, "Facture de démonstration")?;
        assert!(output.starts_with(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n"));
        assert!(output.ends_with(b"%%EOF\n"));
        assert_eq!(output, embed(&pdf, xml, "Facture de démonstration")?);

        let doc = Document::load_mem(&output)?;
        let catalog = doc.catalog()?;
        let names = catalog.get(b"Names")?.as_dict()?.get(b"EmbeddedFiles")?;
        let entries = names.as_dict()?.get(b"Names")?.as_array()?;
        assert_eq!(entries[0].as_str()?, FILE_NAME.as_bytes());
        let filespec = doc.get_dictionary(entries[1].as_reference()?)?;
        assert_eq!(
            catalog.get(b"AF")?.as_array()?[0].as_reference()?,
            entries[1].as_reference()?
        );
        assert_eq!(filespec.get(b"AFRelationship")?.as_name()?, b"Alternative");
        let file = doc
            .get_object(filespec.get(b"EF")?.as_dict()?.get(b"F")?.as_reference()?)?
            .as_stream()?;
        assert_eq!(file.dict.get(b"Subtype")?.as_name()?, b"text/xml");
        // lopdf leaves a stream unfiltered when deflate does not shrink it.
        let content = file.decompressed_content().unwrap_or(file.content.clone());
        assert_eq!(content, xml.as_bytes());
        let params = file.dict.get(b"Params")?.as_dict()?;
        assert_eq!(params.get(b"Size")?.as_i64()?, xml.len() as i64);
        assert!(params.has(b"ModDate"));

        let metadata = doc
            .get_object(catalog.get(b"Metadata")?.as_reference()?)?
            .as_stream()?;
        assert!(!metadata.dict.has(b"Filter"));
        let xmp = String::from_utf8(metadata.content.clone())?;
        for expected in [
            "<pdfaid:part>3</pdfaid:part>",
            "<pdfaid:conformance>B</pdfaid:conformance>",
            "<fx:DocumentType>INVOICE</fx:DocumentType>",
            "<fx:DocumentFileName>factur-x.xml</fx:DocumentFileName>",
            "<fx:Version>1.0</fx:Version>",
            "<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>",
            "Facture de démonstration",
        ] {
            assert!(xmp.contains(expected), "{expected}");
        }
        let intent = &catalog.get(b"OutputIntents")?.as_array()?[0];
        let profile = doc
            .get_object(
                intent
                    .as_dict()?
                    .get(b"DestOutputProfile")?
                    .as_reference()?,
            )?
            .as_stream()?;
        assert_eq!(profile.decompressed_content()?, SRGB);
        assert_eq!(&SRGB[16..20], b"RGB ");

        let info = doc.get_dictionary(doc.trailer.get(b"Info")?.as_reference()?)?;
        assert!(!info.has(b"GTS_PDFXVersion") && !info.has(b"Trapped"));
        assert!(
            !doc.objects
                .values()
                .any(|o| o.as_dict().is_ok_and(|d| d.has(b"GTS_PDFXVersion")))
        );
        let created = String::from_utf8(info.get(b"CreationDate")?.as_str()?.to_vec())?;
        let (_, xmp_created) = xmp_date(&created).unwrap();
        assert!(xmp.contains(&format!("<xmp:CreateDate>{xmp_created}</xmp:CreateDate>")));
        assert!(doc.trailer.has(b"ID"));

        let mut cid_fonts = 0;
        let mut images = 0;
        for object in doc.objects.values() {
            if let Ok(font) = object.as_dict()
                && font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0")
            {
                for descendant in font.get(b"DescendantFonts")?.as_array()? {
                    assert_eq!(
                        descendant.as_dict()?.get(b"CIDToGIDMap")?.as_name()?,
                        b"Identity"
                    );
                    cid_fonts += 1;
                }
            }
            if let Ok(stream) = object.as_stream()
                && stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image")
            {
                assert!(matches!(
                    stream.dict.get(b"Interpolate")?,
                    Object::Boolean(false)
                ));
                images += 1;
            }
        }
        assert_eq!((cid_fonts, images), (2, 1));
        // Every object starts on its own line and every stream ends with an EOL.
        let text = String::from_utf8_lossy(&output);
        assert!(!text.contains("obj<<") && !text.contains(">>endobj"));
        assert_eq!(
            text.matches("endstream").count(),
            text.matches("\nendstream").count()
        );
        // The page content is copied unchanged.
        let original = Document::load_mem(&pdf)?;
        let page = |doc: &Document| -> Result<Vec<u8>> {
            let (_, id) = doc.get_pages().into_iter().next().unwrap();
            Ok(doc.get_page_content(id)?)
        };
        assert_eq!(page(&doc)?, page(&original)?);
        Ok(())
    }

    /// Generates synthetic artifacts for independent XSD, Schematron and PDF/A validation.
    /// Run explicitly; never reads the user's database or source exports.
    #[test]
    #[ignore = "writes synthetic validation artifacts; run explicitly"]
    fn external_validation_fixture() -> Result<()> {
        use facturecraft_core::{
            facturx::{Invoice, cii_xml},
            issuer::Issuer,
            quote::{Line, Quote},
        };
        let directory = std::path::PathBuf::from(
            std::env::var("FACTURECRAFT_VALIDATION_DIR")
                .context("Set FACTURECRAFT_VALIDATION_DIR to the artifact directory")?,
        );
        std::fs::create_dir_all(&directory)?;
        let issuer = Issuer {
            name: "Studio Exemple".into(),
            address: "12 rue Exemple\n75001 Paris".into(),
            country: "France".into(),
            registration: "123456789".into(),
            email: "contact@example.test".into(),
            iban: "FR1420041010050500013M02606".into(),
            payment_terms: "30 jours".into(),
            payment_method: "Virement bancaire".into(),
            ..Default::default()
        };
        let quote = Quote {
            recipient: "Client Exemple".into(),
            recipient_address: "1 rue des Tests\n69001 Lyon".into(),
            recipient_email: "client@example.test".into(),
            service_date: "2026-11-18".into(),
            recipient_country: "France".into(),
            due_date: "2026-12-18".into(),
            lines: vec![Line {
                description: "Prestation de démonstration".into(),
                price: "100".into(),
                exemption_reason: "TVA non applicable, art. 293 B du CGI".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut taxed_issuer = issuer.clone();
        taxed_issuer.vat_id = "FR12123456789".into();
        let mut taxed = quote.clone();
        taxed.lines[0].vat = "20".into();
        taxed.lines[0].exemption_reason.clear();
        let mut mixed = taxed.clone();
        mixed.lines.push(Line {
            description: "Prestation taux réduit".into(),
            price: "77.77".into(),
            quantity: "3".into(),
            vat: "5.5".into(),
            ..Default::default()
        });
        mixed.lines.push(Line {
            description: "Prestation taux zéro".into(),
            price: "13.33".into(),
            exemption_reason: "zero".into(),
            ..Default::default()
        });
        // Explicit zero rate uses no exemption reason (category Z).
        mixed.lines[2].exemption_reason.clear();
        let mut discounted = mixed.clone();
        discounted.lines[0].quantity = "3".into();
        discounted.lines[0].price = "333.33".into();
        discounted.lines[0].discount = "10".into();
        discounted.global_discount = "7.5".into();
        for (name, kind, quote, issuer, prepaid, reference) in [
            ("invoice", "invoice", &quote, &issuer, "0", false),
            ("taxed", "invoice", &taxed, &taxed_issuer, "0", false),
            ("mixed", "invoice", &mixed, &taxed_issuer, "0", false),
            (
                "discount",
                "invoice",
                &discounted,
                &taxed_issuer,
                "0",
                false,
            ),
            ("deposit", "deposit", &taxed, &taxed_issuer, "0", false),
            ("balance", "invoice", &taxed, &taxed_issuer, "24", true),
            ("credit", "credit", &taxed, &taxed_issuer, "0", true),
        ] {
            let invoice = Invoice {
                kind,
                number: "TEST-F-000001",
                issue_date: "2026-11-18",
                currency: "EUR",
                language: "fr",
                quote,
                issuer,
                prepaid,
                preceding: reference.then(|| facturecraft_core::facturx::Reference {
                    number: "TEST-SOURCE-000001".into(),
                    date: "2026-11-01".into(),
                }),
            };
            let xml = cii_xml(&invoice)?;
            let gross = quote.totals("EUR")?.2;
            let due = (rust_decimal::Decimal::from_str_exact(&gross)?
                - rust_decimal::Decimal::from_str_exact(prepaid)?)
            .to_string();
            let pdf = crate::pdf::render(&crate::pdf::Document {
                kind,
                number: Some(invoice.number),
                date: invoice.issue_date,
                title: "Document de démonstration",
                currency: "EUR",
                language: "fr",
                quote,
                issuer,
                logo: None,
                applied_deposit: (prepaid != "0").then_some((prepaid, &due)),
            })?;
            std::fs::write(directory.join(format!("{name}.xml")), &xml)?;
            std::fs::write(
                directory.join(format!("{name}.pdf")),
                embed(&pdf, &xml, "Document de démonstration")?,
            )?;
        }
        Ok(())
    }

    #[test]
    fn converts_pdf_dates_to_xmp() {
        assert_eq!(
            xmp_date("D:20261118174259+00'00'").unwrap().1,
            "2026-11-18T17:42:59+00:00"
        );
        assert_eq!(
            xmp_date("D:20261118174259").unwrap(),
            (
                "D:20261118174259+00'00'".into(),
                "2026-11-18T17:42:59+00:00".into()
            )
        );
        assert!(xmp_date("2026").is_none());
    }
}
