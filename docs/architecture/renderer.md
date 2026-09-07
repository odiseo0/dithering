# Motor gráfico inicial

Estado: decisión inicial, pendiente de mediciones en equipos reales.

El proyecto empieza con Glow y desactiva las funciones por defecto de `eframe`. Solo activa `accesskit`, `default_fonts` y `glow`. No activa persistencia, WGPU, X11 ni Wayland.

La base fija `eframe` 0.35.0 porque 0.36.1 exige Rust 1.95 y el proyecto fija Rust estable 1.94.0.

Esta base cumple dos reglas del plan: incluye un solo motor gráfico y no guarda ajustes. Antes de la primera versión se deben comparar Glow y WGPU en Windows 11 con estos datos:

- tamaño del ejecutable de versión;
- tiempo de inicio;
- memoria en reposo y con una imagen grande;
- uso por escritorio remoto;
- fallos en equipos Intel, AMD y Nvidia disponibles.

La decisión final debe registrar la versión de `eframe`, el equipo, el controlador y el método de medida.
