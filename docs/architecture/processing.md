# Coordinación del procesamiento

`dither-desktop` mantiene un solo hilo de renderizado durante la sesión. La interfaz envía cada
pedido con un documento y una revisión. El coordinador añade una generación interna para impedir
que dos pedidos con una identidad repetida se confundan.

La ranura pendiente solo conserva el pedido más reciente. Un cambio continuo espera 100 ms; una
acción final puede iniciar el cálculo sin espera. Al llegar un pedido nuevo, el trabajo activo
detecta el cambio de generación mediante un valor atómico y se cancela en su siguiente fila
lógica.

El canal de eventos admite hasta 32 mensajes. El progreso se limita a unas 30 actualizaciones por
segundo y nunca baja. Cada evento incluye documento y revisión. El coordinador descarta eventos de
generaciones anteriores antes de entregarlos a la interfaz.

Al cerrar, el coordinador cancela el trabajo, elimina el pedido pendiente y despierta el hilo. La
espera tiene un tiempo máximo. Si un adaptador defectuoso ignora la cancelación, la aplicación
puede soltar el hilo sin esperar de forma indefinida.
