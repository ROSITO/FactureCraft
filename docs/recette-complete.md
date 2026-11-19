# Recette de FactureCraft — 18 novembre 2026

## Vérifications automatisées exécutées

Les tests utilisent des bases temporaires et des données synthétiques.

- Contact, devis, finalisation, acceptation et conversion en facture avec vrais PDF générés.
- Acompte de 30 %, encaissement de l’acompte, facture de solde, imputation, paiement partiel, avoir et solde nul.
- Préservation des documents figés après modification des contacts et de l’émetteur.
- Refus des transitions invalides, doubles imputations, dépassements de paiement et éditions après finalisation.
- Sauvegarde et restauration, préservation des PDF et XML, reprise de numérotation.
- Refus d’une restauration sur une destination occupée ou d’un instantané corrompu.
- Préparation MCP, répétition idempotente et rejet des versions périmées.
- Export/import d’archives avec conservation des originaux.
- Suite workspace et contrôle Clippy exécutés. Les deux tests de recette core ont été réexécutés après l’ajout du cas d’archive corrompue.

Références de tests : `crates/core/tests/full_recipe.rs`, `crates/app/tests/shared_draft.rs`, test `pdf::acceptance::actual_pdfs_follow_quote_invoice_payment_credit_and_restore`.

## Import graphique

L’assistant est intégré dans Paramètres, section « Importer les données Facture.net ». Il charge un dossier d’exports avec un libellé source, conserve le lot analysé jusqu’à la confirmation, affiche les nombres de fiches et PDF ainsi que les avertissements. L’application du lot calcule les doublons et conflits puis rafraîchit les contacts et archives. Les conflits existants sont conservés.

La reprise des lignes historiques reste partielle et fait l’objet d’un rapport explicite via `archive reconcile`.

## Limites de la recette

La matrice Factur-X synthétique et ses rapports externes sont décrits dans `docs/validation-facturx.md`. La distribution Windows/Linux et la signature macOS restent à vérifier/configurer. Cette recette ne confirme pas leur réalisation.

L’assistant d’import a été vérifié nativement sur une base synthétique en FR et EN. Le lot contient quatre fiches et deux PDF dont un non associé. La prévisualisation conserve une archive vide. Après confirmation, six entrées sont ajoutées et les clients/archives apparaissent. Le rejeu donne zéro ajout et six inchangés. Un dossier incomplet affiche une erreur et ne peut pas être appliqué. Le parcours financier a été vérifié par boutons natifs sur la base synthétique. Devis 120 EUR accepté, acompte de 36 EUR, facture 120 EUR avec acompte imputé, paiement de 20 EUR, avoir corrigé à 12 EUR puis finalisé. La fenêtre facture affiche 36 EUR d’acomptes, 20 EUR réglés, 12 EUR d’avoirs et 52 EUR restants. La session de test a été fermée.

Contrôle final du 19 novembre 2026 : 44 tests passent et Clippy sans avertissement. Le test ignoré de génération des fixtures externes est couvert séparément par les rapports Mustang conservés.

Frottement restant : l’avoir proposé pour une facture avec acompte reprend initialement le montant complet et nécessite une correction manuelle. Le garde-fou bloque bien la finalisation au-delà du plafond, mais son message est encore en anglais dans l’interface française.


## Clôture des corrections — 19 novembre 2026

Le paquet macOS produit par scripts/package-macos.sh a été extrait et ouvert graphiquement sur la base synthétique. Le parcours d’avoir propose 72 EUR sur une facture de 120 EUR avec 36 EUR d’acomptes affectés et 12 EUR d’avoirs précédents. Le calcul multi-TVA et la protection de version passent les tests. Les messages concernés sont traduits en français.

La reprise couvre maintenant 35 documents et 44 lignes historiques, plus trois déductions d’acompte associées par preuve. Trois lignes d’acompte n’ont pas de quantité ni de prix unitaire imprimés. Quatre documents n’ont aucun taux de TVA explicite dans les sources. Ces champs restent inconnus.

La distribution finale work/distribution-verified/FactureCraft-macOS-arm64.zip a été reconstruite après toutes les modifications ; les tests workspace et Clippy passent. GitHub Actions reste refusé au niveau du compte. Les builds Windows/Linux et la signature/notarisation restent ouverts.
