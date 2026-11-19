//! Settings section: backup, restore and document exports.
use super::Drafts;
use crate::operations;
use eframe::egui;
use std::path::{Path, PathBuf};

pub(super) struct ImportPreview {
    batch: facturecraft_core::import::Batch,
    folder: PathBuf,
    source: String,
}

fn import_counts(
    ui: &mut egui::Ui,
    report: &facturecraft_core::import::Report,
    fr: bool,
    applied: bool,
) {
    let t = |f, e| if fr { f } else { e };
    for (key, fr_label, en_label) in [
        ("clients", "Clients", "Individuals"),
        ("companies", "Sociétés", "Companies"),
        ("quotations", "Devis", "Quotes"),
        ("invoices", "Factures et acomptes", "Invoices and deposits"),
    ] {
        ui.label(format!(
            "{} : {}",
            t(fr_label, en_label),
            report.records_by_category.get(key).copied().unwrap_or(0)
        ));
    }
    ui.label(format!(
        "{} : {} · {} : {} · {} : {}",
        t("PDF", "PDFs"),
        report.pdfs,
        t("associés", "linked"),
        report.linked_pdfs,
        t("non associés", "unlinked"),
        report.unlinked_pdfs
    ));
    if applied {
        ui.label(format!(
            "{} : {} · {} : {} · {} : {}",
            t("Ajoutés", "Inserted"),
            report.inserted,
            t("Inchangés", "Unchanged"),
            report.unchanged,
            t("Conflits", "Conflicts"),
            report.conflicts
        ));
    }
}

impl Drafts {
    pub(super) fn data_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        let today = self.store.today().unwrap_or_default();
        let database = self.store.path();
        crate::brand::section(
            ui,
            t(
                "Importer les données Facture.net",
                "Import Facture.net data",
            ),
        );
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(t(
                "Choisissez le dossier racine de l’export et donnez un nom stable à cette source. L’analyse ne modifie pas la base.",
                "Choose the export root folder and give this source a stable name. Analysis does not change the database.",
            ));
            if self.import_preview.is_none() {
                ui.label(t("Nom de la source", "Source name"));
                ui.add(
                    crate::brand::field(&mut self.import_source)
                        .hint_text(t("Ex. export initial", "E.g. initial export")),
                );
                if ui
                    .button(t(
                        "Choisir et analyser le dossier…",
                        "Choose and analyze folder…",
                    ))
                    .clicked()
                {
                    if self.import_source.trim().is_empty() {
                        self.import_error = Some(
                            t(
                                "Indiquez un nom de source avant de choisir le dossier.",
                                "Enter a source name before choosing a folder.",
                            )
                            .into(),
                        );
                    } else if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.import_result = None;
                        match facturecraft_core::import::Batch::load(&folder) {
                            Ok(batch) => {
                                self.import_preview = Some(ImportPreview {
                                    batch,
                                    folder,
                                    source: self.import_source.trim().to_owned(),
                                });
                                self.import_error = None;
                            }
                            Err(error) => {
                                self.import_error = Some(format!(
                                    "{} {error:#}",
                                    t("Analyse impossible :", "Analysis failed:")
                                ))
                            }
                        }
                    }
                }
            }
            if let Some(preview) = &self.import_preview {
                ui.strong(t("Analyse avant import", "Pre-import analysis"));
                ui.label(format!("{} : {}", t("Source", "Source"), preview.source));
                ui.label(format!(
                    "{} : {}",
                    t("Dossier", "Folder"),
                    preview.folder.display()
                ));
                import_counts(ui, preview.batch.report(), fr, false);
                ui.weak(t("Les fichiers non associés restent dans l’archive. Les paiements source peuvent être incomplets ; vérifiez les soldes. Les lignes et liens entre documents peuvent nécessiter un rapprochement manuel.", "Unlinked files remain in the archive. Source payments may be incomplete; check balances. Lines and document relationships may need manual reconciliation."));
                ui.weak(t("Les doublons et conflits avec la base seront comptés lors de l’import. Les données existantes en conflit ne seront pas remplacées.", "Duplicates and conflicts with the database are counted during import. Existing conflicting data will not be replaced."));
                let mut apply = false;
                let mut cancel = false;
                ui.horizontal_wrapped(|ui| {
                    apply = ui
                        .add(crate::brand::primary(t(
                            "Importer ce lot",
                            "Import this batch",
                        )))
                        .clicked();
                    cancel = ui
                        .button(t("Annuler l’analyse", "Cancel analysis"))
                        .clicked();
                });
                if cancel {
                    self.import_preview = None;
                    self.import_error = None;
                }
                if apply && let Some(preview) = self.import_preview.take() {
                    match preview.batch.apply(&mut self.store, &preview.source) {
                        Ok(report) => {
                            self.import_result = Some(report);
                            self.import_error = None;
                            self.client_detail = None;
                            self.refresh();
                        }
                        Err(error) => {
                            self.import_error = Some(format!(
                                "{} {error:#}",
                                t(
                                    "Import impossible ; relancez l’analyse :",
                                    "Import failed; analyze the folder again:"
                                )
                            ))
                        }
                    }
                }
            }
            if let Some(error) = &self.import_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            if let Some(report) = &self.import_result {
                ui.separator();
                ui.strong(t("Bilan de l’import", "Import reconciliation"));
                import_counts(ui, report, fr, true);
                ui.weak(t("Les conflits n’ont pas été remplacés. Vérifiez les PDF non associés, les paiements et les liens entre documents dans les archives.", "Conflicts were not replaced. Review unlinked PDFs, payments and document relationships in the archive."));
            }
        });
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
