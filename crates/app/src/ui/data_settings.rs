//! Settings section: backup, restore and document exports.
use super::Drafts;
use crate::operations;
use eframe::egui;
use std::path::{Path, PathBuf};

impl Drafts {
    pub(super) fn data_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        let today = self.store.today().unwrap_or_default();
        let database = self.store.path();
        crate::brand::section(ui, t("Sauvegarde et restauration", "Backup and restore"));
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(t("Base de données actuelle", "Current database"));
            ui.add(
                egui::Label::new(
                    database
                        .as_ref()
                        .map_or_else(|| "(memory)".into(), |p| p.display().to_string()),
                )
                .selectable(true),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(t("Créer une sauvegarde…", "Create a backup…"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name(format!("facturecraft-{today}.sqlite3"))
                        .save_file()
                {
                    self.report(
                        operations::publish_file(&path, |file| self.store.snapshot(file.path())),
                        t("Sauvegarde créée : ", "Backup created: "),
                        &path,
                    );
                }
                if ui
                    .button(t("Restaurer une sauvegarde…", "Restore a backup…"))
                    .clicked()
                    && let Some(source) = rfd::FileDialog::new().pick_file()
                {
                    self.restore(&source, database.as_deref(), &today, fr);
                }
            });
            ui.weak(t(
                "Une restauration ne remplace jamais la base en cours : elle crée une copie vérifiée à côté, que vous ouvrez ensuite volontairement.",
                "Restoring never replaces the current database: it creates a verified copy beside it, which you then open on purpose.",
            ));
        });
        crate::brand::section(ui, t("Exporter les documents", "Export documents"));
        crate::brand::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(t("Exporter les documents (CSV)", "Export documents (CSV)"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name(format!("documents-{today}.csv"))
                        .save_file()
                {
                    let result = self
                        .store
                        .export_documents_csv()
                        .and_then(|bytes| operations::write_file(&path, &bytes));
                    self.report(result, t("Export créé : ", "Export created: "), &path);
                }
                if ui
                    .button(t(
                        "Exporter les documents (JSON)",
                        "Export documents (JSON)",
                    ))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_file_name(format!("documents-{today}.json"))
                        .save_file()
                {
                    let result = self
                        .store
                        .export_documents_json()
                        .and_then(|bytes| operations::write_file(&path, &bytes));
                    self.report(result, t("Export créé : ", "Export created: "), &path);
                }
            });
        });
    }

    fn report(&mut self, result: anyhow::Result<()>, done: &str, path: &Path) {
        match result {
            Ok(()) => {
                self.error = None;
                self.notice = Some(format!("{done}{}", path.display()));
            }
            Err(error) => {
                self.notice = None;
                self.error = Some(error.to_string());
            }
        }
    }

    /// Validates `source` into a new file beside the current database and explains how to open it.
    fn restore(&mut self, source: &Path, database: Option<&Path>, today: &str, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        let Some(folder) = database.and_then(Path::parent) else {
            self.error = Some(
                t(
                    "Base en mémoire : restauration impossible.",
                    "In-memory database: cannot restore.",
                )
                .into(),
            );
            return;
        };
        let mut target = PathBuf::new();
        for n in 1.. {
            let suffix = if n == 1 {
                String::new()
            } else {
                format!("-{n}")
            };
            target = folder.join(format!("facturecraft-restaure-{today}{suffix}.sqlite3"));
            if !target.exists() {
                break;
            }
        }
        let result = operations::publish_file(&target, |file| {
            facturecraft_core::backup::restore_snapshot(source, file.path())
        });
        if result.is_ok() {
            self.report(
                result,
                t(
                    "Sauvegarde vérifiée et copiée dans un nouveau fichier. Votre base actuelle est intacte. Pour l’utiliser, quittez FactureCraft puis lancez : facturecraft --db \"CHEMIN\" gui, avec CHEMIN = ",
                    "Backup verified and copied to a new file. Your current database is untouched. To use it, quit FactureCraft then run: facturecraft --db \"PATH\" gui, with PATH = ",
                ),
                &target,
            );
        } else {
            self.report(result, "", &target);
        }
    }
}
