# Utiliser FactureCraft

Lancer le fichier `facturecraft` ou `facturecraft.exe` ouvre l’interface sur le tableau de bord. L’application, la CLI et le serveur MCP partagent la base locale. L’option globale `--db` ou la variable `FACTURECRAFT_DB` permet de choisir une autre base, notamment pour un essai.

## Installer et mettre à jour la version portable

Téléchargez l’archive correspondant à votre système et à son architecture depuis une source FactureCraft de confiance, puis décompressez-la dans un dossier de votre choix. Sur macOS, copiez `FactureCraft.app` dans Applications puis ouvrez-la. Sous Windows, lancez `facturecraft.exe` ; sous Linux, lancez `facturecraft` et, si nécessaire, rendez-le exécutable avec `chmod +x facturecraft`. Windows et Linux sont livrés en version portable, sans installateur. Les archives actuelles ne sont pas signées ; le système peut afficher un avertissement de sécurité.

Pour une mise à jour, créez d’abord une sauvegarde depuis Paramètres, quittez l’application, puis remplacez l’application macOS ou le binaire Windows/Linux par celui de la nouvelle archive. La base est stockée séparément du binaire ; ne supprimez pas votre dossier de données. Vérifiez le fonctionnement avant de supprimer l’ancienne version. Il n’y a pas de mise à jour automatique. Une ancienne version peut refuser une base ouverte par une version plus récente : conservez la sauvegarde faite avant la mise à jour si un retour arrière est nécessaire.

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

## Factures : mentions obligatoires

Une facture ou un acompte ne se finalise qu’avec une date d’échéance (AAAA-MM-JJ). Une facture créée depuis un devis reçoit par défaut une échéance à 30 jours, modifiable. La date de prestation est facultative. Les deux s’impriment sous la date du document. La mention des pénalités de retard et de l’indemnité forfaitaire de 40 € s’imprime automatiquement sous les totaux des factures, des acomptes et des avoirs, et figure aussi dans leur Factur-X. Son texte se personnalise dans Paramètres. Les CGV peuvent aussi s’imprimer sur les factures (option dans Paramètres).

## Gérer les documents et les clients

Depuis « Voir et agir » :
- dupliquer un devis ou une facture ;
- supprimer un brouillon (un document finalisé est conservé) ;
- marquer un devis finalisé comme accepté, refusé ou annulé ;
- enregistrer un règlement avec sa référence, puis annuler un règlement saisi par erreur.

Les listes se filtrent par état et se recherchent par titre, client ou numéro. Dans Clients, les contacts du carnet se modifient et se suppriment. Les contacts importés de Facture.net restent en lecture seule et peuvent être copiés dans le carnet.

## Étiquettes et notes internes

Dans l’éditeur, le champ « Étiquettes » accepte une liste séparée par des virgules et « Notes internes » un texte libre. Les deux restent privés : ils ne sont jamais imprimés sur le PDF. Les étiquettes apparaissent en pastilles dans les listes, et la recherche de documents les parcourt, notes comprises. Les outils MCP acceptent les champs optionnels `tags` et `notes`.

## Facturer un devis accepté : lignes et acompte

Dans « Voir et agir » d’un devis accepté, « Créer une facture » ouvre la liste des lignes avec une case à cocher chacune (toutes cochées par défaut) : le brouillon ne reprend que les lignes cochées. « Créer un acompte » demande un pourcentage (30 % par défaut) ou un montant TTC. L’acompte reprend la part demandée de chaque taux de TVA du devis, remise globale comprise, avec « Acompte de N % sur devis NUMÉRO » pour libellé. Un devis à un seul taux, ou sans TVA, donne donc une seule ligne ; sans TVA, la mention d’exonération du devis est conservée. Avec plusieurs taux, on obtient une ligne par taux, ce qui garde la TVA exacte. Un montant se convertit en pourcentage du total TTC, d’où un écart possible d’un centime par taux.

## Historique d’un client

Dans Clients, « Historique » (ou un clic sur le nom) ouvre une fiche listant les documents dont le destinataire porte le même nom, sans tenir compte des majuscules ni des espaces autour : type, numéro et statut, date d’émission, montant TTC et reste à payer pour les factures, avec « Voir et agir ». Les devis et factures archivés de Facture.net au même nom s’ajoutent en lecture seule, avec leur numéro et leur montant HT.

## Ouvrir les PDF archivés

Dans Archives, les éléments qui ont un PDF d’origine proposent « Ouvrir le PDF » (visionneuse du système) et « Enregistrer le PDF… ». Un devis ou une facture archivé affiche ces boutons dès que son PDF a été rattaché.

## Convertir un brouillon dans une autre devise

Dans l’éditeur, « Convertir dans une autre devise… » demande la devise cible, le taux (1 devise actuelle = N devise cible), sa date et sa source (par exemple « BCE »). Les prix unitaires sont convertis et arrondis à 6 décimales, puis les totaux suivent la précision de la nouvelle devise. Le PDF imprime sous les totaux « Montants convertis de EUR au taux 1 EUR = 0,9312 CHF (BCE, 18/11/2026) ». Une seule conversion par document ; pour la défaire, abandonner les modifications avant d’enregistrer.

## Langue et devise

Dans Paramètres, la section « Langue et devise » règle la langue de l’interface (français ou anglais), ainsi que la langue et la devise des nouveaux documents. Chaque document garde ensuite sa propre langue et sa devise, modifiables dans l’éditeur. Le PDF suit la langue du document : libellés, dates, format des montants et mention de pénalités par défaut. Les totaux du tableau de bord restent séparés par devise, avec un sélecteur en haut de page. Le tableau de bord s’ouvre sur la devise par défaut si elle a des documents, sinon sur celle qui en compte le plus. L’option `facturecraft gui --language en` force la langue de l’interface pour une session.

## Factur-X

À partir du 1er septembre 2027, les factures entre entreprises passent par une plateforme agréée, dans un format structuré. FactureCraft produit un fichier Factur-X pour chaque facture, facture d’acompte ou avoir finalisé : le PDF figé, converti en PDF/A-3, qui contient en pièce jointe le fichier `factur-x.xml` (format CII, profil EN 16931). Le XML reprend le numéro, la date d’émission, l’émetteur tel qu’il était à la finalisation (nom, SIREN, TVA, adresse, e-mail), le client, les lignes (quantité, unité, prix, remises), le détail de la TVA ou le motif d’exonération, l’échéance, les acomptes déduits et la facture d’origine d’un avoir.

Dans « Voir et agir », « Exporter en Factur-X » propose d’enregistrer `NUMERO-facturx.pdf`. En ligne de commande :

```console
facturecraft draft facturx ID facture-facturx.pdf
```

Le PDF figé enregistré dans la base n’est pas modifié : le fichier Factur-X est recréé à chaque export, à l’identique. Les devis et les brouillons sont refusés.

À vérifier sur la plateforme agréée lors du premier dépôt :

- le fichier est accepté sans erreur de conformité (PDF/A-3 et règles EN 16931) ; aucun validateur n’est intégré à FactureCraft ;
- l’adresse est bien découpée : la dernière ligne de l’adresse doit être « code postal ville », par exemple `75001 PARIS`, et le pays doit être indiqué en toutes lettres (France par défaut) ;
- en franchise de TVA, le motif « TVA non applicable, art. 293 B du CGI » apparaît avec la catégorie E et le code `VATEX-FR-FRANCHISE`. Certaines règles demandent aussi un numéro de TVA de l’émetteur pour la catégorie E : vérifier ce que la plateforme exige ;
- le SIREN du client est renseigné pour les clients professionnels, ainsi que les adresses électroniques attendues par la plateforme.

## Utiliser MCP dans Codex ou Hermes

Déclarer un serveur local stdio avec le chemin absolu du binaire installé comme commande et `mcp` comme argument. Pour choisir une base dédiée, ajouter `--db`, suivi de son chemin absolu, avant `mcp`. La liste des outils publiée par le serveur fournit les schémas d’entrée.

- `search_parties` recherche par nom dans le carnet de contacts local.
- `list_drafts` et `get_draft` lisent les métadonnées. `get_document` fournit le contenu, la version et les totaux.
- `create_draft` et `create_invoice_draft_from_quote` prennent une clé d’idempotence. Le devis source d’une facture doit avoir été accepté dans l’interface.
- `update_draft` prend la version attendue.
- `preview_draft` renvoie le PDF comme ressource intégrée de type `application/pdf`.
- `list_products` lit le catalogue de produits et services.
- `list_archived_documents` et `get_archived_document` lisent l’historique importé de Facture.net, en lecture seule ; le second fournit le destinataire et les lignes reconstruites, prêts à réutiliser.
- `create_invoice_draft` prépare un brouillon de facture (clé d’idempotence, échéance par défaut à 30 jours). La numérotation et l’émission restent une action manuelle dans l’interface.

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

## Sauvegarde, restauration et exports

Dans **Paramètres**, la section « Sauvegarde et restauration » affiche le chemin de la base en cours.

- **Créer une sauvegarde…** écrit une copie cohérente de la base (nom proposé : `facturecraft-AAAA-MM-JJ.sqlite3`). Un fichier existant n’est jamais écrasé.
- **Restaurer une sauvegarde…** vérifie la sauvegarde choisie (intégrité SQLite, tables attendues, version compatible) puis la copie dans un nouveau fichier `facturecraft-restaure-AAAA-MM-JJ.sqlite3`, à côté de la base actuelle. La base en cours n’est pas modifiée. Pour utiliser la copie, quittez FactureCraft puis lancez `facturecraft --db CHEMIN gui`.
- **Exporter les documents (CSV)** et **(JSON)** produisent une ligne par document : identifiant, type, numéro, statut, titre, destinataire, devise, langue, date d’émission, échéance, HT, TVA, TTC, encaissé et reste dû. Le CSV est en UTF-8 avec BOM, séparateur virgule : Excel et LibreOffice lisent correctement les accents.

En ligne de commande :

```console
facturecraft export documents documents.csv
facturecraft export documents documents.json --format json
```

Une base créée par une version plus récente de FactureCraft est refusée avec un message explicite plutôt que modifiée.
