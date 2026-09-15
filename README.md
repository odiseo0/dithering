# Dithering

Aplicación portable para Windows 11 que aplicará dithering a una imagen. El nombre y el diseño final siguen pendientes.

## Estado

El repositorio contiene la base de las fases 0 a 5:

- `dither-engine`: motor sin interfaz ni acceso al sistema operativo;
- `dither-desktop`: aplicación de Windows con `eframe` y Glow;
- contratos iniciales de imagen;
- ráster, color lineal, paleta y ajustes validados;
- reducción por escala con reglas de alfa y bordes;
- Floyd–Steinberg y Atkinson con escala, alfa, cancelación y progreso;
- Bayer 2×2, 4×4 y 8×8 para paletas de 2 a 8 colores;
- semitono con círculos, cuadrados, tamaño, inversión y transparencia;
- carga por contenido de PNG, JPEG/JPG y WebP estáticos;
- límites de decodificación, orientación EXIF y rechazo de APNG y WebP animado;
- adaptador de imagen del portapapeles y guardado PNG seguro;
- datos de prueba mínimos;
- controles locales e integración continua en Windows.

El motor de efectos y los adaptadores de imagen están completos. Aún no están conectados a la
interfaz; esa conexión corresponde a las fases 6 y 7.

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
cargo bench -p dither-engine --bench ordered
```

Consulta los resultados y el coste estimado de memoria en
[`docs/performance-baseline.md`](docs/performance-baseline.md).

Consulta los contratos antes de cambiar el modelo del motor: [`docs/architecture/contracts.md`](docs/architecture/contracts.md).
