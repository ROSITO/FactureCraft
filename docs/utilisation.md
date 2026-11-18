# Utiliser FactureCraft

Lancer le fichier `facturecraft` ou `facturecraft.exe` ouvre l’interface sur le tableau de bord. L’application, la CLI et le serveur MCP partagent la base locale. L’option globale `--db` ou la variable `FACTURECRAFT_DB` permet de choisir une autre base, notamment pour un essai.

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

Les montants et les pourcentages sont des chaînes décimales. La réponse de `draft details` fournit le brouillon, sa version dans `billing.version` et ses totaux. Pour modifier un brouillon, indiquer cette version dans `draft update ID VERSION TITRE FICHIER --currency EUR --language fr`. Une nouvelle valeur est nécessaire si le document a changé. Lors d’une relance de `draft prepare` après une réponse incertaine, réutiliser la même clé avec la même demande. La numérotation et la finalisation se font dans l’interface, jamais par la CLI ni le MCP.

## Tableau de bord

Le tableau de bord résume une année et une devise : devis et factures émis, chiffre d’affaires HT du mois et de l’année, montant en attente de règlement, courbe mensuelle et dernières activités. Le chiffre d’affaires compte les factures moins les avoirs, hors acomptes. Il inclut l’historique importé de Facture.net. Les documents de démonstration numérotés `TEST-` ne sont pas comptés. Le montant en attente ne couvre que les documents émis dans FactureCraft, car l’historique des paiements importé peut être incomplet.

## Produits et services

La page Produits & services conserve les prestations facturées souvent : nom, description, unité, prix HT, TVA ou motif d’exonération. Dans l’éditeur, « Depuis le catalogue » ajoute une ligne avec ces valeurs. La ligne est une copie : modifier ou supprimer un produit ne change aucun document existant. L’outil MCP `list_products` donne le catalogue aux agents.

## Numérotation et émission réelle

Par défaut, la finalisation produit des documents de démonstration numérotés `TEST-…`. Dans Paramètres, l’émission réelle s’active une fois l’émetteur renseigné (nom, adresse, identifiant d’immatriculation) et après confirmation que Facture.net n’est plus utilisé pour émettre.

Les numéros reprennent le format de Facture.net : préfixe (`D` devis, `F` facture, `FA` acompte, `A` avoir), mois, année sur deux chiffres, compteur annuel à cinq chiffres, par exemple `F012700001`. La série continue après le dernier numéro importé de l’année et repart à `00001` chaque année. Un numéro déjà présent dans l’archive n’est jamais attribué. Le PDF figé porte la date d’émission. Le retour au mode démonstration reste possible tant qu’aucun numéro réel n’a été attribué.

## Logo, émetteur et mise en page des PDF

Dans Paramètres, « Choisir un logo… » accepte un PNG ou un JPEG. L’image est réduite à 600 pixels au plus et s’affiche en haut à droite des documents. Les champs de l’émetteur (société, contact, adresse, pays, SIREN, TVA, téléphone, e-mail) alimentent le bloc Émetteur. Les conditions de règlement, le mode de règlement et la validité du devis sont repris dans chaque nouveau document, puis modifiables document par document. Les conditions générales de vente s’impriment sur une page dédiée des devis.

Le PDF suit cette organisation : titre et date, émetteur et destinataire côte à côte, tableau des prestations (intitulé sur la première ligne de la description, détail en dessous), mentions d’exonération et totaux, puis une page Conditions et une page Conditions générales de vente. Un brouillon porte la date du jour ; un document finalisé porte sa date d’émission.

## Apparence

Dans Paramètres, la section Apparence propose cinq couleurs d’interface : Prune, Océan, Forêt, Corail et Ardoise. Le menu, les boutons, les sélections, le graphique et le fond de l’icône suivent la couleur choisie. Les badges d’état gardent leurs couleurs propres. La même section active ou désactive l’animation de page tournée entre les vues. Ces choix sont enregistrés dans la base locale.

## Rédiger un devis détaillé

La description d’une ligne commence par l’intitulé, sur la première ligne. Le détail suit en dessous, sur autant de paragraphes que nécessaire. Les conditions particulières du document s’impriment sur la page Conditions. Les conditions générales de vente sont copiées depuis Paramètres à la création du document. On peut ensuite les adapter dans l’éditeur, rubrique « Conditions générales de vente de ce document », sans modifier le modèle. « Recharger le modèle » reprend le texte de Paramètres.

Mise en forme, valable dans les descriptions, les conditions et les CGV :

- `**texte**` met le texte en gras, par exemple `**Forfait global : 8 000 €.**` ;
- `# Intertitre` en début de ligne crée un intertitre en gras ;
- une ligne vide sépare deux paragraphes.

## Utiliser MCP dans Codex ou Hermes

Déclarer un serveur local stdio avec le chemin absolu du binaire installé comme commande et `mcp` comme argument. Pour choisir une base dédiée, ajouter `--db`, suivi de son chemin absolu, avant `mcp`. La liste des outils publiée par le serveur fournit les schémas d’entrée.

- `search_parties` recherche par nom dans le carnet de contacts local.
- `list_drafts` et `get_draft` lisent les métadonnées. `get_document` fournit le contenu, la version et les totaux.
- `create_draft` et `create_invoice_draft_from_quote` prennent une clé d’idempotence. Le devis source d’une facture doit avoir été accepté dans l’interface.
- `update_draft` prend la version attendue.
- `preview_draft` renvoie le PDF comme ressource intégrée de type `application/pdf`.
- `list_products` lit le catalogue de produits et services.

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
