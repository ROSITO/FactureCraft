# Direction d’interface de FactureCraft

**Référence principale : Indy.** Sur ordinateur, le formulaire et le document évoluent côte à côte. Ce parcours convient à une personne qui prépare quelques devis par an et souhaite voir immédiatement ce que recevra son client.

| Référence | Parcours documenté | Décision FactureCraft |
|---|---|---|
| [Indy](https://wikicompta.indy.fr/fr/articles/6623102-comment-creer-une-facture-sur-indy) | Formulaire et aperçu en temps réel | Éditeur à gauche, aperçu à droite |
| [Pennylane](https://help.pennylane.com/fr/articles/18642-creer-un-devis) | Aperçu PDF explicite, duplication, conversion | Aperçu du PDF réel avant finalisation, conversion depuis le devis |
| [Zoho Invoice](https://www.zoho.com/in/invoice/help/invoice/new-invoice.html) | Ajout d’un client depuis l’éditeur, raccourcis, préférences séparées | Ajouter un destinataire sans quitter le devis, regrouper les réglages avancés |
| Facture.net, relevé du PRD | États depuis la liste, téléchargement et documents liés depuis le détail | Filtres simples, historique et pièces liées au document |

Le benchmark compare des parcours documentés. Il ne mesure ni les taux de réussite ni la satisfaction des utilisateurs. Le choix se validera par une tâche réelle avec un jeu synthétique : retrouver un devis, en créer un pour un nouveau client, ajouter deux lignes, vérifier le total et reprendre le brouillon après fermeture.

## Parcours cible

1. Liste des devis avec recherche, statut, destinataire et total.
2. Éditeur de devis organisé par destinataire, prestations et conditions. En fenêtre étroite, l’aperçu passe sous le formulaire.
3. Aperçu de travail mis à jour pendant la saisie, identifié comme brouillon. L’aperçu PDF définitif utilisera le même moteur de rendu que le fichier exporté.
4. Enregistrement du brouillon au premier plan. Finalisation humaine dans un parcours distinct quand la fonction sera disponible.
5. Identité propre à FactureCraft : accent prune, écureuil, fond papier pour le document et interface native sobre.

Les tests et visuels publics utilisent uniquement des exemples synthétiques. Les données privées restent dans la base locale.


## Direction artistique retenue

Le kit fourni fait référence pour l’identité de FactureCraft. La barre latérale est prune profonde, le fond général est crème, les cartes et l’aperçu sont en papier clair. Le prune sert aux actions et à la navigation active, l’ambre aux accents ponctuels, le lilas aux sélections et le charbon au texte. Le vert est réservé aux états positifs, avec un libellé explicite. La police d’interface Sora est embarquée localement sous licence OFL. L’écureuil sert de marque dans la navigation et l’icône de l’application.

L’éditeur conserve l’agencement inspiré d’Indy. Le kit guide les écrans métier à mesure de leur réalisation. Les chiffres, noms, statuts et écrans mobiles illustrés dans le kit servent uniquement de références graphiques.
