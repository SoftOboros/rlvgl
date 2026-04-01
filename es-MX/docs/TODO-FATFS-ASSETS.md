<!--
docs/TODO-FATFS-ASSETS.md - TODO – Carga de Activos respaldada por FATFS para rlvgl (característica central opcional).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# TODO – Carga de Activos respaldada por FATFS para rlvgl (característica central opcional)

> **Épica:** Agregar carga opcional de activos basada en sistema de archivos a rlvgl usando una implementación FAT portable. El núcleo expone una API `AssetSource` pequeña y estable; los crates de plataforma proporcionan controladores de dispositivo de bloque (tarjeta SD en H747I‑DISCO) o un stub de simulador. Cuando está deshabilitado, el núcleo aún soporta activos integrados.

---

## Metas y No Metas

- **Metas**
  - Característica `` core opcional que habilita activos respaldados por FATFS.
  - Conexión de plataforma a través de un trait `BlockDevice` implementado por cada objetivo (SD en DISCO; imagen respaldada por archivo en simulador).
  - Cero `std` en el núcleo; `std` solo en el backend del simulador.
  - Solo lectura v0 (montar, listar, abrir, leer). Escritura/vaciar son futuras.
  - Manejo seguro de DMA y D‑Cache en H7 para SDMMC.
- **No Metas (v0)**
  - Sin registro de transacciones ni sistemas de archivos exóticos.
  - Sin herramientas de particionamiento dinámico.

---

## Características y Diseño del Crate

| ✓   | Descripción                                     | Dependencias           | Notas                               |
| --- | ----------------------------------------------- | ---------------------- | ----------------------------------- |
| [x] | Agregar característica `fs` a `rlvgl/core`        | `alloc`                | Todo el código FS detrás de la bandera de característica |
| [x] | Traits FS (`BlockDevice`, `FsError`) en el núcleo | —                     | Movido de crate independiente      |
| [x] | Nuevo crate: `rlvgl-fs-sim` (std)                 | `fatfs`, `std`         | Simulador: dispositivo de bloque respaldado por archivo |
| [x] | Módulo de plataforma: `platform/stm32h747i_disco_sd` | HAL + DMA              | SDMMC + DMA + mantenimiento de caché     |

> **Elección de implementación FAT:** Preferir el crate Rust `fatfs` en modo `no_std` para una API consistente en todos los objetivos. `embedded-sdmmc` es una alternativa; mantener la abstracción delgada para que cualquiera pueda encajar más tarde.

---

## API Pública (Orientada al Núcleo)

**En **``

```rust
/// 512-byte logical sectors recommended; expose actual size via `block_size()`.
pub trait BlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError>;
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError>; // v1: may be stubbed for RO
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn flush(&mut self) -> Result<(), FsError>;
}

/// Filesystem handle (FAT volume) constructed over a BlockDevice.
pub struct FatVolume<'a, B: BlockDevice> { /* ... */ }

pub trait AssetSource {
    /// Open an asset by logical path, e.g., "fonts/regular.bin".
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, FsError>;
    fn exists(&self, path: &str) -> bool;
    fn list(&self, dir: &str) -> Result<AssetIter, FsError>;
}

pub trait AssetRead {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, FsError>;
    fn len(&self) -> usize;
    fn seek(&mut self, pos: u64) -> Result<u64, FsError>;
}
```

**En **``** (detrás de **``**)**

```rust
pub struct AssetManager<S: AssetSource> { /* ... */ }
impl<S: AssetSource> AssetManager<S> {
    pub fn load_font(&self, path: &str) -> Result<Font, AssetError>;
    pub fn load_image(&self, path: &str) -> Result<Image, AssetError>;
    // generic helper
    pub fn open(&self, path: &str) -> Result<Box<dyn AssetRead + '_>, AssetError>;
}
```

---

## Simulador (std) – Imagen de Disco de Archivo Único

| ✓   | Descripción                   | Dependencias        | Notas                                               |
| --- | ----------------------------- | ------------------- | --------------------------------------------------- |
| [x] | Implementar `SimBlockDevice`    | `std::fs::File`     | Un único archivo de **imagen de disco**, pre-dimensionado (ej., 32MB) |
| [x] | Mapeo de memoria opcional para velocidad | `memmap2` (feature) | Retorno a pread/pwrite si no está disponible             |
| [x] | Herramienta: crear/poblar imagen   | Rust CLI            | `mkfatimg --size 32M --from ./assets/`              |
| [ ] | Montar y prueba de humo            | rlvgl sim           | Leer un PNG/fuente, renderizar una etiqueta                     |

**Justificación:** Mantener la lógica FAT intacta permitiendo que FATFS gestione el diseño en disco. El simulador solo proporciona lecturas/escrituras de sector en un único archivo anfitrión.

---

## Controlador de Tarjeta SD STM32H747I‑DISCO (SDMMC + DMA)
