# Local et GitHub

La copie de travail personnelle garde les exports Facture.net, les bases SQLite et leurs fichiers annexes, les sauvegardes et les documents générés. Utiliser le dossier de données de l’application ou un chemin hors de la checkout publique pour toute base réelle.

Publier depuis une checkout distincte. Copier uniquement les chemins autorisés pour le changement, puis contrôler le diff indexé avant tout push. Le dépôt public reçoit les sources, l’icône officielle, les fichiers Cargo, les workflows et la documentation publique. Les tests fabriquent leurs données dans une base temporaire.

Les noms de clients, factures, devis, pièces jointes, configurations personnelles et rapports de migration restent locaux. Vérifier les captures d’écran et exemples avant publication. Les règles d’exclusion Git complètent cette séparation.

Le workflow Rust vérifie les tests sur macOS, Windows et Linux. Un lancement manuel construit aussi les archives portables et les joint au résultat du workflow. Les archives contiennent le binaire, la documentation d’utilisation et les licences des polices embarquées. Le binaire est compilé pour l’architecture du runner indiquée dans le nom de l’archive. Les archives sont livrées sans signature ni installateur.
