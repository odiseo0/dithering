# Informe de endurecimiento

Fecha: 2026-09-16  
Destino: Windows 11 x64, versión optimizada

Este informe fija los límites de la primera versión y guarda evidencia de la fase 9. La sonda se
puede repetir con:

```powershell
.\scripts\hardening.ps1
```

## Archivos dañados y límites

- La prueba de integración cubre datos vacíos, cabeceras truncadas, bytes arbitrarios, formatos no
  admitidos y todos los prefijos truncados de un PNG válido.
- APNG y WebP animado se rechazan antes de tomar un cuadro.
- El códec lee la cabecera PNG y valida el tamaño antes de reservar el búfer de píxeles.
- El texto principal siempre pertenece a la aplicación y está en español. El error técnico solo se
  muestra en la sección «Detalles».
- El límite final es 16.384 píxeles por lado, 10.000.000 de píxeles y 512 MiB para el archivo
  comprimido. Este tope admite 4K UHD, que tiene 8.294.400 píxeles.

## Memoria y cancelación

La sonda procesó una imagen 4K UHD con Floyd–Steinberg, escala 1 y paleta blanco y negro. El máximo
del proceso fue 312,1 MiB y el render tardó 1.048 ms. La estimación lineal para el límite final es
376 MiB durante el render. Un resultado anterior puede sumar unos 38 MiB y el caché de texturas
tiene un tope aparte de 128 MiB.

El objetivo de cancelación queda fijado en 100 ms. La primera medición tardó 317 ms y detectó que la
reducción por escala no consultaba la señal. Tras añadir una consulta por fila lógica, la misma
sonda respondió en menos de 1 ms, que es la resolución mostrada por la prueba.

## Cambios rápidos y prueba larga

La prueba del coordinador confirma que tres cambios pendientes conservan solo el último y que un
resultado viejo no se aplica, incluso si un motor no atiende la cancelación.

La sonda larga envió 263.500 revisiones durante 30 segundos. El proceso mantuvo cinco hilos en las
ventanas estables del primer y último cuarto. El máximo fue 13,9 MiB y el máximo de la ventana final
quedó 4,3 MiB por encima del de la ventana inicial. El criterio automático falla si aparecen hilos
nuevos o si el crecimiento supera 32 MiB.

El caché de vista tiene una prueba propia con 10.000 revisiones de textura. Fuerza el desalojo en
cada vuelta y comprueba que los bytes y la cantidad de entradas no superan sus topes.

## Dependencias y licencias

La aplicación usa Glow como único motor gráfico. El grafo de producción para Windows x64 contiene
162 paquetes externos fijados por `Cargo.lock`. Todos declaran una licencia. El archivo
[`THIRD-PARTY-NOTICES.txt`](../THIRD-PARTY-NOTICES.txt) incluye el inventario, la expresión SPDX y
los textos de licencia presentes en cada paquete. Se regenera sin red con:

```powershell
.\scripts\generate-third-party-notices.ps1
```

El ZIP de la fase 10 debe incluir ese archivo junto al ejecutable y la nota de uso. El proyecto aún
no declara una licencia propia; no se debe presentar el código de la aplicación como software con
licencia abierta hasta que se tome esa decisión.

## Límites conocidos

- No se gestionan perfiles ICC; una imagen de gama amplia puede cambiar de color.
- El resultado no conserva EXIF, comentarios ni otros datos.
- El límite de memoria busca equipos de 8 GiB o más. Un equipo con poca memoria libre aún puede
  rechazar una reserva del sistema.
- La prueba larga mide el coordinador y el proceso. La prueba del caché de texturas usa un contexto
  de `egui`; la fase 10 aún debe hacer la prueba manual final con varios controladores gráficos.
