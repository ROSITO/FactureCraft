# Local et GitHub

La copie de travail personnelle garde les exports Facture.net, les bases SQLite et leurs fichiers annexes, les sauvegardes et les documents générés. Utiliser le dossier de données de l’application ou un chemin hors de la checkout publique pour toute base réelle.

Publier depuis une checkout distincte. Copier uniquement les chemins autorisés pour le changement, puis contrôler le diff indexé avant tout push. Le dépôt public reçoit les sources, l’icône officielle, les fichiers Cargo, les workflows et la documentation publique. Les tests fabriquent leurs données dans une base temporaire.

Les noms de clients, factures, devis, pièces jointes, configurations personnelles et rapports de migration restent locaux. Vérifier les captures d’écran et exemples avant publication. Les règles d’exclusion Git complètent cette séparation.

Le workflow Rust vérifie les tests sur macOS, Windows et Linux. Un lancement manuel construit aussi les archives et les joint au résultat du workflow. L’archive macOS contient une application `.app` à copier dans Applications ; les archives Windows et Linux contiennent le binaire portable. Toutes contiennent les deux guides d’utilisation et les licences des polices embarquées. Le binaire est compilé pour l’architecture du runner indiquée dans le nom de l’archive. Les archives sont livrées sans signature ni notarisation.

## Vérification avant distribution

Lancer manuellement le workflow `Rust` depuis GitHub Actions et vérifier les trois tâches ainsi que les archives jointes. Pour chaque système, extraire l’archive sur une machine de l’architecture indiquée, vérifier `facturecraft --version`, l’ouverture de l’interface, une base de test et la sauvegarde/restauration. La réussite de la compilation seule ne prouve pas que l’interface fonctionne sur la machine cible. Les archives de workflow sont des artefacts temporaires, pas une publication ni un mécanisme de mise à jour.

Pour construire localement l’archive macOS après `cargo build --package facturecraft --release --locked`, exécuter `scripts/package-macos.sh`. Le script produit une archive non signée dans `distribution/`, avec le binaire de l’architecture locale. Il refuse d’écraser une archive existante.

Pour distribuer une application macOS signée et notariée, il faut notamment une identité Developer ID Application valide, les accès Apple nécessaires à la notarisation et un paquet adapté ; une identité Apple Development ne suffit pas pour cette distribution. Aucune signature ni notarisation n’est effectuée par le workflow actuel. Windows et Linux restent également des archives portables non signées, sans installateur ni mise à jour automatique.
