# Academic Assistant — Requisitos funcionales

## 1. Objetivo
Academic Assistant es una aplicación de escritorio para centralizar y automatizar el seguimiento de la información académica de varias universidades. Su objetivo es detectar novedades relevantes en correo electrónico, resumirlas y estructurarlas mediante IA, evitando revisiones manuales constantes.

La primera versión se centrará en el correo. La arquitectura deberá permitir incorporar posteriormente Moodle, Blackboard, calendarios y otras fuentes.

## 2. Alcance inicial
Incluye:
- Gestión de múltiples cuentas desde la interfaz.
- Conexión mediante distintos proveedores/protocolos.
- Comprobaciones manuales y automáticas.
- Frecuencia de 1–2 comprobaciones diarias y horarios configurables.
- Ejecución en segundo plano.
- Filtros deterministas antes de la IA.
- Análisis de asunto y cuerpo únicamente.
- Clasificación y extracción estructurada mediante IA.
- Conservación de resultados de IA y enlaces a los correos originales.
- Estado nuevo/visto.
- Dashboard.
- Notificaciones nativas de Windows.
- Registro de ejecuciones y errores.
- Funcionamiento completamente local.
- Opción de utilizar una API externa mediante API key.

Fuera del alcance inicial:
- Adjuntos.
- Moodle.
- Blackboard.
- Calendarios.
- Scraping web.
- Agentes de navegador.
- Notion.
- Diseño visual definitivo de todas las pantallas.
- Comparativa de LLM.

## 3. Gestión de cuentas
El usuario podrá añadir, editar, activar/desactivar, eliminar y comprobar cuentas desde la aplicación. No se limitará inicialmente a UV y UAH.

La arquitectura deberá soportar distintos métodos de acceso, por ejemplo IMAP y Microsoft Graph/Exchange. Las credenciales y tokens no se almacenarán como texto plano en SQLite.

## 4. Obtención y filtrado
Cada ejecución localizará información nueva desde la última sincronización. Antes de utilizar IA se aplicarán filtros deterministas basados, entre otros, en remitente, dominio, destinatarios, asunto, carpeta, identificadores, estado de procesamiento y duplicados.

Los filtros tendrán como objetivo descartar información claramente irrelevante, no sustituir la clasificación semántica de la IA.

## 5. Análisis mediante IA
La primera versión analizará únicamente asunto y cuerpo. Para cada comunicación seleccionada deberá producir información estructurada:

- relevancia;
- universidad;
- asignatura/curso;
- profesor/remitente;
- tipo;
- resumen;
- acción requerida;
- prioridad;
- fechas;
- plazos;
- posibles cambios respecto a información anterior;
- enlace al correo original.

Tipos mínimos: aviso, tarea, examen, evento, cambio e información general.

## 6. Detección de cambios
Deberá poder identificar modificaciones de información previamente conocida, como cambios de fecha de examen o entrega, aula, cancelaciones o nuevas instrucciones.

La aplicación deberá poder relacionar comunicaciones relacionadas en lugar de tratar siempre cada correo como una novedad independiente.

## 7. Persistencia
Se conservarán los resultados estructurados de IA, metadatos necesarios para deduplicación/relación, estado de sincronización y registro de ejecuciones. No se almacenará indefinidamente el cuerpo completo de los correos.

Los resultados deberán permitir consultar qué ocurrió, cuándo se detectó, de qué cuenta procede, su clasificación y el enlace al mensaje original.

## 8. Avisos nuevos/vistos
Los resultados no revisados deberán distinguirse de los vistos. El usuario podrá marcarlos como vistos y consultar posteriormente el historial.

## 9. Dashboard
La vista principal deberá mostrar al menos última comprobación, estado, novedades y recuentos de tareas, exámenes, eventos y cambios/avisos. Incluirá una acción equivalente a «Comprobar ahora».

La especificación visual detallada se realizará posteriormente. También existirán vistas de configuración y gestión de cuentas.

## 10. Automatización
Se soportarán:
- ejecución manual;
- 1–2 ejecuciones automáticas al día;
- horarios configurables;
- ejecución aunque la ventana principal no esté abierta.

Cada ejecución tendrá estado y estadísticas. Un fallo de una cuenta no deberá impedir necesariamente procesar las demás.

## 11. Notificaciones
Se utilizarán notificaciones nativas de Windows para informar, cuando corresponda, de finalización, novedades relevantes y errores importantes.

## 12. Registro
Se registrarán como mínimo:
- inicio/fin;
- cuenta procesada;
- mensajes encontrados;
- mensajes descartados;
- mensajes enviados a IA;
- resultados creados;
- errores de conexión/autenticación/IA.

La forma de visualización de logs se decidirá posteriormente.

## 13. IA local y API externa
La aplicación deberá poder funcionar sin enviar información académica fuera del equipo.

Existirán dos posibilidades:
1. proveedor de IA local;
2. proveedor de IA externo configurado mediante API key.

El proveedor será intercambiable y la elección será explícita.

## 14. Privacidad
No se almacenarán contraseñas, tokens ni API keys en texto plano. No se enviará información a servicios externos salvo que el usuario configure un proveedor externo. Se minimizará la información persistida.

## 15. Extensibilidad
La arquitectura deberá permitir añadir posteriormente:
- Moodle de UV;
- Blackboard de UAH;
- calendarios;
- páginas web;
- otras plataformas.

## 16. Principios funcionales
1. Automatizar la recopilación.
2. Detectar cambios antes de usar IA cuando sea posible.
3. Minimizar procesamiento innecesario.
4. Transformar comunicaciones en información estructurada.
5. Mantener historial útil sin almacenar innecesariamente correos completos.
6. Mostrar siempre el estado de la última comprobación.
7. Desacoplar fuentes e IA.
8. Priorizar privacidad y funcionamiento local.
9. Permitir añadir cuentas y fuentes.
10. Mantener la base de datos como fuente de verdad, no la interfaz.
