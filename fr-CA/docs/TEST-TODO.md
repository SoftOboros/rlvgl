```markdown
<!--
docs/TEST-TODO.md - rlvgl – Test TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – TÂCHES À FAIRE pour les tests

Ce fichier énumère le **flux de travail de test** pour rlvgl. Chaque entrée est classée approximativement dans l'ordre où elle devrait être abordée, liste ses **dépendances** en amont – soit par référence aux sections de `docs/TODO.md` (`TODO#N`) ou à des tests antérieurs – et indique si elle peut être **entièrement automatisée** (via `cargo test` piloté par Codex, simulateur sans tête, image-diff CI, etc.) ou nécessite une **vérification humaine** (par exemple, acceptation visuelle sur du matériel réel).

| ✔ | Ordre | ID du test | Description | Dépend de | Automatisation |
|---|-------|------------|-------------|-----------|---------------|
| [x] | 1 | T-01 | **Tests unitaires fondamentaux** – invariants du trait Widget, mutations de l'arbre, abandon sans panique | TODO#1 | Automatisé (Codex + `cargo test`) |
| [x] | 2 | T-02 | **Tests de distribution d'événements** – ordre de capture/bulle, arrêt de la propagation | T-01 | Automatisé |
| [x] | 3 | T-03 | **Tests de constructeur de style** – le modèle de constructeur produit les structures attendues et les valeurs par défaut | T-01 | Automatisé |
| [x] | 4 | T-04 | **Test de fumée du Dummy DisplayDriver et du Renderer** – rend une image de couleur unie dans un tampon RAM | TODO#3 | Automatisé (sans tête) |
| [x] | 5 | T-05 | **Tests de stub InputDevice** – marshaling des événements clavier/souris | TODO#3 | Automatisé |
| [ ] | 6 | T-06 | **Intégration SPI `st7789` test de fumée** sur carte STM32H7 NUCLEO | T-04, matériel | **Humain** (visuel et portée) |
| [x] | 7 | T-07 | **Rendu "golden" du widget de niveau 1** – Label, Button, Container PNG diff vs goldens | TODO#4, T-04 | Automatisé (sim sans tête) |
| [x] | 8 | T-08 | **Test de stress de la mise en page** – fuzzer les tailles de conteneurs et affirmer aucune panique / limites incorrectes | T-07 | Automatisé |
| [x] | 9 | T-09 | **Test de fenêtre du backend du simulateur** – ouvrir une fenêtre SDL/pixels et rendre l'image | TODO#5 | Automatisé (CI sans tête-X) |
| [x] | 10 | T-10 | **Widgets "goldens" de niveau 2** – Checkbox, Slider, Arc, List, Image | TODO#6, T-09 | Automatisé |
| [x] | 11 | T-11 | **Test d'application de thème** – cascade de thème clair/foncé, correction | TODO#7, T-10 | Automatisé |
| [x] | 12 | T-12 | **Test de la chronologie d'animation** – fondu/glissement produisent les images clés attendues (diff de hachage dans le temps) | TODO#7, T-11 | *Automatisé* (hachage d'images) + **Humain** pour la fluidité |
| [ ] | 13 | T-13 | **Diff de démonstration de parité LVGL** – rendre la démo C et rlvgl, diff d'image perceptuel ≤ ε | TODO#9, T-10 | Automatisé (CI) + **Humain** sur diff > ε |
| [x] | 14 | T-14 | **Régression de fuzzer d'événements** – tapotements/glissements aléatoires contre les widgets pour 1k itérations avec MIRI | T-07 | Automatisé |
| [x] | 15 | T-15 | **Régression de taille embarquée** – `arm-none-eabi-size` + vérification de la carte de lien en CI | TODO#2 | Automatisé |
| [x] | 16 | T-16 | **Détection de mémoire/fuite** avec valgrind/asan sous simulateur | T-09 | Automatisé |
| [ ] | 17 | T-17 | **Benchmark de performance** – FPS @ 240x320 sur bureau et carte H7 | T-09, T-06 | **Assisté par l'humain** (chronométrage matériel) |
| [x] | 18 | T-18 | **Test de compilation des extraits de code de la documentation** – `doctest` tous les README/Exemples | TODO#8 | Automatisé |
| [x] | 19 | T-19 | **Énumération des cartes de fournisseurs** – consolider les crates de fournisseurs dans une liste unifiée | TODO-SUPPORT-PUCE | Automatisé |
| [x] | 20 | T-20 | **Gestion des erreurs de recherche de carte** – correspondance exacte du nom et erreurs utiles | T-19 | Automatisé |
| [x] | 21 | T-21 | **Liste déroulante de carte UI** – la liste de sélection se remplit à partir des crates de fournisseurs | T-19 | Automatisé |
| [x] | 22 | T-22 | **Câblage de l'environnement de la base de données de puces** – la construction intègre les définitions de carte de `RLVGL_CHIP_SRC` | TODO-SUPPORT-PUCE | Automatisé |
| [x] | 23 | T-23 | **Publication des crates de puces via script** – le script de publication liste les crates de chipdb | T-22 | Automatisé |
| [x] | 24 | T-24 | **Tests d'ingestion AFDB MCU/IP** – échantillon de STM32 XML aller-retour via des superpositions canoniques | TODO-SUPPORT-PUCE | Automatisé |
| [x] | 25 | T-25 | **Test de fumée du constructeur de catalogue AFDB** – vérifier les mappages de broches et les IOModes GPIO dans le catalogue généré | T-24 | Automatisé |
| [x] | 26 | T-26 | **Rendu de rapport AFDB** – le tableau markdown liste les broches attendues et les modes GPIO | T-25 | Automatisé |

---

### Légende
- **Colonne ✔** – marquer `[x]` une fois le test et ses critères de réussite satisfaits.
- **Automatisé** – peut être exécuté en CI à l'aide de tests Rust pilotés par Codex, d'un simulateur sans tête ou d'outils de diff perceptuel.
- **Humain** – nécessite un examen visuel ou des mesures physiques ; essayer de limiter la portée à l'approbation uniquement là où c'est inévitable.
- **Assisté par l'humain** – métriques collectées automatiquement mais nécessitant toujours une interprétation manuelle ou une configuration matérielle.

> Au fur et à mesure que de nouveaux éléments TODO sont ajoutés, ajoutez les tests correspondants ici, connectez-les à la chaîne de dépendances et laissez la case à cocher vide jusqu'à ce que le test soit entièrement vert en CI (ou vérifié par l'humain le cas échéant).
```
```
