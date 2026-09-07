# AGENTS.md

## Propósito

- Aplicación portable para Windows 11 que aplica dithering a una imagen.
- Código en Rust con `egui`, `eframe` y una interfaz sencilla, oscura y en español.
- El nombre, el icono y el diseño final siguen pendientes.

## Alcance de la primera versión

- Abrir una imagen PNG, JPEG, JPG o WebP estática mediante selector, arrastre o portapapeles.
- Guardar el resultado como PNG mediante **Guardar como**.
- Conservar el tamaño y el canal alfa de cada píxel.
- Mostrar vista previa automática, original y resultado, con zoom, 100 %, ajuste y desplazamiento.
- Procesar fuera del hilo de la interfaz, con progreso y cancelación.

## Efectos

- Incluir Floyd–Steinberg, Atkinson y Bayer 2×2, 4×4 y 8×8.
- Difusión y Bayer usan paletas de 2 a 8 colores.
- Semitono usa dos colores, círculo o cuadrado, tamaño e inversión.
- Valores iniciales: Floyd–Steinberg, escala 1 y negro/blanco.

## Fuera de alcance

- No añadir CLI, lotes, instalador, idiomas, brillo, contraste, ajustes guardados ni historial.
- No aceptar animaciones ni otros formatos sin cambiar el alcance.

## Arquitectura

- Usar arquitectura hexagonal y empaquetado por componentes.
- Separar `dither-engine` y `dither-desktop` en el espacio de trabajo.
- El motor no conoce GUI, ventanas, archivos ni portapapeles.
- Los puertos viven junto a su caso de uso y los adaptadores los implementan en escritorio.
- Evitar `utils` y mantener el motor reutilizable por una futura CLI.

## Reglas de imagen

- Trabajar en RGBA de 8 bits, tratar RGB como sRGB y calcular en RGB lineal.
- Copiar el alfa de entrada sin cambios.
- Un píxel con alfa cero no recibe ni transmite error.
- Detectar el formato por contenido y aplicar la orientación EXIF antes de procesar.
- Rechazar tamaños inválidos y limitar memoria antes de reservarla.
- Producir resultados deterministas y solo con colores de la paleta activa.

## Trabajo en segundo plano

- Usar un trabajador fijo y conservar solo el pedido más reciente.
- Identificar cada pedido con documento y revisión, y comprobar cancelación por fila lógica.
- Ignorar todo progreso o resultado que ya no esté vigente.
- No permitir guardar mientras el resultado actual esté obsoleto.

## Orden de trabajo

1. Fijar contratos y crear el modelo del motor.
2. Implementar algoritmos con pruebas y mediciones.
3. Crear códecs, límites, coordinación y una interfaz mínima de extremo a extremo.
4. Pulir la vista, probar Windows y generar el `.exe`.

## Calidad

- Añadir pruebas unitarias, de propiedades, de referencia e integración.
- Probar tamaños impares, bordes, alfa, archivos dañados y cancelación.
- Ejecutar `cargo fmt`, `cargo clippy` sin avisos y `cargo test`.
- No usar `unwrap` con datos del usuario ni cambiar el alcance sin dejar constancia.
