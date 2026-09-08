# Dithering

Aplicación portable para Windows 11 que aplicará dithering a una imagen. El nombre y el diseño final siguen pendientes.

## Estado

El repositorio contiene la base de las fases 0, 1 y 2:

- `dither-engine`: motor sin interfaz ni acceso al sistema operativo;
- `dither-desktop`: aplicación de Windows con `eframe` y Glow;
- contratos iniciales de imagen;
- ráster, color lineal, paleta y ajustes validados;
- reducción por escala con reglas de alfa y bordes;
- Floyd–Steinberg y Atkinson con escala, alfa, cancelación y progreso;
- datos de prueba mínimos;
- controles locales e integración continua en Windows.

Bayer, semitono y las funciones para abrir o guardar imágenes aún no están implementados.

## Uso durante el desarrollo

Inicia la base de la aplicación:

```powershell
cargo run -p dither-desktop
```

Ejecuta formato, análisis, pruebas y compilación de versión:

```powershell
.\scripts\check.ps1
```

Ejecuta la medición base de difusión de error:

```powershell
cargo bench -p dither-engine --bench error_diffusion
```

Consulta los resultados y el coste estimado de memoria en
[`docs/performance-baseline.md`](docs/performance-baseline.md).

Consulta los contratos antes de cambiar el modelo del motor: [`docs/architecture/contracts.md`](docs/architecture/contracts.md).
