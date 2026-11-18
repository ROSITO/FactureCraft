# Utiliser FactureCraft

Lancer le fichier `facturecraft` ou `facturecraft.exe` ouvre l’interface. L’application, la CLI et le serveur MCP partagent la base locale. L’option globale `--db` ou la variable `FACTURECRAFT_DB` permet de choisir une autre base, notamment pour un essai.

## Préparer et consulter un devis

```console
facturecraft draft prepare "Prestation" exemple.json --currency EUR --language fr --key exemple-001
facturecraft draft details 1
facturecraft draft preview 1 exemple.pdf
```

Contenu de `exemple.json` :

```json
{
  "recipient": "Client exemple",
  "lines": [
    {
      "description": "Prestation",
      "quantity": "1.5",
      "price": "100.00",
      "discount": "0",
      "vat": "20"
    }
  ]
}
```

Les montants et les pourcentages sont des chaînes décimales. La réponse de `draft details` fournit le brouillon, sa version dans `billing.version` et ses totaux. Pour modifier un brouillon, indiquer cette version dans `draft update ID VERSION TITRE FICHIER --currency EUR --language fr`. Une nouvelle valeur est nécessaire si le document a changé. Lors d’une relance de `draft prepare` après une réponse incertaine, réutiliser la même clé avec la même demande. La numérotation et la finalisation se font dans l’interface.

## Utiliser MCP dans Codex ou Hermes

Déclarer un serveur local stdio avec le chemin absolu du binaire installé comme commande et `mcp` comme argument. Pour choisir une base dédiée, ajouter `--db`, suivi de son chemin absolu, avant `mcp`. La liste des outils publiée par le serveur fournit les schémas d’entrée.

- `search_parties` recherche par nom dans le carnet de contacts local.
- `list_drafts` et `get_draft` lisent les métadonnées. `get_document` fournit le contenu, la version et les totaux.
- `create_draft` et `create_invoice_draft_from_quote` prennent une clé d’idempotence. Le devis source d’une facture doit avoir été accepté dans l’interface.
- `update_draft` prend la version attendue.
- `preview_draft` renvoie le PDF comme ressource intégrée de type `application/pdf`.

## Importer et sauvegarder

```console
facturecraft import CHEMIN_EXPORT
facturecraft import CHEMIN_EXPORT --apply --source facture-net-2026
facturecraft archive list
facturecraft archive get facture-net-2026 quotations CLE
facturecraft archive export-pdf facture-net-2026 CLE_PDF original.pdf
facturecraft backup copie.sqlite3
facturecraft --db restauree.sqlite3 restore copie.sqlite3
```

La prévisualisation d’import n’ouvre pas la base. Pour rejouer un lot, reprendre le même libellé de source. Le rapport indique les enregistrements déjà présents, les conflits et les PDF à rapprocher. `archive link-pdf SOURCE CLE_PDF quotations ID` permet de rattacher un PDF à un devis. Utiliser `invoices` pour les factures et acomptes. Les enregistrements archivés et les PDF restent dans la base, y compris dans le fichier de sauvegarde SQLite.

La restauration vérifie l’intégrité SQLite et crée une nouvelle base. Une base déjà présente à la destination est conservée. Ouvrir la copie restaurée avec `facturecraft --db restauree.sqlite3 gui`.
