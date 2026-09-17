# Construcción portable

Fecha: 2026-09-16  
Destino: Windows 11 x64, MSVC

## Configuración

- `.cargo/config.toml` activa `crt-static` para `x86_64-pc-windows-msvc`.
- La versión usa el subsistema `Windows GUI`, por lo que no abre una consola.
- `build.rs` incrusta el icono provisional, el manifiesto y los datos de versión.
- El manifiesto pide ejecución normal, DPI por monitor y rutas largas.
- `eframe` se compila sin su función de persistencia. El código no usa almacenamiento de `eframe`,
  `AppData` ni el registro.
- Glow es el único motor gráfico incluido.

El icono se generó con la herramienta integrada de imágenes y se convirtió a un `.ico` con tamaños
de 16, 24, 32, 48, 64, 128 y 256 píxeles. Sigue siendo provisional, igual que el nombre.

## Paquete

Genera y verifica el paquete con:

```powershell
.\scripts\package.ps1
```

El resultado es `dist/dithering-0.1.0-windows-x64.zip`. Contiene solo:

- `dithering.exe`;
- `DISTRIBUTION-README.txt`;
- `LICENSE.txt`;
- `THIRD-PARTY-NOTICES.txt`.

El archivo `.sha256` queda junto al ZIP. `verify-release.ps1` comprueba la arquitectura, el
subsistema, los recursos, la ausencia de DLL del entorno de Visual C++, el contenido del ZIP y su
SHA-256.

## Resultado comprobado

- Ejecutable PE x64: sí.
- Consola oculta: sí, subsistema `Windows GUI`.
- Icono y manifiesto incrustados: sí.
- DLL externas del entorno de Visual C++: ninguna.
- DLL no propias de Windows: ninguna.
- ZIP y SHA-256: válidos.
- Inicio desde el ZIP extraído: comprobado en el equipo de desarrollo.

Por petición del usuario no se hizo la prueba en un Windows limpio sin Rust. Esta es la única prueba
omitida de la fase 10 y no afecta la configuración estática ni la revisión de DLL hecha sobre el
binario.
