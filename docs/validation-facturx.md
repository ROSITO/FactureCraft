# Validation indépendante Factur-X — 18 novembre 2026

Sept scénarios synthétiques, produits par le générateur XML et le moteur PDF de l’application, ont été contrôlés localement avec Mustang CLI 2.26.0 (veraPDF embarqué). Aucun export ni document client n’a été envoyé à un service.

Les sept conteneurs Factur-X, leurs PDF/A-3B et leurs XML EN16931 ont un résultat global **valide**. Les rapports bruts et les empreintes des fichiers testés sont conservés dans `test/validation/` ; `facturx-matrix.json` fournit la synthèse exacte.

| Cas | Assertions PDF/A réussies | Règles XML déclenchées | Avertissements |
| --- | ---: | ---: | ---: |
| Exonération française | 7 625 | 53 | 0 |
| TVA 20 % | 7 781 | 53 | 0 |
| Taux mixtes 20 %, 5,5 %, zéro | 8 075 | 81 | 0 |
| Remise par ligne et remise globale | 8 246 | 95 | 0 |
| Facture d’acompte | 7 781 | 53 | 0 |
| Facture avec acompte déduit et référence | 7 871 | 56 | 0 |
| Avoir avec référence à la facture | 7 688 | 55 | 3 |

L’avoir reçoit trois avertissements BR-FR-05 : absence des notes PMT, PMD et AAB. Son XML et le conteneur restent valides. Le générateur n’ajoute actuellement pas de clauses de retard, recouvrement ou escompte à un avoir ; cette décision métier reste à revoir avant d’affirmer une conformité aux exigences d’une plateforme française. Le compteur `rules_failed=3` de ce cas correspond à ces avertissements, pas à trois erreurs bloquantes.

Cette campagne couvre ces jeux de données seulement. Elle ne constitue pas une certification de tous les documents possibles, ni une validation des obligations de transmission à une plateforme française. Les autres devises, moyens de paiement, taux et motifs d’exonération, très longs documents et logos ne sont pas couverts par cette campagne indépendante.

Le premier contrôle a révélé deux erreurs réelles : un virement sans compte bancaire et une exonération sans identifiant fiscal du vendeur. Le générateur exige désormais un IBAN avec somme de contrôle valide pour les virements et fournit le SIREN fiscal du vendeur français. Les notes PMT et AAB sont également codées. Les documents sans date de prestation ou adresse électronique destinataire peuvent encore recevoir des avertissements du validateur ; aucune valeur n’est inventée.

## Reproduction

Source : [publication Mustang 2.26.0](https://github.com/ZUGFeRD/mustangproject/releases/tag/core-2.26.0). Le [FNFE](https://fnfe-mpe.org/factur-x/) publie les versions et règles Factur-X.

Télécharger `Mustang-CLI-2.26.0.jar` dans `work/validation/`. SHA-256 du fichier utilisé : `42d7868cb68264874a7b8cab4c3587b03b23ccc7cd72373da917f66758bb9736`.

```sh
FACTURECRAFT_VALIDATION_DIR="$PWD/work/validation" cargo test -p facturecraft external_validation_fixture -- --ignored
java -jar work/validation/Mustang-CLI-2.26.0.jar --action validate --source work/validation/invoice.xml --no-notices --disable-file-logging
java -jar work/validation/Mustang-CLI-2.26.0.jar --action validate --source work/validation/invoice.pdf --no-notices --disable-file-logging
```

Pour reproduire la matrice, valider également les fichiers `taxed.pdf`, `mixed.pdf`, `discount.pdf`, `deposit.pdf`, `balance.pdf` et `credit.pdf` générés dans le même dossier. Chaque validation PDF vérifie aussi le XML embarqué. Contrôler le `<summary status="valid"/>` final du rapport : ne pas se fier au seul code de sortie Java. Les empreintes de chaque cas se trouvent dans `test/validation/facturx-matrix.json`. La date de création du PDF varie lors d’une nouvelle génération.

## Historique

`Store::reconcile_archive()` reconstruit une vue structurée et répétable des archives : type, numéro, montants source, destinataire figé, état et date de paiement d’origine, pièces PDF. Les paiements absents, lignes non reconstruites et relations métier non confirmées sont signalés explicitement. L’opération est en lecture seule et ne crée aucun solde comptable. Les textes complémentaires disponibles restent des preuves à rapprocher ; ils ne sont pas convertis automatiquement en lignes ou liens présumés.
