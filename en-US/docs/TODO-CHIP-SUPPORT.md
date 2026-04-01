# MIGRATIONS.md

Comprehensive guide for Django migration management across local development and GCP production environments.

## Table of Contents
- [Database Architecture](#database-architecture)
- [Development Workflow](#development-workflow)
- [Local Commands](#local-commands)
- [GCP Deployment](#gcp-deployment)
- [Squashing Migrations](#squashing-migrations)
- [Database Manipulations](#database-manipulations)
- [Troubleshooting](#troubleshooting)

---

## Database Architecture

This project uses a split-database architecture for scalability. Each database has its own migration history.

| Database Alias     | Purpose                                      | Key Models                                    |
|--------------------|----------------------------------------------|-----------------------------------------------|
| `default`          | Core user data, cross-app references         | User, UserEmailAddress                        |
| `memalpha`         | RAG metadata                                 | MADocument, MAText, MAChunk, MACorpus, MANotebook |
| `memalpha_vectors` | pgvector embeddings                          | MAEmbedding                                   |
| `media`            | File storage metadata                        | UserPublicMedia, UserPrivateMedia             |
| `mailman`          | Email integration                            | UserEmailMessage, UserEmailAttachment         |
| `pdf_ingest`       | PDF processing                               | PdfDocument, IngestEventLog, TranslationSpec  |

**Database Routers:** Located in each app's `router.py` file. These automatically route models to the correct database.

**Cross-Database Foreign Keys:** Use `db_constraint=False` since PostgreSQL cannot enforce FK constraints across databases.

---

## Development Workflow

### Creating Migrations

```bash
# Inside the webslinger container
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations <app_name>"

# Example: create migration for memalpha app
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations memalpha"

# Create migrations for all apps with pending changes
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations"
```

### Applying Migrations

```bash
# Apply all migrations (uses Makefile target)
make local-migrate

# Or manually apply to specific database
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=memalpha"
```

### Checking Migration Status

```bash
# Check for unapplied migrations and pending model changes
make local-migrate-check

# Show all migrations and their status
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations"

# Show migrations for specific app
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations memalpha"

# Check if models have unmigrated changes
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations --check --dry-run"
```

### Best Practices

1. **One logical change per migration:** Don't mix unrelated model changes
2. **Commit migrations with code:** Migration files should be in the same commit as model changes
3. **Never edit applied migrations:** Create new migrations to fix issues
4. **Test migrations locally:** Always run `make local-migrate` before pushing
5. **Name migrations descriptively:** Use `--name` flag: `makemigrations memalpha --name add_corpus_description`

---

## Local Commands

### Full Local Deploy (mirrors GCP)

```bash
# Standard deploy: pull images, start services, ensure DBs, migrate
make local-deploy

# Force rebuild images then deploy
make local-deploy-force
```

### Migration-Specific Commands

```bash
# Run all migrations on all databases
make local-migrate

# Check migration status
make local-migrate-check

# Generate SQL without executing (preview)
docker compose exec webslinger bash -lc "cd backend && python manage.py sqlmigrate <app> <migration_name>"

# Example: see SQL for memalpha 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py sqlmigrate memalpha 0014"
```

---

## GCP Deployment

### How GCP Deploy Handles Migrations

The `make gcp-deploy` command (via `scripts/gcp/deploy_compose.sh`) automatically:

1. Ensures all split databases exist
2. Runs migrations on `default` database
3. Runs migrations on each split database: `memalpha`, `memalpha_vectors`, `media`, `mailman`, `pdf_ingest`

### Pre-Deployment Checklist

Before deploying to GCP:

```bash
# 1. Ensure all migrations are committed
git status

# 2. Verify migrations apply cleanly locally
make local-migrate

# 3. Check no pending model changes
make local-migrate-check

# 4. Test the feature/fix works locally
# ... run your tests ...

# 5. Push to the target branch
git push origin <branch>
```

### Deploying to GCP

```bash
# Deploy main branch (default)
make gcp-deploy

# Deploy specific branch
GIT_REF=origin/feature-branch make gcp-deploy

# Deploy with specific secret version
SECRET_NAME=SoftOboros SECRET_VERSION=5 make gcp-deploy
```

### Monitoring Deployment

```bash
# SSH to GCP VM
make gcp-ssh

# Exec into webslinger on GCP
make gcp-compose-exec SERVICE=webslinger

# Check migration status on GCP
make gcp-compose-exec SERVICE=webslinger CMD="cd backend && python manage.py showmigrations"
```

---

## Squashing Migrations

### When to Squash

- **Before production promotion:** Squash testing branch migrations before merging to main
- **Accumulated migrations:** When an app has 3+ migrations from iterative development
- **Cleanup:** When migrations reference deleted models/fields
- **Performance:** Large migration chains slow down fresh deployments

### Squashing Procedure

#### Step 1: Verify Clean State

```bash
# All migrations must be applied
make local-migrate-check

# Should show no unapplied migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations <app>"
```

#### Step 2: Backup (if data matters)

```bash
# Backup specific database
docker compose exec db pg_dump -U postgres -d memalpha > memalpha_backup.sql

# Backup all databases
docker compose exec db pg_dump -U postgres -d softoboros > default_backup.sql
docker compose exec db pg_dump -U postgres -d memalpha > memalpha_backup.sql
# ... repeat for other databases
```

#### Step 3: Squash Migrations

```bash
# Squash a range of migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> <start_migration> <end_migration>"

# Example: squash memalpha migrations 0010 through 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations memalpha 0010 0014"

# Squash all migrations for an app (use with caution)
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> 0001"
```

#### Step 4: Review the Squashed Migration

The command creates a file like `0010_squashed_0014_<name>.py`. Review it:

- Check the `replaces = [...]` list contains all squashed migrations
- Verify `operations` list is correct
- Look for any `RunPython` operations that may need adjustment

#### Step 5: Test on Fresh Database

```bash
# Drop and recreate the specific database
docker compose exec db dropdb -U postgres memalpha
docker compose exec db createdb -U postgres -T template0 memalpha

# Run migrations - should apply squashed migration cleanly
make local-migrate

# Verify app works correctly
```

#### Step 6: Commit and Deploy

```bash
# Commit the squashed migration
git add backend/<app>/migrations/
git commit -m "chore(<app>): squash migrations 0010-0014"
```

#### Step 7: Remove Old Migrations (Post-Deploy)

**Only after ALL environments have applied the squashed migration:**

1. Delete the individual migration files that were squashed
2. Edit the squashed migration to remove the `replaces = [...]` line
3. Commit the cleanup

```bash
# Example cleanup
rm backend/memalpha/migrations/0010_*.py
rm backend/memalpha/migrations/0011_*.py
rm backend/memalpha/migrations/0012_*.py
rm backend/memalpha/migrations/0013_*.py
rm backend/memalpha/migrations/0014_*.py

# Edit the squashed file to remove 'replaces' attribute
# Then commit
git add -A && git commit -m "chore(memalpha): remove squashed migration sources"
```

---

## Database Manipulations

### Resetting a Single Database

```bash
# Drop and recreate (DESTROYS ALL DATA)
docker compose exec db dropdb -U postgres memalpha
docker compose exec db createdb -U postgres -T template0 memalpha

# Re-run migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=memalpha"
```

### Resetting All Split Databases

```bash
# Use RESET_SPLIT_DBS flag with local-deploy (preserves default DB)
RESET_SPLIT_DBS=1 make local-deploy
```

### Fake Migrations

Use when migration state is out of sync with database schema:

```bash
# Mark migration as applied without running it
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"

# Example: fake-apply memalpha 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate memalpha 0014 --fake --database=memalpha"

# Fake all migrations for an app (dangerous - use only when DB schema matches)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> --fake --database=<alias>"
```

### Rolling Back Migrations

```bash
# Rollback to specific migration (WILL LOSE DATA from rolled-back migrations)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <target_migration> --database=<alias>"

# Example: rollback memalpha to 0012
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate memalpha 0012 --database=memalpha"

# Rollback all migrations for an app (reset to zero)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> zero --database=<alias>"
```

### Database Shell Access

```bash
# PostgreSQL shell (local)
docker compose exec db psql -U postgres -d softoboros

# PostgreSQL shell for specific database
docker compose exec db psql -U postgres -d memalpha

# Django database shell
docker compose exec webslinger bash -lc "cd backend && python manage.py dbshell --database=memalpha"
```

### Data Export/Import

```bash
# Export data as JSON fixtures
docker compose exec webslinger bash -lc "cd backend && python manage.py dumpdata <app> --database=<alias> > fixtures/<app>.json"

# Import fixtures
docker compose exec webslinger bash -lc "cd backend && python manage.py loaddata fixtures/<app>.json --database=<alias>"

# PostgreSQL native dump/restore
docker compose exec db pg_dump -U postgres -d memalpha > memalpha.sql
docker compose exec db psql -U postgres -d memalpha < memalpha.sql
```

---

## Troubleshooting

### "Table already exists" Error

The migration is trying to create a table that exists. Options:

```bash
# Option 1: Fake the migration (if schema matches)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"

# Option 2: Drop the table and re-run migration (DESTROYS DATA)
docker compose exec db psql -U postgres -d <database> -c "DROP TABLE <table_name> CASCADE;"
make local-migrate
```

### "Column does not exist" Error

Migration expects a column that doesn't exist:

```bash
# Check current table schema
docker compose exec db psql -U postgres -d <database> -c "\d <table_name>"

# Option 1: Manually add the column
docker compose exec db psql -U postgres -d <database> -c "ALTER TABLE <table> ADD COLUMN <col> <type>;"

# Option 2: Reset migration state and re-run
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <previous_migration> --fake --database=<alias>"
make local-migrate
```

### Duplicate Key / Constraint Violation

```bash
# Check for duplicate data
docker compose exec db psql -U postgres -d <database> -c "SELECT <col>, COUNT(*) FROM <table> GROUP BY <col> HAVING COUNT(*) > 1;"

# Remove duplicates (keep first)
docker compose exec db psql -U postgres -d <database> -c "DELETE FROM <table> a USING <table> b WHERE a.id > b.id AND a.<col> = b.<col>;"
```

### Migration Dependency Conflicts

When migrations have circular or conflicting dependencies:

```bash
# Show migration dependency graph
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations --plan"

# Manually edit migration dependencies in the migration file
# Look for: dependencies = [('app', 'migration'), ...]
```

### "No such table: django_migrations"

The database exists but hasn't been initialized:

```bash
# Run migrate to create django_migrations table
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=<alias>"
```

### Resync Migration State with Database

When migration records don't match actual schema:

```bash
# 1. See what Django thinks is applied
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations <app>"

# 2. Inspect actual database schema
docker compose exec db psql -U postgres -d <database> -c "\dt"  # list tables
docker compose exec db psql -U postgres -d <database> -c "\d <table>"  # describe table

# 3. Fake migrations to match reality
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"
```

---

## Quick Reference

| Task | Command |
|------|---------|
| Create migration | `docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations <app>"` |
| Apply all migrations | `make local-migrate` |
| Check status | `make local-migrate-check` |
| Show migrations | `docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations"` |
| Squash migrations | `docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> <start> <end>"` |
| Fake migration | `docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"` |
| Rollback | `docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <target> --database=<alias>"` |
| Deploy to GCP | `make gcp-deploy` |
| Full local deploy | `make local-deploy` |
```
