```markdown
<!--
examples/stm32h747i-disco/DEBUG-SETUP.md - Configuración de depuración STM32 (OpenOCD, GDB), LaunchAgent de macOS y recetas de recuperación.
-->

# Configuración de Depuración

Esta guía documenta un flujo de trabajo confiable de OpenOCD + GDB para placas STM32H7 (por ejemplo, STM32H747I-DISCO), incluyendo un LaunchAgent de macOS para mantener OpenOCD en ejecución, un flujo de reconexión automática de GDB resiliente y comandos de recuperación cuando SWD/JTAG entra en un estado defectuoso.

## LaunchAgent de macOS para OpenOCD

Cree un LaunchAgent para iniciar OpenOCD al iniciar sesión y reiniciarlo si falla.

1) Cree `~/Library/LaunchAgents/com.softoboros.openocd.plist` con el siguiente contenido.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.softoboros.openocd</string>

  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/openocd</string>
    <string>-f</string><string>interface/stlink.cfg</string>
    <string>-f</string><string>target/stm32h7x.cfg</string>
    <string>-c</string>
    <string>transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init</string>
  </array>

  <!-- restart if it dies -->
  <key>KeepAlive</key>
  <true/>

  <!-- log files -->
  <key>StandardOutPath</key>
  <string>/tmp/openocd.out.log</string>
  <key>StandardErrorPath</string>
  <string>/tmp/openocd.err.log</string>

  <!-- start at login -->
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
```

- Ajuste `/usr/local/bin/openocd` a donde Homebrew lo instaló: `$(brew --prefix open-ocd)/bin/openocd`.

2) Cargar y descargar:

```bash
launchctl load ~/Library/LaunchAgents/com.softoboros.openocd.plist
launchctl unload ~/Library/LaunchAgents/com.softoboros.openocd.plist
```

3) Estado y registros:

```bash
launchctl list | grep openocd
tail -f /tmp/openocd.out.log /tmp/openocd.err.log
```

## Configuración del Servidor GDB (Auto-Reconexión)

Dos enfoques resilientes para mantener GDB conectado a OpenOCD incluso si el cable SWD se desconecta.

### Opción A — Envoltorio de Shell que reinicia GDB

`debug-gdb.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
while :; do
  gdb -q --command=gdb_init.gdb || true
  echo "[gdb] disconnected; retrying in 1s"; sleep 1
done
```

`gdb_init.gdb` (ligero, resiliente):

```gdb
set pagination off
set confirm off
set target-async on
set remotetimeout 2

# connect; if OpenOCD is not ready yet, GDB exits and wrapper restarts it
target extended-remote :3333

# typical STM32H7 session warmup
monitor reset halt
monitor arm semihosting enable

# stay attached; if the wire drops, GDB exits → wrapper relaunches
```

### Opción B — Bucle de "reconexión" dentro de GDB (sin envoltorio)

```gdb
set pagination off
set confirm off
set target-async on
set remotetimeout 1

define _reconnect
  while 1
    echo [gdb] trying :3333...\n
    if $_target_connected
      monitor reset halt
      break
    end
    set timeout 1
    target extended-remote :3333
    sleep 1
  end
end

_reconnect

## OpenOCD de Doble Núcleo

- Use `openocd/stm32h747_dual_core.cfg` (SWD lento + conexión bajo reinicio):
  - Puertos GDB: CM7 en 3333, CM4 en 3334
  - Ejecutar: `make openocd-dual`
- Orden de lanzamiento: conecte CM7 primero, luego CM4.

## SVDs

- Coloque los SVDs del dispositivo en `.svd/` y apunte las configuraciones de lanzamiento a:
  - CM7: `.svd/STM32H747_CM7.svd`
  - CM4: `.svd/STM32H747_CM4.svd`
```

Si su GDB carece de `$_target_connected`, prefiera el envoltorio de shell.

Características deseables:

- Mantener los puertos estables: configure `gdb_port 3333`, `telnet_port 4444`, `tcl_port 6666` en su comando OpenOCD o `.cfg`.
- Retroceso: si la sonda está desconectada, agregue un `sleep` exponencial para evitar el spam en el registro.
- Verificación de estado: un pequeño `watchdog` puede `nc -z localhost 3333` y reiniciar si es necesario (principalmente redundante con LaunchAgent/systemd).
- Peculiaridades del objetivo (H7): reducir la velocidad y conectarse bajo reinicio a menudo ayuda:
  `-c "transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init"`

## Recuperación de Depuración (STM32H7)

Si OpenOCD falla durante "examinar" (por ejemplo, no puede leer `DBGMCU` en `0x5C001004`) y SWD/JTAG se vuelve inválido, generalmente se debe a problemas de tiempo de conexión, reloj SWD demasiado rápido o un objetivo protegido/de bajo consumo. Intente, en orden:

1) SWD lento + conexión bajo reinicio (solución más común):

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; flash erase_address 0x08000000 0x200000; exit"
```

2) Use el borrado masivo del controlador (banco 0; agregue el banco 1 si es de doble banco):

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x mass_erase 0; exit"
```

Para partes de doble banco, también ejecute:

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x mass_erase 1; exit"
```

3) Si la parte está bloqueada para depuración (RDP), desbloquee (esto realiza un borrado masivo):

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x unlock 0; exit"
```
```
