# Visor de imágenes

El visor guarda el centro en coordenadas de la imagen. Por eso el centro visible no cambia al
alternar entre original y resultado. El zoom representa píxeles físicos de pantalla por píxel de
imagen: 100 % muestra una relación uno a uno incluso con escalado de Windows.

“Ajustar” calcula el mayor zoom que muestra la imagen completa. El intervalo permitido va de 5 %
a 3200 %. `Ctrl` más rueda cambia el zoom alrededor del puntero. La rueda sola desplaza la vista.
También se puede desplazar con el botón central o con espacio más botón izquierdo.

La vista usa una textura reducida con filtrado bilineal al ajustar o alejar. El filtrado pondera
el color por alfa para que el RGB invisible no cause bordes. A 100 % y con zoom alto, el visor
crea solo los mosaicos visibles, más un margen. Cada mosaico mide como máximo 1024 × 1024 y nunca
supera el límite de textura del equipo.

La clave de caché incluye documento, revisión y tipo de imagen. El límite inicial es 128 MiB. Los
mosaicos menos usados salen primero y las texturas de otro documento se eliminan al abrir una
imagen. Un fondo de cuadros se dibuja antes de original y resultado para mostrar la transparencia.
