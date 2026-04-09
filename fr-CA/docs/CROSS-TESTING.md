```markdown
<!--
docs/CROSS-TESTING.md - Cross-target test linker requirements and native test guidance.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Tests multi-cibles

L'exécution de tests pour des cibles embarquées telles que `thumbv7em-none-eabihf` nécessite un lieur compatible. Par défaut, `cargo test` invoque `arm-none-eabi-gcc`, ce qui échoue si la chaîne d'outils GCC est manquante. Pour éviter cette dépendance, installez le composant `rust-lld` et configurez Cargo pour l'utiliser :

```bash
rustup component add rust-lld
```

Placez l'extrait suivant dans `.cargo/config.toml` pour sélectionner le lieur pour cette cible :

```toml
# .cargo/config.toml
[target.thumbv7em-none-eabihf]
linker = "rust-lld"
```

Avec cette configuration, les tests multi-cibles se lient sans la chaîne d'outils GCC externe.

## Exécutions de tests natifs

La plupart des tests unitaires ne dépendent pas des cibles embarquées et peuvent s'exécuter sur l'hôte :

```bash
cargo test --workspace
```

Cela exécute les tests avec le lieur hôte et ignore l'exigence du lieur croisé. Seuls les tests d'intégration matérielle nécessitent la cible embarquée.

## Notes sur l'intégration continue (CI)

Le flux de travail CI actuel exécute les tests uniquement sur la cible hôte, mais les constructions multi-cibles devraient garantir que `rust-lld` est disponible si des tests sont ajoutés. Installez le composant pendant la configuration et réutilisez la configuration ci-dessus :

```yaml
- name: Install rust-lld
  run: rustup component add rust-lld
```

## Dépannage

- **`linker "rust-lld" not found`** – assurez-vous que le composant est installé avec `rustup component add rust-lld`.
- **Les tests invoquent toujours `arm-none-eabi-gcc`** – vérifiez que `.cargo/config.toml` contient le bloc `[target.thumbv7em-none-eabihf]`.
- **Erreurs de lieur concernant `memory.x`** – certains exemples nécessitent un script de lieur ; construisez avec le `build.rs` de la carte ou supprimez l'option `--target` pour exécuter sur l'hôte.

## Nuances spécifiques à la carte

- **STM32H747I-DISCO** – Activez la fonctionnalité `stm32h747i_disco` et laissez le `build.rs` de l'exemple configurer `memory.x`. Construisez ou testez avec :

  ```bash
  cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco" \
    --target thumbv7em-none-eabihf

  SD + FATFS smoke (no_std adapter; CI marked allow-failure due to `core_io` on newer rustc):

  cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,fatfs_nostd" \
    --target thumbv7em-none-eabihf
  ```

  Les tests uniquement hôte peuvent omettre l'option `--target` pour s'exécuter nativement.
```
