# Dithering

Aplicación portable para Windows 11 que aplicará dithering a una imagen. El nombre y el diseño final siguen pendientes.

## Estado

El repositorio contiene la base de la fase 0:

- `dither-engine`: motor sin interfaz ni acceso al sistema operativo;
- `dither-desktop`: aplicación de Windows con `eframe` y Glow;
- contratos iniciales de imagen;
- datos de prueba mínimos;
- controles locales e integración continua en Windows.

Los algoritmos y las funciones para abrir o guardar imágenes aún no están implementados.

## Uso durante el desarrollo

Inicia la base de la aplicación:

```powershell
cargo run -p dither-desktop
```

Ejecuta formato, análisis, pruebas y compilación de versión:

```powershell
.\scripts\check.ps1
```

Consulta los contratos antes de cambiar el modelo del motor: [`docs/architecture/contracts.md`](docs/architecture/contracts.md).
