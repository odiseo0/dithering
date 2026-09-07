# Contratos de imagen de la primera versión

Esta nota fija las decisiones que afectan al motor y a los adaptadores. Las pruebas de cada fase deben aplicar estas reglas.

## Entrada y salida

- La entrada admite PNG, JPEG, JPG y WebP estáticos.
- El adaptador detecta el formato por el contenido y rechaza animaciones.
- El adaptador aplica la orientación EXIF antes de crear el ráster.
- El motor recibe RGBA de 8 bits y trata RGB como sRGB.
- La salida es PNG RGBA de 8 bits, sin metadatos de entrada.
- El resultado conserva el ancho, el alto y cada byte alfa de la imagen orientada.
- La primera versión no gestiona perfiles ICC.

## Transparencia

- La paleta contiene colores RGB opacos. El alfa siempre viene de la imagen.
- Un píxel con alfa cero no recibe ni transmite error.
- Los promedios omiten los píxeles con alfa cero y ponderan los demás por alfa.
- El RGB de salida siempre pertenece a la paleta activa, también con alfa cero.

## Escala

- La escala va de 1 a 16 y solo se aplica a difusión de error y Bayer.
- La escala 1 usa una muestra por píxel.
- La escala N crea bloques de hasta N por N y conserva los bloques parciales del borde.
- Cada bloque usa su promedio RGB lineal ponderado por alfa.
- El motor amplía el color elegido con vecino más cercano y restaura el alfa original.

## Paleta y color

- Una paleta contiene de 2 a 8 colores y puede tener colores repetidos.
- Un empate elige el color con el índice menor.
- Distancias, promedios y errores usan RGB lineal con valores `f32`.
- El brillo usa los pesos 0,2126, 0,7152 y 0,0722.
- El orden de cálculo no cambia entre ejecuciones.

## Límites y errores

- El ráster rechaza dimensiones cero y tamaños de búfer incorrectos.
- Todo cálculo de tamaño usa aritmética comprobada antes de reservar memoria.
- Los límites iniciales son 16 384 píxeles por lado y 40 millones de píxeles.
- El motor devuelve errores con tipo. Los adaptadores crean los textos en español.

## Cancelación y progreso

- El motor comprueba la cancelación al menos una vez por fila lógica.
- El progreso va de 0 a 1 y nunca baja.
- La cancelación es un resultado normal del renderizado.
- El motor no usa archivos, ventanas, portapapeles ni estado global.
