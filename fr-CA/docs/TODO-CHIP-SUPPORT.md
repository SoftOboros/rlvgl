# MIGRATIONS.md

Guide complet pour la gestion des migrations Django dans les environnements de développement local et de production GCP.

## Table des matières
- [Architecture de la base de données](#architecture-de-la-base-de-données)
- [Flux de travail de développement](#flux-de-travail-de-développement)
- [Commandes locales](#commandes-locales)
- [Déploiement GCP](#déploiement-gcp)
- [Regroupement des migrations](#regroupement-des-migrations)
- [Manipulations de la base de données](#manipulations-de-la-base-de-données)
- [Dépannage](#dépannage)

---

## Architecture de la base de données

Ce projet utilise une architecture de base de données scindée pour l'évolutivité. Chaque base de données a son propre historique de migration.

| Alias de la base de données | Objectif                                     | Modèles clés                                  |
|-----------------------------|----------------------------------------------|-----------------------------------------------|
| `default`                   | Données utilisateur de base, références inter-applications | User, UserEmailAddress                        |
| `memalpha`                  | Métadonnées RAG                              | MADocument, MAText, MAChunk, MACorpus, MANotebook |
| `memalpha_vectors`          | Incorporations pgvector                      | MAEmbedding                                   |
| `media`                     | Métadonnées de stockage de fichiers          | UserPublicMedia, UserPrivateMedia             |
| `mailman`                   | Intégration de courriel                      | UserEmailMessage, UserEmailAttachment         |
| `pdf_ingest`                | Traitement de PDF                            | PdfDocument, IngestEventLog, TranslationSpec  |

**Routeurs de base de données:** Situés dans le fichier `router.py` de chaque application. Ceux-ci acheminent automatiquement les modèles vers la bonne base de données.

**Clés étrangères inter-bases de données:** Utilisez `db_constraint=False` car PostgreSQL ne peut pas appliquer de contraintes de clé étrangère entre les bases de données.

---

## Flux de travail de développement

### Création de migrations

```bash
# À l'intérieur du conteneur webslinger
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations <app_name>"

# Exemple: créer une migration pour l'application memalpha
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations memalpha"

# Créer des migrations pour toutes les applications avec des changements en attente
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations"
```

### Application des migrations

```bash
# Appliquer toutes les migrations (utilise la cible Makefile)
make local-migrate

# Ou appliquer manuellement à une base de données spécifique
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=memalpha"
```

### Vérification de l'état des migrations

```bash
# Vérifier les migrations non appliquées et les changements de modèle en attente
make local-migrate-check

# Afficher toutes les migrations et leur état
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations"

# Afficher les migrations pour une application spécifique
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations memalpha"

# Vérifier si les modèles ont des changements non migrés
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations --check --dry-run"
```

### Meilleures pratiques

1. **Un seul changement logique par migration:** Ne mélangez pas les changements de modèle non liés
2. **Engagez les migrations avec le code:** Les fichiers de migration doivent être dans le même commit que les changements de modèle
3. **Ne jamais modifier les migrations appliquées:** Créez de nouvelles migrations pour corriger les problèmes
4. **Testez les migrations localement:** Exécutez toujours `make local-migrate` avant de pousser
5. **Nommez les migrations de manière descriptive:** Utilisez le drapeau `--name` : `makemigrations memalpha --name add_corpus_description`

---

## Commandes locales

### Déploiement local complet (reflète GCP)

```bash
# Déploiement standard: télécharger les images, démarrer les services, s'assurer des BD, migrer
make local-deploy

# Forcer la reconstruction des images puis le déploiement
make local-deploy-force
```

### Commandes spécifiques aux migrations

```bash
# Exécuter toutes les migrations sur toutes les bases de données
make local-migrate

# Vérifier l'état des migrations
make local-migrate-check

# Générer du SQL sans l'exécuter (aperçu)
docker compose exec webslinger bash -lc "cd backend && python manage.py sqlmigrate <app> <migration_name>"

# Exemple: voir le SQL pour memalpha 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py sqlmigrate memalpha 0014"
```

---

## Déploiement GCP

### Comment le déploiement GCP gère les migrations

La commande `make gcp-deploy` (via `scripts/gcp/deploy_compose.sh`) automatiquement:

1. S'assure que toutes les bases de données scindées existent
2. Exécute les migrations sur la base de données `default`
3. Exécute les migrations sur chaque base de données scindée: `memalpha`, `memalpha_vectors`, `media`, `mailman`, `pdf_ingest`

### Liste de vérification avant le déploiement

Avant de déployer sur GCP:

```bash
# 1. S'assurer que toutes les migrations sont commises
git status

# 2. Vérifier que les migrations s'appliquent proprement localement
make local-migrate

# 3. Vérifier qu'il n'y a pas de changements de modèle en attente
make local-migrate-check

# 4. Tester que la fonctionnalité/correction fonctionne localement
# ... exécuter vos tests ...

# 5. Pousser vers la branche cible
git push origin <branch>
```

### Déploiement sur GCP

```bash
# Déployer la branche principale (par défaut)
make gcp-deploy

# Déployer une branche spécifique
GIT_REF=origin/feature-branch make gcp-deploy

# Déployer avec une version secrète spécifique
SECRET_NAME=SoftOboros SECRET_VERSION=5 make gcp-deploy
```

### Surveillance du déploiement

```bash
# SSH vers la VM GCP
make gcp-ssh

# Exécuter dans webslinger sur GCP
make gcp-compose-exec SERVICE=webslinger

# Vérifier l'état des migrations sur GCP
make gcp-compose-exec SERVICE=webslinger CMD="cd backend && python manage.py showmigrations"
```

---

## Regroupement des migrations

### Quand regrouper

- **Avant la promotion en production:** Regrouper les migrations de la branche de test avant de les fusionner avec la branche principale
- **Migrations accumulées:** Lorsqu'une application a plus de 3 migrations issues du développement itératif
- **Nettoyage:** Lorsque les migrations référencent des modèles/champs supprimés
- **Performance:** Les longues chaînes de migration ralentissent les nouveaux déploiements

### Procédure de regroupement

#### Étape 1: Vérifier l'état propre

```bash
# Toutes les migrations doivent être appliquées
make local-migrate-check

# Ne devrait afficher aucune migration non appliquée
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations <app>"
```

#### Étape 2: Sauvegarde (si les données importent)

```bash
# Sauvegarder une base de données spécifique
docker compose exec db pg_dump -U postgres -d memalpha > memalpha_backup.sql

# Sauvegarder toutes les bases de données
docker compose exec db pg_dump -U postgres -d softoboros > default_backup.sql
docker compose exec db pg_dump -U postgres -d memalpha > memalpha_backup.sql
# ... répéter pour les autres bases de données
```

#### Étape 3: Regrouper les migrations

```bash
# Regrouper une plage de migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> <start_migration> <end_migration>"

# Exemple: regrouper les migrations memalpha 0010 à 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations memalpha 0010 0014"

# Regrouper toutes les migrations pour une application (à utiliser avec prudence)
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> 0001"
```

#### Étape 4: Examiner la migration regroupée

La commande crée un fichier comme `0010_squashed_0014_<name>.py`. Examinez-le:

- Vérifiez que la liste `replaces = [...]` contient toutes les migrations regroupées
- Vérifiez que la liste `operations` est correcte
- Recherchez les opérations `RunPython` qui pourraient nécessiter un ajustement

#### Étape 5: Tester sur une nouvelle base de données

```bash
# Supprimer et recréer la base de données spécifique
docker compose exec db dropdb -U postgres memalpha
docker compose exec db createdb -U postgres -T template0 memalpha

# Exécuter les migrations - la migration regroupée devrait s'appliquer proprement
make local-migrate

# Vérifier que l'application fonctionne correctement
```

#### Étape 6: Commiter et déployer

```bash
# Commiter la migration regroupée
git add backend/<app>/migrations/
git commit -m "chore(<app>): squash migrations 0010-0014"
```

#### Étape 7: Supprimer les anciennes migrations (après déploiement)

**Seulement après que TOUS les environnements ont appliqué la migration regroupée:**

1. Supprimer les fichiers de migration individuels qui ont été regroupés
2. Modifier la migration regroupée pour supprimer la ligne `replaces = [...]`
3. Commiter le nettoyage

```bash
# Exemple de nettoyage
rm backend/memalpha/migrations/0010_*.py
rm backend/memalpha/migrations/0011_*.py
rm backend/memalpha/migrations/0012_*.py
rm backend/memalpha/migrations/0013_*.py
rm backend/memalpha/migrations/0014_*.py

# Modifier le fichier regroupé pour supprimer l'attribut 'replaces'
# Puis commiter
git add -A && git commit -m "chore(memalpha): remove squashed migration sources"
```

---

## Manipulations de la base de données

### Réinitialisation d'une seule base de données

```bash
# Supprimer et recréer (DÉTRUIT TOUTES LES DONNÉES)
docker compose exec db dropdb -U postgres memalpha
docker compose exec db createdb -U postgres -T template0 memalpha

# Ré-exécuter les migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=memalpha"
```

### Réinitialisation de toutes les bases de données scindées

```bash
# Utiliser le drapeau RESET_SPLIT_DBS avec local-deploy (préserve la base de données par défaut)
RESET_SPLIT_DBS=1 make local-deploy
```

### Fausses migrations

Utiliser lorsque l'état de la migration n'est pas synchronisé avec le schéma de la base de données:

```bash
# Marquer la migration comme appliquée sans l'exécuter
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"

# Exemple: appliquer fictivement memalpha 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate memalpha 0014 --fake --database=memalpha"

# Appliquer fictivement toutes les migrations pour une application (dangereux - utiliser uniquement lorsque le schéma de la BD correspond)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> --fake --database=<alias>"
```

### Annulation des migrations

```bash
# Annuler une migration spécifique (PERDRA LES DONNÉES des migrations annulées)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <target_migration> --database=<alias>"

# Exemple: annuler memalpha à 0012
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate memalpha 0012 --database=memalpha"

# Annuler toutes les migrations pour une application (réinitialiser à zéro)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> zero --database=<alias>"
```

### Accès au shell de la base de données

```bash
# Shell PostgreSQL (local)
docker compose exec db psql -U postgres -d softoboros

# Shell PostgreSQL pour une base de données spécifique
docker compose exec db psql -U postgres -d memalpha

# Shell de base de données Django
docker compose exec webslinger bash -lc "cd backend && python manage.py dbshell --database=memalpha"
```

### Exportation/Importation de données

```bash
# Exporter les données sous forme de fixtures JSON
docker compose exec webslinger bash -lc "cd backend && python manage.py dumpdata <app> --database=<alias> > fixtures/<app>.json"

# Importer des fixtures
docker compose exec webslinger bash -lc "cd backend && python manage.py loaddata fixtures/<app>.json --database=<alias>"

# Sauvegarde/restauration native de PostgreSQL
docker compose exec db pg_dump -U postgres -d memalpha > memalpha.sql
docker compose exec db psql -U postgres -d memalpha < memalpha.sql
```

---

## Dépannage

### Erreur "Table already exists" (La table existe déjà)

La migration essaie de créer une table qui existe. Options:

```bash
# Option 1: Appliquer fictivement la migration (si le schéma correspond)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"

# Option 2: Supprimer la table et ré-exécuter la migration (DÉTRUIT LES DONNÉES)
docker compose exec db psql -U postgres -d <database> -c "DROP TABLE <table_name> CASCADE;"
make local-migrate
```

### Erreur "Column does not exist" (La colonne n'existe pas)

La migration attend une colonne qui n'existe pas:

```bash
# Vérifier le schéma actuel de la table
docker compose exec db psql -U postgres -d <database> -c "\d <table_name>"

# Option 1: Ajouter manuellement la colonne
docker compose exec db psql -U postgres -d <database> -c "ALTER TABLE <table> ADD COLUMN <col> <type>;"

# Option 2: Réinitialiser l'état de la migration et ré-exécuter
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <previous_migration> --fake --database=<alias>"
make local-migrate
```

### Conflit de clé dupliquée / Violation de contrainte

```bash
# Rechercher les données dupliquées
docker compose exec db psql -U postgres -d <database> -c "SELECT <col>, COUNT(*) FROM <table> GROUP BY <col> HAVING COUNT(*) > 1;"

# Supprimer les doublons (garder le premier)
docker compose exec db psql -U postgres -d <database> -c "DELETE FROM <table> a USING <table> b WHERE a.id > b.id AND a.<col> = b.<col>;"
```

### Conflits de dépendances de migration

Lorsque les migrations ont des dépendances circulaires ou conflictuelles:

```bash
# Afficher le graphe de dépendances des migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations --plan"

# Modifier manuellement les dépendances de migration dans le fichier de migration
# Rechercher: dependencies = [('app', 'migration'), ...]
```

### Erreur "No such table: django_migrations" (Pas de table : django_migrations)

La base de données existe mais n'a pas été initialisée:

```bash
# Exécuter migrate pour créer la table django_migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=<alias>"
```

### Resynchroniser l'état de la migration avec la base de données

Lorsque les enregistrements de migration ne correspondent pas au schéma réel:

```bash
# 1. Voir ce que Django pense être appliqué
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations <app>"

# 2. Inspecter le schéma réel de la base de données
docker compose exec db psql -U postgres -d <database> -c "\dt"  # lister les tables
docker compose exec db psql -U postgres -d <database> -c "\d <table>"  # décrire la table

# 3. Appliquer fictivement les migrations pour correspondre à la réalité
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"
```

---

## Référence rapide

| Tâche | Commande |
|---|---|
| Créer une migration | `docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations <app>"` |
| Appliquer toutes les migrations | `make local-migrate` |
| Vérifier l'état | `make local-migrate-check` |
| Afficher les migrations | `docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations"` |
| Regrouper les migrations | `docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> <start> <end>"` |
| Fausse migration | `docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"` |
| Annuler | `docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <target> --database=<alias>"` |
| Déployer sur GCP | `make gcp-deploy` |
| Déploiement local complet | `make local-deploy` |
