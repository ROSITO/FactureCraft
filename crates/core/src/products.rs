use crate::{
    Store,
    quote::{Line, Quote},
};
use anyhow::{Result, ensure};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// A reusable service or product. Documents copy its values into their own lines,
/// so editing or deleting a product never changes an existing document.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub unit: String,
    pub price: String,
    pub vat: String,
    pub exemption_reason: String,
}

impl Product {
    pub fn line(&self) -> Line {
        let name = self.name.trim();
        let details = self.description.trim();
        Line {
            description: if details.is_empty() {
                name.into()
            } else {
                format!("{name}\n{details}")
            },
            price: self.price.trim().into(),
            vat: self.vat.trim().into(),
            unit: self.unit.trim().into(),
            exemption_reason: self.exemption_reason.trim().into(),
            ..Default::default()
        }
    }
}

pub(crate) fn initialize(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY, name TEXT NOT NULL,
        description TEXT NOT NULL, unit TEXT NOT NULL, price TEXT NOT NULL, vat TEXT NOT NULL,
        exemption_reason TEXT NOT NULL);",
    )?;
    Ok(())
}

impl Store {
    pub fn products(&self) -> Result<Vec<Product>> {
        let mut query = self.0.prepare(
            "SELECT id,name,description,unit,price,vat,exemption_reason FROM products ORDER BY name COLLATE NOCASE",
        )?;
        let rows = query.query_map([], |row| {
            Ok(Product {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                unit: row.get(3)?,
                price: row.get(4)?,
                vat: row.get(5)?,
                exemption_reason: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Creates the product when `id` is 0, otherwise updates it. Returns its id.
    pub fn save_product(&self, product: &Product) -> Result<i64> {
        let name = product.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 200,
            "Product name must contain 1–200 characters"
        );
        // Same amount, tax and exemption rules as a document line.
        Quote {
            lines: vec![product.line()],
            ..Default::default()
        }
        .totals("EUR")?;
        let values = params![
            name,
            product.description.trim(),
            product.unit.trim(),
            product.price.trim(),
            product.vat.trim(),
            product.exemption_reason.trim(),
            product.id
        ];
        if product.id == 0 {
            self.0.execute(
                "INSERT INTO products(name,description,unit,price,vat,exemption_reason) VALUES (?1,?2,?3,?4,?5,?6)",
                &values[..6],
            )?;
            return Ok(self.0.last_insert_rowid());
        }
        ensure!(
            self.0.execute(
                "UPDATE products SET name=?1,description=?2,unit=?3,price=?4,vat=?5,exemption_reason=?6 WHERE id=?7",
                values,
            )? == 1,
            "Product not found"
        );
        Ok(product.id)
    }

    pub fn delete_product(&self, id: i64) -> Result<()> {
        ensure!(
            self.0.execute("DELETE FROM products WHERE id=?1", [id])? == 1,
            "Product not found"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_validates_and_copies_into_lines() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let mut product = Product {
            name: "Atelier".into(),
            description: "Demi-journée".into(),
            unit: "jour".into(),
            price: "450".into(),
            vat: "0".into(),
            exemption_reason: "Motif de test".into(),
            ..Default::default()
        };
        product.id = store.save_product(&product)?;
        assert_eq!(store.products()?, vec![product.clone()]);
        let line = product.line();
        assert_eq!(line.description, "Atelier\nDemi-journée");
        assert_eq!(line.quantity, "1");
        product.vat = "20".into();
        assert!(
            store.save_product(&product).is_err(),
            "exemption needs 0 % tax"
        );
        product.exemption_reason.clear();
        product.price = "-1".into();
        assert!(store.save_product(&product).is_err());
        product.price = "500".into();
        store.save_product(&product)?;
        assert_eq!(store.products()?[0].price, "500");
        store.delete_product(product.id)?;
        assert!(store.products()?.is_empty());
        Ok(())
    }
}
