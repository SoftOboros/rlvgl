```markdown
# MIGRACIONES.md

Guía completa para la gestión de migraciones de Django en entornos de desarrollo local y producción en GCP.

## Tabla de Contenidos
- [Arquitectura de Base de Datos](#arquitectura-de-base-de-datos)
- [Flujo de Trabajo de Desarrollo](#flujo-de-trabajo-de-desarrollo)
- [Comandos Locales](#comandos-locales)
- [Implementación en GCP](#gcp-deployment)
- [Unificación de Migraciones (Squashing)](#squashing-migrations)
- [Manipulaciones de Base de Datos](#database-manipulations)
- [Resolución de Problemas](#troubleshooting)

---

## Arquitectura de Base de Datos

Este proyecto utiliza una arquitectura de base de datos dividida para la escalabilidad. Cada base de datos tiene su propio historial de migraciones.

| Alias de Base de Datos | Propósito                                    | Modelos Clave                                 |
|------------------------|----------------------------------------------|-----------------------------------------------|
| `default`              | Datos de usuario principales, referencias cruzadas entre aplicaciones | User, UserEmailAddress                        |
| `memalpha`             | Metadatos RAG                                | MADocument, MAText, MAChunk, MACorpus, MANotebook |
| `memalpha_vectors`     | Embeddings de pgvector                       | MAEmbedding                                   |
| `media`                | Metadatos de almacenamiento de archivos      | UserPublicMedia, UserPrivateMedia             |
| `mailman`              | Integración de correo electrónico            | UserEmailMessage, UserEmailAttachment         |
| `pdf_ingest`           | Procesamiento de PDF                         | PdfDocument, IngestEventLog, TranslationSpec  |

**Enrutadores de Base de Datos:** Ubicados en el archivo `router.py` de cada aplicación. Estos enrutan automáticamente los modelos a la base de datos correcta.

**Claves Foráneas entre Bases de Datos:** Utilice `db_constraint=False` ya que PostgreSQL no puede aplicar restricciones de FK entre bases de datos.

---

## Flujo de Trabajo de Desarrollo

### Creación de Migraciones

```bash
# Dentro del contenedor webslinger
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations <app_name>"

# Ejemplo: crear migración para la aplicación memalpha
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations memalpha"

# Crear migraciones para todas las aplicaciones con cambios pendientes
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations"
```

### Aplicación de Migraciones

```bash
# Aplicar todas las migraciones (usa el objetivo del Makefile)
make local-migrate

# O aplicar manualmente a una base de datos específica
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=memalpha"
```

### Verificación del Estado de las Migraciones

```bash
# Verificar migraciones no aplicadas y cambios de modelo pendientes
make local-migrate-check

# Mostrar todas las migraciones y su estado
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations"

# Mostrar migraciones para una aplicación específica
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations memalpha"

# Verificar si los modelos tienen cambios no migrados
docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations --check --dry-run"
```

### Mejores Prácticas

1. **Un cambio lógico por migración:** No mezcle cambios de modelo no relacionados.
2. **Confirmar migraciones con código:** Los archivos de migración deben estar en el mismo commit que los cambios del modelo.
3. **Nunca edite migraciones aplicadas:** Cree nuevas migraciones para corregir problemas.
4. **Pruebe las migraciones localmente:** Siempre ejecute `make local-migrate` antes de enviar.
5. **Nombre las migraciones descriptivamente:** Use el indicador `--name`: `makemigrations memalpha --name add_corpus_description`

---

## Comandos Locales

### Despliegue Local Completo (refleja GCP)

```bash
# Despliegue estándar: descarga imágenes, inicia servicios, asegura DBs, migra
make local-deploy

# Fuerza la reconstrucción de imágenes y luego despliega
make local-deploy-force
```

### Comandos Específicos de Migración

```bash
# Ejecutar todas las migraciones en todas las bases de datos
make local-migrate

# Verificar el estado de la migración
make local-migrate-check

# Generar SQL sin ejecutar (vista previa)
docker compose exec webslinger bash -lc "cd backend && python manage.py sqlmigrate <app> <migration_name>"

# Ejemplo: ver SQL para memalpha 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py sqlmigrate memalpha 0014"
```

---

## Implementación en GCP

### Cómo maneja GCP Deploy las Migraciones

El comando `make gcp-deploy` (a través de `scripts/gcp/deploy_compose.sh`) automáticamente:

1. Asegura que todas las bases de datos divididas existen.
2. Ejecuta las migraciones en la base de datos `default`.
3. Ejecuta las migraciones en cada base de datos dividida: `memalpha`, `memalpha_vectors`, `media`, `mailman`, `pdf_ingest`.

### Lista de Verificación Pre-despliegue

Antes de implementar en GCP:

```bash
# 1. Asegúrese de que todas las migraciones estén confirmadas
git status

# 2. Verifique que las migraciones se apliquen limpiamente localmente
make local-migrate

# 3. Verifique que no haya cambios de modelo pendientes
make local-migrate-check

# 4. Pruebe que la característica/solución funciona localmente
# ... ejecute sus pruebas ...

# 5. Envíe a la rama objetivo
git push origin <branch>
```

### Implementación en GCP

```bash
# Desplegar la rama principal (por defecto)
make gcp-deploy

# Desplegar una rama específica
GIT_REF=origin/feature-branch make gcp-deploy

# Desplegar con una versión secreta específica
SECRET_NAME=SoftOboros SECRET_VERSION=5 make gcp-deploy
```

### Monitoreo del Despliegue

```bash
# SSH a la VM de GCP
make gcp-ssh

# Ejecutar en webslinger en GCP
make gcp-compose-exec SERVICE=webslinger

# Verificar el estado de la migración en GCP
make gcp-compose-exec SERVICE=webslinger CMD="cd backend && python manage.py showmigrations"
```

---

## Unificación de Migraciones (Squashing)

### Cuándo Unificar

- **Antes de la promoción a producción:** Unifique las migraciones de la rama de prueba antes de fusionar a la rama principal.
- **Migraciones acumuladas:** Cuando una aplicación tiene 3 o más migraciones de desarrollo iterativo.
- **Limpieza:** Cuando las migraciones hacen referencia a modelos/campos eliminados.
- **Rendimiento:** Las cadenas de migración grandes ralentizan los despliegues nuevos.

### Procedimiento de Unificación

#### Paso 1: Verificar Estado Limpio

```bash
# Todas las migraciones deben aplicarse
make local-migrate-check

# No debe mostrar migraciones no aplicadas
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations <app>"
```

#### Paso 2: Copia de Seguridad (si los datos importan)

```bash
# Copia de seguridad de una base de datos específica
docker compose exec db pg_dump -U postgres -d memalpha > memalpha_backup.sql

# Copia de seguridad de todas las bases de datos
docker compose exec db pg_dump -U postgres -d softoboros > default_backup.sql
docker compose exec db pg_dump -U postgres -d memalpha > memalpha_backup.sql
# ... repetir para otras bases de datos
```

#### Paso 3: Unificar Migraciones

```bash
# Unificar un rango de migraciones
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> <start_migration> <end_migration>"

# Ejemplo: unificar migraciones de memalpha de 0010 a 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations memalpha 0010 0014"

# Unificar todas las migraciones para una aplicación (usar con precaución)
docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> 0001"
```

#### Paso 4: Revisar la Migración Unificada

El comando crea un archivo como `0010_squashed_0014_<name>.py`. Revíselo:

- Verifique que la lista `replaces = [...]` contenga todas las migraciones unificadas.
- Verifique que la lista `operations` sea correcta.
- Busque cualquier operación `RunPython` que pueda necesitar ajuste.

#### Paso 5: Probar en una Base de Datos Nueva

```bash
# Eliminar y recrear la base de datos específica
docker compose exec db dropdb -U postgres memalpha
docker compose exec db createdb -U postgres -T template0 memalpha

# Ejecutar migraciones: la migración unificada debe aplicarse limpiamente
make local-migrate

# Verificar que la aplicación funciona correctamente
```

#### Paso 6: Confirmar y Desplegar

```bash
# Confirmar la migración unificada
git add backend/<app>/migrations/
git commit -m "chore(<app>): unificar migraciones 0010-0014"
```

#### Paso 7: Eliminar Migraciones Antiguas (Post-despliegue)

**Solo después de que TODOS los entornos hayan aplicado la migración unificada:**

1. Elimine los archivos de migración individuales que fueron unificados.
2. Edite la migración unificada para eliminar la línea `replaces = [...]`.
3. Confirme la limpieza.

```bash
# Ejemplo de limpieza
rm backend/memalpha/migrations/0010_*.py
rm backend/memalpha/migrations/0011_*.py
rm backend/memalpha/migrations/0012_*.py
rm backend/memalpha/migrations/0013_*.py
rm backend/memalpha/migrations/0014_*.py

# Edite el archivo unificado para eliminar el atributo 'replaces'
# Luego confirme
git add -A && git commit -m "chore(memalpha): eliminar fuentes de migración unificadas"
```

---

## Manipulaciones de Base de Datos

### Reiniciar una Sola Base de Datos

```bash
# Eliminar y recrear (DESTRUYE TODOS LOS DATOS)
docker compose exec db dropdb -U postgres memalpha
docker compose exec db createdb -U postgres -T template0 memalpha

# Volver a ejecutar migraciones
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=memalpha"
```

### Reiniciar Todas las Bases de Datos Divididas

```bash
# Use el indicador RESET_SPLIT_DBS con local-deploy (conserva la DB predeterminada)
RESET_SPLIT_DBS=1 make local-deploy
```

### Migraciones Falsas

Use cuando el estado de la migración no está sincronizado con el esquema de la base de datos:

```bash
# Marcar la migración como aplicada sin ejecutarla
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"

# Ejemplo: aplicar de forma falsa memalpha 0014
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate memalpha 0014 --fake --database=memalpha"

# Aplicar de forma falsa todas las migraciones para una aplicación (peligroso, usar solo cuando el esquema de la DB coincide)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> --fake --database=<alias>"
```

### Revertir Migraciones

```bash
# Revertir a una migración específica (PERDERÁ DATOS de las migraciones revertidas)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <target_migration> --database=<alias>"

# Ejemplo: revertir memalpha a 0012
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate memalpha 0012 --database=memalpha"

# Revertir todas las migraciones para una aplicación (reiniciar a cero)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> zero --database=<alias>"
```

### Acceso a la Consola de la Base de Datos

```bash
# Consola de PostgreSQL (local)
docker compose exec db psql -U postgres -d softoboros

# Consola de PostgreSQL para una base de datos específica
docker compose exec db psql -U postgres -d memalpha

# Consola de base de datos de Django
docker compose exec webslinger bash -lc "cd backend && python manage.py dbshell --database=memalpha"
```

### Exportación/Importación de Datos

```bash
# Exportar datos como fixtures JSON
docker compose exec webslinger bash -lc "cd backend && python manage.py dumpdata <app> --database=<alias> > fixtures/<app>.json"

# Importar fixtures
docker compose exec webslinger bash -lc "cd backend && python manage.py loaddata fixtures/<app>.json --database=<alias>"

# Volcado/restauración nativo de PostgreSQL
docker compose exec db pg_dump -U postgres -d memalpha > memalpha.sql
docker compose exec db psql -U postgres -d memalpha < memalpha.sql
```

---

## Resolución de Problemas

### Error "La tabla ya existe"

La migración está intentando crear una tabla que ya existe. Opciones:

```bash
# Opción 1: Aplicar de forma falsa la migración (si el esquema coincide)
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"

# Opción 2: Eliminar la tabla y volver a ejecutar la migración (DESTRUYE DATOS)
docker compose exec db psql -U postgres -d <database> -c "DROP TABLE <table_name> CASCADE;"
make local-migrate
```

### Error "La columna no existe"

La migración espera una columna que no existe:

```bash
# Verificar el esquema de la tabla actual
docker compose exec db psql -U postgres -d <database> -c "\d <table_name>"

# Opción 1: Añadir manualmente la columna
docker compose exec db psql -U postgres -d <database> -c "ALTER TABLE <table> ADD COLUMN <col> <type>;"

# Opción 2: Reiniciar el estado de la migración y volver a ejecutar
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <previous_migration> --fake --database=<alias>"
make local-migrate
```

### Clave Duplicada / Violación de Restricción

```bash
# Buscar datos duplicados
docker compose exec db psql -U postgres -d <database> -c "SELECT <col>, COUNT(*) FROM <table> GROUP BY <col> HAVING COUNT(*) > 1;"

# Eliminar duplicados (mantener el primero)
docker compose exec db psql -U postgres -d <database> -c "DELETE FROM <table> a USING <table> b WHERE a.id > b.id AND a.<col> = b.<col>;"
```

### Conflictos de Dependencia de Migración

Cuando las migraciones tienen dependencias circulares o conflictivas:

```bash
# Mostrar el gráfico de dependencia de migración
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations --plan"

# Editar manualmente las dependencias de migración en el archivo de migración
# Buscar: dependencies = [('app', 'migration'), ...]
```

### "No existe la tabla: django_migrations"

La base de datos existe pero no ha sido inicializada:

```bash
# Ejecutar migrate para crear la tabla django_migrations
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate --database=<alias>"
```

### Resincronizar el Estado de la Migración con la Base de Datos

Cuando los registros de migración no coinciden con el esquema real:

```bash
# 1. Ver lo que Django cree que está aplicado
docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations <app>"

# 2. Inspeccionar el esquema real de la base de datos
docker compose exec db psql -U postgres -d <database> -c "\dt"  # listar tablas
docker compose exec db psql -U postgres -d <database> -c "\d <table>"  # describir tabla

# 3. Aplicar migraciones falsas para que coincidan con la realidad
docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"
```

---

## Referencia Rápida

| Tarea | Comando |
|---|---|
| Crear migración | `docker compose exec webslinger bash -lc "cd backend && python manage.py makemigrations <app>"` |
| Aplicar todas las migraciones | `make local-migrate` |
| Verificar estado | `make local-migrate-check` |
| Mostrar migraciones | `docker compose exec webslinger bash -lc "cd backend && python manage.py showmigrations"` |
| Unificar migraciones | `docker compose exec webslinger bash -lc "cd backend && python manage.py squashmigrations <app> <start> <end>"` |
| Migración falsa | `docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <migration> --fake --database=<alias>"` |
| Revertir | `docker compose exec webslinger bash -lc "cd backend && python manage.py migrate <app> <target> --database=<alias>"` |
| Desplegar en GCP | `make gcp-deploy` |
| Despliegue local completo | `make local-deploy` |
```
