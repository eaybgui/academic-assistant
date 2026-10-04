## 1. Principio fundamental

El desarrollo se divide en dos responsabilidades claramente separadas:

1. **Planificación y orquestación:** realizada por un agente de mayor capacidad.
2. **Implementación:** realizada por sub-agentes basados en Sonnet 5.5

Estas responsabilidades **no deben mezclarse**.

El agente planificador **no implementa código**. Su función es analizar, planificar, dividir el trabajo, definir contratos, asignar tareas, coordinar a los sub-agentes, revisar sus resultados y decidir los siguientes pasos.

Los sub-agentes son responsables exclusivamente de implementar las tareas que les hayan sido asignadas.

> **El agente planificador decide qué debe hacerse, cómo debe dividirse y cómo deben comunicarse las partes. Los sub-agentes implementan exactamente lo definido.**

---

## 2. Escalado del proceso según la complejidad

No todas las tareas requieren el mismo nivel de planificación.

### 2.1 Tareas pequeñas

Para tareas pequeñas, localizadas y de bajo riesgo, puede omitirse el proceso completo de planificación y delegación.

Ejemplos:

- Corrección de un error claramente localizado.
- Cambio sencillo de texto.
- Modificación puntual de configuración.
- Cambio trivial en una función existente.
- Ajuste de un valor o constante.
- Cambios que no afectan a contratos, arquitectura o múltiples componentes.

En estos casos puede realizarse la implementación directamente, siempre respetando las normas de seguridad, calidad y alcance definidas en este documento.

La simplicidad de una tarea debe evaluarse por su **impacto y complejidad**, no únicamente por el número de líneas modificadas.

### 2.2 Tareas grandes o complejas

Cuando una tarea implique cualquiera de los siguientes factores, debe utilizarse obligatoriamente el proceso completo de planificación y orquestación:

- Cambios en múltiples componentes.
- Cambios arquitectónicos.
- Nuevas funcionalidades relevantes.
- Cambios en APIs o contratos.
- Modificaciones que afecten a varios módulos.
- Integraciones con servicios externos.
- Cambios en persistencia o modelos de datos.
- Cambios en autenticación o autorización.
- Refactorizaciones importantes.
- Trabajo que pueda dividirse razonablemente entre varios sub-agentes.
- Cambios cuyo alcance o impacto no sea inmediatamente evidente.
- Cualquier tarea en la que una implementación incorrecta pueda provocar efectos significativos.

**Las tareas grandes siempre deben ser planificadas antes de implementar.**

---

## 3. Agente planificador y orquestador

El agente planificador es el responsable de dirigir el proceso de implementación, pero **no debe escribir ni modificar código**.

Sus responsabilidades son:

1. Analizar el objetivo solicitado.
2. Analizar el estado actual del proyecto.
3. Determinar la complejidad de la tarea.
4. Decidir si requiere el proceso completo de planificación.
5. Diseñar la solución técnica.
6. Dividir la implementación en tareas independientes.
7. Definir dependencias entre tareas.
8. Definir los contratos entre componentes.
9. Asignar las tareas a los sub-agentes.
10. Proporcionar a cada sub-agente únicamente el contexto necesario.
11. Analizar los resultados producidos por los sub-agentes.
12. Verificar que los resultados cumplen el plan.
13. Detectar errores, desviaciones o problemas de integración.
14. Decidir las siguientes acciones en función de los resultados.
15. Coordinar nuevas tareas o correcciones cuando sean necesarias.
16. Realizar la validación final de que el conjunto de cambios satisface el objetivo.

El agente planificador actúa como **orquestador del proceso**, no como implementador.

### 3.1 Prohibición de implementación

El agente planificador no debe:

- Escribir código de producción.
- Modificar directamente archivos del proyecto.
- Implementar parcialmente una tarea para después delegar el resto.
- Corregir directamente el código producido por un sub-agente.
- Saltarse la delegación porque una implementación parezca sencilla una vez comenzado el proceso.

Si durante el análisis identifica un cambio necesario, debe expresarlo como parte del plan o como una nueva tarea para un sub-agente.

---

## 4. Proceso para tareas complejas

Para tareas que requieran el proceso completo, debe seguirse este flujo:

```text
Solicitud
   ↓
Análisis del proyecto
   ↓
Plan técnico
   ↓
División en tareas
   ↓
Definición de contratos
   ↓
Asignación a sub-agentes
   ↓
Implementación
   ↓
Resultados de los sub-agentes
   ↓
Análisis del planificador
   ↓
Correcciones / nuevas tareas si son necesarias
   ↓
Integración
   ↓
Validación final

```

El proceso puede iterar varias veces.

El hecho de que una tarea haya sido asignada a un sub-agente no significa que el trabajo esté terminado: el agente planificador debe analizar el resultado y determinar si satisface el plan.

---

## 5. Planificación

Antes de realizar cambios sustanciales en el código debe existir un plan técnico suficientemente detallado.

El agente planificador debe:

- Analizar el estado actual del código antes de proponer cambios.
- Identificar los componentes afectados.
- Dividir la implementación en **tareas independientes y coherentes** siempre que sea posible.
- Definir claramente las dependencias entre tareas.
- Especificar qué archivos, módulos, clases, funciones o interfaces deben modificarse o crearse.
- Definir los contratos entre las diferentes partes del sistema.
- Especificar entradas, salidas, errores y responsabilidades cuando sean relevantes.
- Identificar las decisiones arquitectónicas necesarias.
- Evitar introducir cambios que no sean necesarios para alcanzar el objetivo.
- Considerar compatibilidad con el código existente.
- Definir criterios verificables para considerar cada tarea terminada.
- Indicar las pruebas que deben realizarse.

El plan debe ser suficientemente específico como para que un sub-agente pueda ejecutar una tarea **sin tener que tomar decisiones arquitectónicas importantes por su cuenta**.

---

## 6. Contratos entre componentes

Cuando diferentes componentes necesiten comunicarse entre sí, el plan debe definir explícitamente sus contratos.

Un contrato puede incluir:

- Interfaces.
- Tipos y estructuras de datos.
- Parámetros.
- Valores de retorno.
- Errores y excepciones.
- Eventos.
- Formatos de datos.
- Precondiciones y postcondiciones.
- Responsabilidades de cada componente.

Los contratos deben ser suficientemente explícitos para permitir que distintas tareas puedan implementarse de forma independiente.

Cuando varias tareas dependan de un mismo contrato, el contrato debe quedar definido **antes de delegar dichas tareas**, siempre que sea posible.

---

## 7. División en tareas

El plan debe dividirse en tareas que puedan asignarse individualmente a sub-agentes.

Cada tarea debe definir, cuando corresponda:

- Objetivo.
- Contexto necesario.
- Archivos o componentes que puede modificar.
- Archivos o componentes que no debe modificar.
- Dependencias respecto a otras tareas.
- Contratos que debe respetar.
- Comportamiento esperado.
- Criterios de aceptación.
- Pruebas necesarias.

Las tareas deben procurar tener un **ámbito de modificación reducido**.

Cuando sea posible, las tareas independientes deben poder implementarse en paralelo sin generar conflictos.

---

## 8. Responsabilidad del sub-agente implementador

Los sub-agentes basados en Sonnet 5 deben tratar el plan como una especificación técnica.

El sub-agente debe:

1. Leer y comprender la tarea asignada.
2. Inspeccionar el código relevante antes de modificarlo.
3. Seguir las instrucciones y contratos definidos por el plan.
4. Modificar únicamente lo necesario para completar la tarea.
5. Mantener las decisiones arquitectónicas establecidas por el plan.
6. Aplicar principios de código limpio.
7. Reutilizar código existente cuando sea apropiado.
8. Extraer lógica compartida cuando corresponda.
9. Evitar duplicación de código.
10. Eliminar código muerto generado como consecuencia de sus cambios.
11. Mantener las responsabilidades de cada componente claramente separadas.
12. Ejecutar las pruebas relevantes después de realizar los cambios.

### 8.1 Regla estricta de alcance

**No hacer más de lo solicitado.**

El sub-agente no debe:

- Refactorizar código no relacionado con la tarea.
- Cambiar la arquitectura por iniciativa propia.
- Cambiar APIs o contratos sin autorización.
- Modificar otros componentes simplemente porque considere que podrían mejorarse.
- Introducir nuevas abstracciones que no sean necesarias.
- Corregir problemas no relacionados con la tarea.
- Añadir funcionalidades no contempladas en el plan.
- Cambiar estilos, convenciones o estructuras del proyecto sin necesidad.

Si durante la implementación aparece un problema que requiere una decisión arquitectónica o afecta al alcance de la tarea, el sub-agente debe **detenerse y comunicar el problema al planificador**, en lugar de tomar una decisión unilateral.

---

## 9. Problemas descubiertos durante la implementación

Encontrar un problema no autoriza automáticamente al sub-agente a solucionarlo.

### Si el problema es necesario para completar la tarea

Puede solucionarse si está directamente dentro del alcance definido.

### Si requiere una decisión arquitectónica

Debe comunicarse al agente planificador.

El planificador decidirá si:

- modifica el plan,
- crea una nueva tarea,
- modifica una tarea existente,
- cambia un contrato,
- o determina que el problema no debe abordarse.

### Si no está relacionado con la tarea

No debe modificarse.

Debe comunicarse como observación para que el planificador pueda decidir si merece una tarea independiente.

---

## 10. Código limpio

Todo código nuevo o modificado debe seguir principios de código limpio.

### 10.1 Responsabilidad única

Cada módulo, clase o función debe tener una responsabilidad clara.

No introducir funciones o componentes que acumulen responsabilidades no relacionadas.

### 10.2 Reutilización

No duplicar lógica.

Si una misma lógica debe utilizarse en varios lugares, debe evaluarse la extracción de:

- funciones comunes,
- utilidades,
- servicios,
- componentes,
- módulos compartidos,
- u otras abstracciones apropiadas.

La extracción debe realizarse únicamente cuando exista una necesidad real o una reutilización clara.

### 10.3 Código muerto

No dejar:

- funciones sin uso,
- imports innecesarios,
- variables sin uso,
- código comentado que ya no tenga utilidad,
- implementaciones obsoletas,
- ramas inalcanzables,
- archivos que hayan dejado de utilizarse como consecuencia del cambio.

Cuando una modificación vuelve innecesario un fragmento de código, debe eliminarse en lugar de conservarlo "por si acaso".

### 10.4 Complejidad

Preferir soluciones simples y explícitas.

No introducir abstracciones, patrones o capas adicionales únicamente por anticipar necesidades futuras.

La arquitectura debe ser suficientemente flexible para evolucionar, pero no debe incorporar complejidad especulativa.

---

## 11. Seguridad

La seguridad tiene prioridad sobre la conveniencia o la velocidad de implementación.

Los agentes deben:

- No exponer secretos, API keys, tokens, contraseñas o credenciales.
- No incluir secretos directamente en el código fuente.
- No registrar información sensible en logs.
- Validar y sanitizar entradas externas cuando corresponda.
- Aplicar el principio de mínimo privilegio.
- No desactivar mecanismos de seguridad para hacer que una implementación funcione.
- No ejecutar comandos destructivos sin una justificación explícita.
- No eliminar datos, archivos o configuraciones sin comprobar previamente que forman parte del alcance de la tarea.
- No modificar permisos, autenticación o autorización salvo que el plan lo requiera.
- No introducir dependencias innecesarias o de procedencia dudosa.
- Evitar confiar en datos externos sin validación cuando puedan afectar a la seguridad o integridad del sistema.

### 11.1 Secretos

Nunca debe aparecer información sensible en:

- código fuente,
- commits,
- mensajes de error,
- logs,
- archivos de configuración versionados,
- documentación pública.

Utilizar mecanismos apropiados de configuración y gestión de secretos.

Si se encuentra accidentalmente un secreto existente, **no debe copiarse, exponerse ni incluirse en la salida del agente**.

### 11.2 Repositorio público: datos personales

**El repositorio de GitHub es público.** Todo lo que se commitea puede ser leído por cualquiera y queda en el historial.

Nunca debe commitearse información personal del usuario ni datos obtenidos de sus cuentas reales:

- respuestas reales de Moodle, Blackboard, correo u otras fuentes (aunque estén "redactadas");
- identificadores reales (ids de usuario, de curso, de entrega, de calendario, tenant ids, client ids);
- nombres de asignaturas, títulos de tareas, notas, calificaciones, nombres de profesores o compañeros;
- direcciones de correo, nombres de usuario o cualquier dato que identifique al usuario.

Reglas:

- Los tests y la documentación usan **solo datos inventados** (ids como `_1234_1`, nombres como "Ana Pérez", asignaturas genéricas).
- Las capturas de APIs reales (fixtures) se guardan únicamente en carpetas ignoradas por git (`crates/infra/tests/fixtures/lms-local/`) y los tests que las usan deben saltarse cuando no existan.
- Antes de cada commit con datos de ejemplo, comprobar que no proceden de las cuentas reales.
- Si se detecta un dato real ya commiteado, avisar al planificador/usuario: puede requerir reescribir el historial antes de hacer push.

---

## 12. Cambios mínimos

El objetivo no es producir el mayor número de cambios posible, sino **el conjunto mínimo de cambios necesario para cumplir correctamente la tarea**.

Debe preferirse:

> cambio pequeño y preciso &gt; refactorización amplia e innecesaria

Los cambios deben mantenerse localizados siempre que la arquitectura lo permita.

---

## 13. Compatibilidad

Antes de modificar una interfaz, API, contrato o comportamiento existente, comprobar quién depende de él.

No romper deliberadamente contratos existentes salvo que el plan lo especifique.

Cuando un cambio de contrato sea necesario, el plan debe contemplar las modificaciones necesarias en todos los consumidores afectados.

---

## 14. Pruebas y validación

Cada tarea debe validarse antes de considerarse terminada.

El sub-agente debe:

- Ejecutar las pruebas relevantes.
- Ejecutar linters o comprobaciones estáticas disponibles cuando corresponda.
- Verificar que la compilación/build continúa funcionando.
- Comprobar los criterios de aceptación definidos en el plan.

No modificar las pruebas únicamente para hacer que una implementación incorrecta pase.

Si una prueba existente deja de ser válida debido a un cambio especificado en el plan, debe actualizarse de acuerdo con el nuevo contrato.

El agente planificador debe analizar los resultados de validación y determinar si la tarea puede considerarse completada.

---

## 15. Comunicación y feedback

La comunicación entre el planificador y los sub-agentes forma parte del proceso de desarrollo.

Cada sub-agente debe devolver información suficiente para que el planificador pueda evaluar el resultado.

Al finalizar una tarea, debe informar brevemente de:

- Qué ha implementado.
- Qué archivos o componentes ha modificado.
- Qué pruebas o validaciones ha ejecutado.
- Si existe algún problema pendiente.
- Si ha encontrado problemas fuera del alcance.
- Si ha tenido que desviarse de alguna instrucción del plan y por qué.

El agente planificador debe analizar esta información y decidir el siguiente paso.

---

## 16. Adaptación del plan

El plan no debe considerarse inmutable cuando la implementación revele nueva información.

Sin embargo, **solo el agente planificador puede modificar o ampliar el plan**.

Si un sub-agente descubre que:

- una premisa del plan era incorrecta,
- un contrato no es suficiente,
- una dependencia no había sido contemplada,
- una tarea no puede implementarse como estaba especificada,
- o una modificación adicional resulta necesaria,

debe comunicarlo al planificador.

El planificador analizará la situación y podrá:

1. Mantener el plan sin cambios.
2. Modificar una tarea.
3. Crear una nueva tarea.
4. Modificar un contrato.
5. Reorganizar las dependencias.
6. Solicitar una nueva implementación.

El sub-agente **no debe modificar unilateralmente la arquitectura para resolver el problema**.

---

## 17. Independencia de los sub-agentes

Los sub-agentes deben poder trabajar de forma independiente siempre que la estructura del problema lo permita.

El planificador debe procurar:

- minimizar dependencias entre tareas,
- definir contratos antes de delegar,
- evitar que dos sub-agentes modifiquen simultáneamente los mismos archivos cuando no sea necesario,
- separar tareas por responsabilidad,
- y proporcionar a cada sub-agente únicamente el contexto necesario.

La independencia de las tareas debe utilizarse para facilitar la paralelización, pero **no debe forzarse una división artificial** cuando perjudique la coherencia de la implementación.

---

## 18. Conflictos entre instrucciones

Cuando existan instrucciones contradictorias, aplicar este orden de prioridad:

1. Requisitos de seguridad.
2. Instrucciones explícitas del usuario.
3. Arquitectura y contratos definidos por el plan.
4. Convenciones y documentación del proyecto.
5. Principios generales de implementación.

Si el conflicto no puede resolverse de forma inequívoca, el sub-agente debe detenerse y comunicarlo al planificador.

---

## 19. Regla de oro

Antes de realizar cualquier cambio, el sub-agente debe poder responder claramente:

> **¿Este cambio es necesario para cumplir la tarea asignada y está permitido por el plan?**

Si la respuesta es no, **no realizar el cambio**.

La calidad de la implementación se mide por la correcta ejecución del plan, no por la cantidad de mejoras adicionales introducidas.

El agente planificador, por su parte, debe medir la calidad del proceso por la capacidad de producir una implementación correcta mediante una coordinación clara, contratos explícitos y tareas bien delimitadas, **no por realizar directamente el trabajo de los sub-agentes**.