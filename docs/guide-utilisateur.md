# Guide utilisateur de FactureCraft

## Sommaire

1. [Bienvenue](#1-bienvenue)
2. [Premier lancement et réglages](#2-premier-lancement-et-réglages)
3. [Produits & services](#3-produits--services)
4. [Clients](#4-clients)
5. [Créer un devis](#5-créer-un-devis)
6. [Finaliser, accepter, refuser](#6-finaliser-accepter-refuser)
7. [Facturer](#7-facturer)
8. [Règlements](#8-règlements)
9. [Factur-X et facture électronique](#9-factur-x-et-facture-électronique)
10. [Tableau de bord](#10-tableau-de-bord)
11. [Archives Facture.net](#11-archives-facturenet)
12. [Sauvegarde, restauration et exports](#12-sauvegarde-restauration-et-exports)
13. [Questions fréquentes](#13-questions-fréquentes)

---

## 1. Bienvenue

FactureCraft vous aide à préparer vos **devis**, à émettre vos **factures** et à suivre les paiements de votre activité. Il est conçu pour les indépendants et les micro-entrepreneurs.

Quelques points importants avant de commencer :

- **Vos données restent sur votre ordinateur.** Aucun compte à créer, aucune connexion à un service en ligne.
- **Un document finalisé ne se modifie plus.** Une fois numéroté, un devis ou une facture est figé. Pour corriger une erreur, vous devez créer un autre document (voir le chapitre 6 et le chapitre 7).
- **Les menus se trouvent à gauche de l’écran.** Ils donnent accès aux pages suivantes : **Tableau de bord**, **Devis**, **Factures**, **Clients**, **Produits & services**, **Archives** et **Paramètres**.

Pour ouvrir l’application, lancez le fichier FactureCraft comme n’importe quel logiciel. Vous arrivez sur le tableau de bord.

---

## 2. Premier lancement et réglages

Ouvrez **Paramètres** dans le menu. Cette page comporte plusieurs sections. Prenez le temps de les remplir avant d’établir votre premier document, car les informations saisies sont reprises dans chaque nouveau devis et chaque nouvelle facture.

### Apparence

La section **Apparence** contient deux réglages.

- **Couleur de l’interface et de l’icône** : cliquez sur une couleur parmi **Prune**, **Océan**, **Forêt**, **Corail** ou **Ardoise**. Le menu, les boutons et les graphiques changent de teinte immédiatement. Les badges d’état gardent leurs couleurs propres.
- **Animation de page tournée entre les vues** : cochez ou décochez cette case pour activer ou désactiver l’effet visuel lorsque vous changez de page. Ce choix n’a aucune incidence sur vos documents.

### Langue et devise

La section **Langue et devise** contient trois réglages.

- **Langue de l’interface** : **Français** ou **English**.
- **Langue des nouveaux documents** : la langue utilisée dans le PDF de chaque nouveau devis ou facture.
- **Devise des nouveaux documents** : la devise proposée par défaut.

Chaque document garde ensuite sa propre langue et sa propre devise, que vous pouvez changer dans l’éditeur.

### Identité de l’émetteur

La section **Identité de l’émetteur** regroupe les informations qui figurent sur vos documents : **Société**, **Votre contact**, **Adresse**, **Pays**, **N° SIREN**, **Numéro de TVA**, **Téléphone** et **E-mail**. Remplissez au minimum le nom, l’adresse et le numéro SIREN.

Pour les mentions de votre entreprise, voici un exemple fictif :

> Atelier Exemple
> 12 rue de l’Exemple, 75001 Paris
> contact@example.com

Vous pouvez aussi ajouter votre logo. Cliquez sur **Choisir un logo…**, puis sélectionnez une image au format PNG ou JPEG. Le logo apparaît en haut à droite de vos documents. Pour le supprimer, cliquez sur **Retirer le logo**.

Cliquez sur **Enregistrer les paramètres** pour sauvegarder. Les informations sont figées dans chaque document au moment de sa finalisation. Modifier votre adresse plus tard ne change donc pas les anciennes factures.

### Conditions par défaut

La section **Conditions par défaut** contient les textes repris dans chaque nouveau document :

- **Conditions de règlement** (par exemple : « Paiement à 30 jours »),
- **Mode de règlement** (par exemple : « Virement bancaire »),
- **Validité du devis** (par exemple : « 30 jours »),
- **Mentions complémentaires**.

Cochez **Imprimer aussi les CGV sur les factures** si vous voulez que vos conditions générales de vente figurent aussi sur les factures.

Le champ **Mention de pénalités de retard (factures)** contient le texte imprimé sous les totaux des factures. Laissez-le vide pour utiliser le texte standard, affiché en grisé. Il mentionne les pénalités de retard et l’indemnité forfaitaire de 40 € pour frais de recouvrement.

Le champ **Conditions générales de vente (page dédiée des devis, copiées dans chaque nouveau document)** contient vos conditions générales. Elles sont imprimées sur une page dédiée des devis.

### Numérotation et émission

Cette section indique le mode dans lequel vous travaillez.

- **Mode démonstration** : tant que l’émission réelle n’est pas activée, les documents finalisés reçoivent un numéro commençant par **TEST-**. Ils servent à essayer l’application. Ils ne sont pas valables comme factures.
- **Émission réelle activée** : les documents finalisés reçoivent un vrai numéro de facturation.

Cette section affiche aussi le **Prochain devis**, la **Prochaine facture**, le **Prochain acompte** et le **Prochain avoir**. Les numéros suivent le format de Facture.net. Ils continuent la série déjà utilisée : si vous avez importé vos anciens documents, la numérotation reprend après le dernier numéro existant de l’année.

**Quand activer l’émission réelle ?** Uniquement lorsque vous n’émettez plus de documents avec Facture.net. Vérifiez deux fois avant de cocher la case **Je n’émets plus de documents sur Facture.net et je confirme la reprise de la série ci-dessus.**, puis cliquez sur **Activer l’émission réelle**.

**Attention :** tant qu’aucun document n’a reçu de vrai numéro, le bouton **Revenir au mode démonstration** permet d’annuler l’activation. Dès qu’un premier numéro réel est attribué, ce retour n’est plus possible, et les numéros déjà attribués ne peuvent pas être réutilisés.

---

## 3. Produits & services

La page **Produits & services** conserve les prestations que vous facturez souvent. Vous évitez ainsi de retaper les mêmes prix.

Pour créer un produit :

1. Ouvrez **Produits & services** dans le menu.
2. Cliquez sur **+ Nouveau produit**.
3. Renseignez le nom, la description, l’unité, le **Prix unitaire HT**, la **TVA %** et, si besoin, le **Motif d’exonération**.
4. Enregistrez.

Pour modifier ou supprimer un produit, utilisez les boutons **Modifier** ou **Supprimer** sur sa ligne. La suppression demande une confirmation.

**Point important :** lorsque vous ajoutez un produit à un devis, l’application en copie les valeurs dans le document. **Modifier ou supprimer un produit ne change jamais les devis et factures existants.** Cela protège vos anciens documents.

Pour utiliser un produit dans un devis, ouvrez l’éditeur (voir le chapitre 5), puis cliquez sur **+ Depuis le catalogue** et choisissez le produit. Une ligne est ajoutée avec ses valeurs.

Si le catalogue est vide, l’écran affiche **Votre catalogue est vide**.

---

## 4. Clients

La page **Clients** est votre carnet d’adresses. Elle regroupe vos clients et leurs sociétés.

### Ajouter un client

1. Cliquez sur **+ Nouveau client**.
2. Renseignez le **Nom ou société**, l’**Adresse**, le **Pays**, le **N° d’entreprise** (pour un client professionnel) et l’**E-mail**.
3. Enregistrez.

Par exemple : Nom ou société : « Boutique Exemple » ; Adresse : « 3 avenue de l’Exemple, 69001 Lyon » ; E-mail : « contact@example.com ».

### Modifier ou supprimer un client

Cliquez sur **Modifier** pour changer les informations. Cliquez sur **Supprimer** pour retirer le contact du carnet, après confirmation.

**Supprimer un client n’efface pas ses documents.** Chaque devis et chaque facture conserve sa propre copie du nom et de l’adresse du destinataire, telle qu’elle était au moment de la création. Vos anciens documents restent donc inchangés.

### Les contacts importés

Si vous avez importé vos données depuis Facture.net, vos anciens contacts apparaissent dans la liste. Ils sont en **lecture seule** : vous ne pouvez pas les modifier. Pour les utiliser, cliquez sur **Copier dans le carnet**. Une copie modifiable est alors créée.

### Historique d’un client

Cliquez sur **Historique** (ou sur le nom du client) pour voir tous les documents adressés à ce client. La liste indique le type, le numéro, le statut, la date, le montant TTC et le **Reste à payer** pour les factures. Depuis cette fiche, le bouton **Voir et agir** ouvre le document.

Les devis et factures archivés de Facture.net au même nom apparaissent aussi, en lecture seule.

---

## 5. Créer un devis

### Ouvrir l’éditeur

1. Ouvrez **Devis** dans le menu.
2. Cliquez sur **+ Nouveau devis**. Si la liste est vide, cliquez sur **Commencer un devis**.

L’éditeur s’ouvre. Il reprend automatiquement les conditions par défaut de vos paramètres.

### Les champs de l’éditeur

**En-tête du document**

- **Titre du devis** : par exemple « Refonte du site web ». Il sert à retrouver le document dans vos listes ; il n’est pas imprimé sur le PDF, qui affiche « Devis Provisoire » puis le numéro une fois finalisé.
- **Devise** et **Langue du document** : elles sont proposées par défaut, mais vous pouvez les changer pour ce document.
- **Destinataire** : cliquez sur **Choisir dans le carnet** pour sélectionner un client. Vous pouvez aussi écrire directement le nom et l’adresse. Le bouton **Ajouter au carnet** enregistre ce destinataire dans vos clients.

**Prestations**

Chaque ligne comporte une **Description**, une **Qté**, un **Prix HT**, une **Remise %** et une **TVA %**.

- **Première ligne de la description : l’intitulé.** Le détail se place en dessous, sur autant de paragraphes que nécessaire. Dans le PDF, l’intitulé vient en premier, suivi du détail.
- **+ Ajouter une ligne** ajoute une ligne vide.
- **+ Depuis le catalogue** ajoute une ligne à partir d’un produit enregistré (chapitre 3).
- **Remise globale %** applique une réduction à l’ensemble du document.

Pour vérifier les montants, l’application affiche le total HT, la TVA et le total TTC. Si le total ne peut pas être calculé, un message vous invite à vérifier les montants, les descriptions et la devise.

**Conditions**

- **Conditions de règlement**, **Mode de règlement** et **Validité du devis** sont repris des paramètres.
- **Conditions particulières (page Conditions du PDF)** : textes spécifiques à ce devis, par exemple les délais de réalisation.
- **Conditions générales de vente de ce document** : une copie des CGV de vos paramètres. Vous pouvez l’adapter ici sans modifier le modèle. Le bouton **Recharger le modèle** reprend le texte de vos paramètres.

Pour une facture, ce bloc contient aussi l’**Échéance (AAAA-MM-JJ, factures)** et la **Date de prestation (factures)** (chapitre 7).

**Étiquettes et notes internes**

- **Étiquettes (séparées par des virgules, privées)** : par exemple « chantier, 2026 ». Elles facilitent la recherche.
- **Notes internes (jamais imprimées sur le PDF)** : vos remarques personnelles.

Ces deux champs restent privés. Le client ne les voit jamais.

### Convertir un brouillon dans une autre devise

Si vous devez facturer dans une autre devise, cliquez sur **Convertir dans une autre devise…** dans l’éditeur d’un brouillon. Choisissez la **Devise cible**, indiquez le taux (« 1 EUR = … »), la **Date du taux (AAAA-MM-JJ)** et la **Source** (par exemple « BCE »), puis cliquez sur **Convertir**. Les prix unitaires sont convertis. Le taux, sa date et sa source sont imprimés sous les totaux du PDF. Un document ne peut être converti qu’une seule fois. La conversion convient aux documents créés directement : une facture ou un acompte issu d’un devis doit rester dans la devise du devis, sinon la finalisation est refusée.

### Mise en forme du texte

Vous pouvez mettre en forme les descriptions, les conditions et les CGV avec trois règles simples :

- entourez un mot de deux astérisques pour le mettre en gras, par exemple `**Forfait global : 8 000 €.**` ;
- commencez une ligne par `#` suivi d’un espace pour créer un intertitre en gras, par exemple `# Livrables` ;
- laissez une ligne vide pour commencer un nouveau paragraphe.

### Enregistrer, prévisualiser, exporter

- **Aperçu du brouillon** : ce panneau, à droite de l’éditeur, se met à jour pendant la saisie. Pour voir le PDF exact, utilisez **Exporter le PDF brouillon**.
- **Enregistrer le brouillon** : sauvegarde le document sans le numéroter. Vous pourrez le reprendre plus tard. Le document reste modifiable.
- **Exporter le PDF brouillon** : crée un PDF de travail, à envoyer pour relecture. Ce PDF ne porte pas de numéro officiel : à la place du numéro, il affiche « Provisoire ».

Si vous quittez l’éditeur sans enregistrer, l’application demande une confirmation avec **Abandonner les modifications ?**. Cliquez sur **Continuer la saisie** pour revenir au document, ou sur **Abandonner** pour perdre les changements.

---

## 6. Finaliser, accepter, refuser

### Finaliser un document

Un brouillon devient un document officiel lorsque vous le **finalisez**. Pour cela :

1. Ouvrez le document dans la liste **Devis** ou **Factures**.
2. Cliquez sur **Voir et agir**. La fenêtre **Vérifier le document** s’ouvre et récapitule le document.
3. Cliquez sur **Finaliser et numéroter**.

Si la numérotation réelle n’est pas activée, le bouton s’intitule **Finaliser la démonstration** et le numéro commence par **TEST-**.

**Ce que fait la finalisation :**

- le document reçoit son numéro définitif, construit à partir du type de document, du mois, de l’année et d’un compteur ;
- le PDF est figé. Il ne changera plus, même si vous modifiez vos paramètres par la suite ;
- le document n’est plus modifiable. Vous ne pouvez plus changer ses lignes ni ses conditions.

**La finalisation est irréversible.** Vérifiez donc le document avec **Exporter le PDF brouillon** avant de confirmer. Pour une facture, la date d’échéance doit être renseignée. Sinon, l’application vous le signale : « Indiquez la date d’échéance dans le document avant de le finaliser. »

### Dupliquer un document

Dans **Voir et agir**, le bouton **Dupliquer** crée un nouveau brouillon copié à partir d’un devis ou d’une facture. Son titre commence par « Copie de ». C’est la bonne méthode pour refaire un devis proche d’un ancien, ou pour corriger une facture (voir le chapitre 7).

### Supprimer un brouillon

Le bouton **Supprimer le brouillon** n’apparaît que pour un document non finalisé. Cliquez ensuite sur **Confirmer la suppression**. Un document finalisé ne peut jamais être supprimé.

### Accepter, refuser ou annuler un devis

Un devis finalisé peut recevoir un statut :

1. Ouvrez **Voir et agir** sur le devis.
2. Renseignez la **Date (AAAA-MM-JJ)** de la décision.
3. Cliquez sur **Marquer accepté**, **Marquer refusé** ou **Marquer annulé**.

Un devis accepté peut ensuite être transformé en facture (chapitre 7). Dans la liste, les statuts s’affichent sous forme de badges : **Finalisé**, **Accepté**, **Refusé** ou **Annulé**.

---

## 7. Facturer

### Créer une facture à partir d’un devis accepté

Une facture se crée à partir d’un devis **accepté**.

1. Ouvrez **Voir et agir** sur le devis.
2. Cliquez sur **Créer une facture**.
3. La liste **Lignes à facturer** s’affiche. Chaque ligne du devis comporte une case à cocher, cochée par défaut.
4. Décochez les lignes que vous ne voulez pas facturer, puis cliquez sur **Créer la facture**.

La facture est créée en **brouillon**. Elle reçoit une **échéance à 30 jours**, que vous pouvez modifier. Vous pouvez aussi ajouter une **Date de prestation**. Ces deux dates apparaissent sous la date du document.

### Créer une facture directement

Dans **Factures**, cliquez sur **+ Nouvelle facture** pour rédiger une facture sans devis. Vous remplissez l’éditeur comme pour un devis (chapitre 5). Pour finaliser, renseignez obligatoirement la date d’échéance.

### Créer un acompte

Un acompte est une facture partielle payée à l’avance.

1. Ouvrez **Voir et agir** sur un devis accepté.
2. Cliquez sur **Créer un acompte**.
3. Choisissez le mode :
   - **Pourcentage** : par défaut **30 %**.
   - **Montant TTC** : un montant exact dans la devise du document.
4. Cliquez sur **Créer l’acompte**.

L’acompte reprend la part demandée de chaque taux de TVA du devis. Son libellé est « Acompte de N % sur devis NUMÉRO ». Si le devis n’a qu’un seul taux de TVA, l’acompte n’a qu’une ligne. Si le devis en contient plusieurs, l’acompte en affiche une par taux, ce qui garde la TVA exacte.

**Bon à savoir :** lorsque vous indiquez un montant, l’application le convertit en pourcentage du total TTC. Un écart d’un centime est donc possible.

### Appliquer un acompte à une facture

Pour déduire un acompte d’une facture finale, ouvrez la facture en brouillon avec **Voir et agir**, puis choisissez l’acompte dans la liste **Choisir un acompte** et cliquez sur **Affecter**. Le montant de l’acompte est alors déduit de la facture. Le texte du bouton peut mentionner « démonstration » tant que la numérotation réelle n’est pas active.

### Créer un avoir

Un **avoir** est un document qui annule tout ou partie d’une facture. Il est nécessaire pour corriger une facture déjà finalisée, car une facture finalisée ne se modifie pas.

1. Ouvrez la facture concernée avec **Voir et agir**.
2. Cliquez sur **Créer un avoir**.
3. Vérifiez le brouillon d’avoir, puis finalisez-le.

L’avoir apparaît avec le statut **Avoir émis** et réduit le chiffre d’affaires de la période (chapitre 10).

---

## 8. Règlements

Vous pouvez noter chaque paiement reçu sur une facture ou un acompte.

### Enregistrer un règlement

1. Ouvrez la facture avec **Voir et agir**.
2. Une ligne récapitule les **Acomptes**, le montant **Réglé**, les **Avoirs** et le **Reste à payer**.
3. Renseignez le **Montant**, la **Date AAAA-MM-JJ**, le **Mode** (par exemple « Virement ») et la **Référence** (par exemple un numéro de virement).
4. Cliquez sur **Enregistrer le règlement**.

Les règlements reçus apparaissent dans la section **Règlements reçus**.

### Annuler un règlement saisi par erreur

Dans la liste **Règlements reçus**, cliquez sur **Annuler ce règlement** à côté du paiement concerné, puis sur **Confirmer**. Le montant est retiré du total encaissé et le reste à payer est recalculé.

### Les statuts de paiement

Chaque facture affiche l’un de ces statuts :

- **En attente** : aucun paiement n’a été enregistré.
- **Partiellement payée** : une partie du montant est réglée.
- **Payée** : le reste à payer est nul.

---

## 9. Factur-X et facture électronique

### Qu’est-ce que le Factur-X ?

Le Factur-X est un format de facture qui combine un PDF lisible par l’humain et un fichier structuré (XML) lisible par un logiciel. FactureCraft crée ce fichier pour chaque facture, facture d’acompte ou avoir finalisé.

### Exporter un fichier Factur-X

1. Ouvrez la facture finalisée avec **Voir et agir**.
2. Cliquez sur **Exporter en Factur-X**.
3. Choisissez où enregistrer le fichier. Le nom proposé est du type « F012700001-facturx.pdf ».

Le PDF d’origine enregistré dans FactureCraft n’est pas modifié. Le fichier Factur-X est recréé à l’identique à chaque export. Les devis et les brouillons ne peuvent pas être exportés en Factur-X.

### Ce que vous devez faire ensuite

Le fichier Factur-X ne s’envoie pas tel quel à l’administration. Vous devez le **déposer sur une plateforme de facturation agréée**, que vous choisissez et qui transmettra la facture.

**Calendrier (à vérifier sur impots.gouv.fr) :**

- depuis le **1er septembre 2026**, les entreprises doivent être en mesure de **recevoir** des factures électroniques ;
- à partir du **1er septembre 2027**, les micro-entreprises et les TPE doivent **émettre** leurs factures électroniques via une plateforme.

Le calendrier peut évoluer. Consultez le site officiel avant d’organiser votre facturation.

### Points à confirmer avec votre plateforme

Lors du premier dépôt, vérifiez les points suivants :

- **Conformité du fichier.** La plateforme doit accepter le fichier sans erreur. FactureCraft ne contient pas de vérificateur intégré.
- **Adresse.** La dernière ligne de l’adresse doit être « code postal ville », par exemple « 75001 PARIS ». Le pays doit être écrit en toutes lettres (France par défaut).
- **Franchise en base de TVA.** Si vous n’êtes pas assujetti à la TVA, la facture porte la mention « TVA non applicable, art. 293 B du CGI » avec le code d’exonération `VATEX-FR-FRANCHISE`. Certaines plateformes demandent aussi votre numéro de TVA, même en franchise. Vérifiez ce qu’elles exigent.
- **Client professionnel.** Le numéro SIREN du client doit figurer pour un client professionnel, ainsi que les adresses électroniques attendues par la plateforme.

---

## 10. Tableau de bord

Le **Tableau de bord** est la page d’accueil. Il résume votre activité pour une année et une devise.

### Les boutons de navigation

- **◀** et **▶** changent d’année. Survolez-les pour lire « Année précédente » ou « Année suivante ».
- Si vous travaillez dans plusieurs devises, un sélecteur apparaît en haut à droite. Choisissez la devise à afficher.

### Les chiffres

- **Devis émis en [année]** et **Factures émises en [année]** : le nombre de documents.
- **CA HT ce mois** : le chiffre d’affaires hors taxes du mois en cours.
- **En attente de règlement** : le montant restant à encaisser.
- **Chiffre d’affaires HT** : le graphique mois par mois de l’année.
- **Dernières activités** : les derniers devis, factures, acomptes, avoirs et règlements reçus.

### Ce que comptent ces chiffres

- Le **chiffre d’affaires HT** correspond aux factures **moins** les avoirs. Les **acomptes** ne sont pas comptés, car ils sont déduits de la facture finale.
- Les documents de **démonstration** (numéro **TEST-**) ne sont pas comptés.
- Le montant **En attente de règlement** ne couvre que les documents émis dans FactureCraft. Les anciennes factures importées de Facture.net ne sont pas prises en compte, car leur historique de paiements peut être incomplet.

---

## 11. Archives Facture.net

La page **Archives** contient l’historique de vos documents émis avec Facture.net, importé dans FactureCraft. Elle est en **lecture seule** : vous consultez, vous ne modifiez pas.

Vous y trouvez les devis et factures anciens, avec leur numéro, leur date et leurs montants. La barre **Rechercher** permet de filtrer la liste.

Lorsqu’un document archivé possède son PDF d’origine, deux boutons apparaissent :

- **Ouvrir le PDF** affiche le document dans la visionneuse de votre ordinateur ;
- **Enregistrer le PDF…** copie le fichier à l’endroit de votre choix.

Si la section affiche « Aucune archive importée. », aucun historique n’a été importé.

---

## 12. Sauvegarde, restauration et exports

Toutes vos données sont dans un seul fichier, sur votre ordinateur. Une sauvegarde régulière vous protège contre la perte ou la panne du matériel.

### Créer une sauvegarde

1. Ouvrez **Paramètres**.
2. Dans la section **Sauvegarde et restauration**, vérifiez le chemin de la **Base de données actuelle**.
3. Cliquez sur **Créer une sauvegarde…**.
4. Choisissez l’emplacement. Le nom proposé contient la date, par exemple « facturecraft-2026-11-18 ».

Conservez vos sauvegardes sur un autre support, par exemple un disque dur externe. Un fichier existant n’est jamais écrasé.

### Restaurer une sauvegarde

1. Dans la même section, cliquez sur **Restaurer une sauvegarde…**.
2. Choisissez le fichier de sauvegarde.

FactureCraft vérifie d’abord que la sauvegarde est intacte et compatible. Il la copie ensuite dans un **nouveau fichier**, nommé « facturecraft-restaure-[date] », placé à côté de votre base actuelle.

**Votre base actuelle n’est jamais remplacée.** Pour utiliser la copie restaurée, il faut quitter FactureCraft puis l’ouvrir en lui indiquant ce fichier. Cette opération se fait en ligne de commande ; le guide technique (docs/utilisation.md) en détaille la procédure.

### Exporter vos documents

La section **Exporter les documents** produit un tableau qui récapitule vos documents : identifiant, type, numéro, statut, titre, destinataire, devise, langue, date d’émission, échéance, montants HT, TVA et TTC, montant encaissé et reste dû.

- **Exporter les documents (CSV)** crée un fichier lisible dans Excel ou LibreOffice. Les accents sont correctement affichés. C’est le format à transmettre à votre expert-comptable.
- **Exporter les documents (JSON)** produit un fichier destiné aux logiciels et aux personnes techniques. Pour votre comptable, préférez le CSV.

Les exports ne modifient pas vos documents. Ils servent uniquement à consulter ou transmettre vos données.

---

## 13. Questions fréquentes

### J’ai fait une erreur sur une facture finalisée. Que faire ?

Une facture finalisée ne se modifie pas. Créez un **avoir** avec **Créer un avoir** dans **Voir et agir**, puis finalisez-le. Si la facture doit être refaite, cliquez ensuite sur **Dupliquer** pour créer un brouillon corrigé, et finalisez-le. Ainsi, le document d’origine reste tel qu’il a été émis.

### Pourquoi mon devis s’appelle « Provisoire » ?

Tant qu’un document n’est pas finalisé, il n’a pas encore de numéro. Son PDF affiche donc « Provisoire » à la place du numéro. Une fois finalisé, le numéro officiel apparaît.

### Comment changer la couleur de l’application ?

Ouvrez **Paramètres**, puis la section **Apparence**. Cliquez sur la couleur de votre choix : **Prune**, **Océan**, **Forêt**, **Corail** ou **Ardoise**.

### Où sont mes données ?

Elles sont sur votre ordinateur, dans un fichier géré par FactureCraft. Le chemin exact est affiché dans **Paramètres**, section **Sauvegarde et restauration**, sous **Base de données actuelle**. FactureCraft ne demande aucun compte.

### Comment passer en anglais ?

Ouvrez **Paramètres**, section **Langue et devise**, puis choisissez **English** dans **Langue de l’interface**. Cela change l’interface. Les documents gardent leur propre langue, réglée séparément dans **Langue des nouveaux documents**.

### Puis-je supprimer un client utilisé dans un document ?

Oui. Les documents conservent leur propre copie du nom et de l’adresse du destinataire. Supprimer le client du carnet n’altère donc ni vos devis ni vos factures.

### Puis-je modifier un devis déjà accepté ?

Non, un devis finalisé ne se modifie plus. Utilisez **Dupliquer** pour créer un nouveau devis basé sur celui-ci, puis finalisez la nouvelle version.

### Pourquoi mes chiffres du tableau de bord sont-ils différents de ce que j’attendais ?

Le chiffre d’affaires est calculé à partir des factures **moins** les avoirs, **hors acomptes**. Les documents de démonstration (numéro **TEST-**) ne sont pas comptés. Vérifiez aussi que la bonne année et la bonne devise sont sélectionnées.

### Dois-je activer l’émission réelle tout de suite ?

Non. Tant que vous établissez encore des documents avec Facture.net, restez en **mode démonstration**. Activez l’émission réelle seulement lorsque FactureCraft devient votre seul outil de facturation (chapitre 2).

### Mon document a un numéro « TEST- ». Est-ce grave ?

Non, si vous êtes en mode démonstration. Ce numéro indique que le document est un essai, qui n’a pas de valeur comptable. Pour émettre de vrais documents, activez l’émission réelle dans **Paramètres**.
