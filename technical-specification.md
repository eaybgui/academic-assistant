# Academic Assistant — Especificación técnica

## 1. Stack inicial
- Tauri.
- Rust.
- React.
- TypeScript.
- Tailwind CSS.
- SQLite.

La aplicación será local-first, modular y extensible.

## 2. Arquitectura

```text
Tauri Desktop
├── React + TypeScript + Tailwind
│   ├── Dashboard
│   ├── Avisos / historial
│   └── Configuración
│
└── Rust Core
    ├── Scheduler
    ├── Source orchestration
    ├── Change detection
    ├── AI orchestration
    ├── Notifications
    ├── Secure credentials
    └── SQLite
         ├── Accounts
         ├── Sync state
         ├── AI results
         └── Runs
```

React no accederá directamente a correo, SQLite o proveedores externos. Se comunicará con Rust mediante comandos/eventos de Tauri.

## 3. Capas

### Presentation
React/TypeScript: presentación, formularios, configuración, estado de lectura e invocación de comandos.

### Application/Core
Rust: orquestación, sincronización, filtros, IA, persistencia, scheduler y notificaciones.

### Infrastructure
Implementaciones concretas de SQLite, IMAP, Microsoft Graph/Exchange, proveedores de IA, almacenamiento seguro y notificaciones.

Las capas superiores dependerán de abstracciones y no de implementaciones concretas.

## 4. Providers de correo

Se definirá una abstracción equivalente a:

```rust
trait EmailProvider {
    fn test_connection(&self) -> Result<ConnectionStatus>;
    fn get_new_messages(&self, state: &SyncState) -> Result<Vec<Email>>;
}
```

Implementaciones previstas:

```text
EmailProvider
├── ImapEmailProvider
└── MicrosoftGraphEmailProvider
```

Cada provider convertirá su protocolo a un modelo común.

## 5. Modelo normalizado

```text
Email
├── account_id
├── provider_message_id
├── message_id
├── sender
├── recipients
├── subject
├── date
├── body
├── folder
└── source_url
```

El mensaje completo podrá existir temporalmente durante el procesamiento sin convertirse en almacenamiento histórico permanente.

## 6. Sincronización incremental

Cada cuenta tendrá estado independiente:

```text
SyncState
├── account_id
├── last_run_at
├── last_success_at
├── provider_cursor / last_uid
└── status
```

La sincronización utilizará el cursor/UID disponible y, cuando sea útil, Message-ID y hash del contenido. No dependerá únicamente de fechas.

## 7. Pipeline

```text
Scheduler / Check now
        ↓
Cuentas activas
        ↓
Sincronización de correo
        ↓
Normalización
        ↓
Filtros deterministas
        ↓
Candidatos nuevos
        ↓
AIProvider
        ↓
Resultado estructurado
        ↓
Detección/relación de cambios
        ↓
SQLite
        ↓
UI + notificaciones
```

Cada etapa deberá poder registrar errores y métricas.

## 8. Filtros
Antes de IA se podrán descartar:
- mensajes ya procesados;
- duplicados;
- carpetas no relevantes;
- remitentes/dominios bloqueados;
- promociones claramente irrelevantes;
- mensajes propios no relevantes;
- reglas futuras del usuario.

La estrategia por defecto será conservadora para evitar falsos negativos.

## 9. AIProvider

Se abstraerá el proveedor:

```rust
trait AIProvider {
    fn analyze_email(&self, email: &Email) -> Result<AIAnalysis>;
}
```

Implementaciones conceptuales:

```text
AIProvider
├── ExternalApiProvider
└── LocalProvider
```

El núcleo no dependerá de un proveedor concreto. La salida será estructurada y validada antes de persistirla.

## 10. Resultado de IA

```text
AIAnalysis
├── relevant
├── university
├── course
├── professor
├── category
├── summary
├── action_required
├── priority
├── dates[]
├── deadlines[]
├── possible_change
├── change_description
└── confidence
```

Los campos podrán evolucionar durante el desarrollo.

## 11. Detección de cambios

Los resultados podrán relacionarse semánticamente. Ejemplo:

```text
Resultado A: examen 20/06
        ↓
Resultado B: examen trasladado al 27/06
        ↓
Cambio: 20/06 → 27/06
```

La primera implementación podrá utilizar heurísticas y procesamiento estructurado antes de introducir mecanismos más complejos.

## 12. SQLite

SQLite será la fuente de verdad local.

Ventajas para esta fase:
- aplicación de escritorio;
- un único usuario;
- sin servidor;
- persistencia en un archivo;
- transacciones;
- consultas y búsqueda;
- backups sencillos;
- integración natural con Rust.

## 13. Modelo de datos inicial

### Account

```text
Account
├── id
├── name
├── email_address
├── provider_type
├── configuration
├── enabled
├── created_at
└── updated_at
```

### SyncState

```text
SyncState
├── account_id
├── last_run_at
├── last_success_at
├── provider_cursor
└── status
```

### AIResult

```text
AIResult
├── id
├── account_id
├── source_message_id
├── detected_at
├── university
├── course
├── professor
├── category
├── summary
├── action_required
├── priority
├── dates
├── deadlines
├── change_data
├── confidence
├── source_url
└── seen
```

### Run

```text
Run
├── id
├── started_at
├── finished_at
├── status
├── accounts_processed
├── messages_found
├── messages_filtered
├── messages_analyzed
├── results_created
└── error_summary
```

El esquema definitivo se gestionará mediante migraciones versionadas.

## 14. Retención

No se almacenará históricamente el cuerpo completo de cada correo. Se conservarán resultados de IA, metadatos de deduplicación, estado de sincronización, historial de ejecuciones y relaciones entre resultados.

El contenido completo podrá mantenerse únicamente en memoria/almacenamiento temporal durante el procesamiento.

## 15. Credenciales

Se priorizará:
- OAuth cuando exista;
- tokens en almacenamiento seguro del sistema operativo;
- Credential Manager/keychain equivalente;
- API keys en secure storage.

SQLite podrá almacenar referencias, pero no secretos en texto plano. Los logs tampoco incluirán credenciales, tokens, API keys ni cuerpos completos.

## 16. Scheduler

Rust gestionará:
- ejecución manual;
- 1–2 ejecuciones diarias;
- horarios configurables;
- ejecución en segundo plano;
- recuperación razonable ante errores.

La solución concreta de Windows podrá utilizar el mecanismo de tareas/inicio apropiado sin depender de que React esté abierto.

## 17. Notificaciones

Se abstraerá un servicio:

```text
NotificationService
├── notify_check_completed()
├── notify_new_results()
└── notify_error()
```

La implementación inicial usará notificaciones nativas de Windows.

## 18. React ↔ Rust

Comandos conceptuales:

```text
check_now()
get_dashboard()
get_results()
mark_result_seen(id)
get_accounts()
add_account(...)
update_account(...)
remove_account(id)
test_account(id)
get_settings()
update_settings(...)
get_run_history()
```

La lógica crítica permanecerá en Rust.

## 19. Configuración

Se persistirán:
- cuentas;
- activación/desactivación;
- horarios;
- frecuencia;
- proveedor de IA;
- configuración del proveedor externo;
- notificaciones;
- preferencias futuras.

La interfaz concreta se diseñará posteriormente.

## 20. Errores y observabilidad

Los errores se aislarán por cuenta/componente cuando sea posible.

Ejemplo:

```text
Run
├── UV → OK
├── UAH → authentication error
└── AI → OK
```

Se registrarán duración, cuentas, mensajes encontrados/filtrados/analizados, resultados y errores.

## 21. Extensibilidad de fuentes

Se preverá una abstracción general:

```rust
trait SourceProvider {
    fn sync(&self, state: &SourceState) -> Result<Vec<SourceItem>>;
}
```

Futuras implementaciones:

```text
SourceProvider
├── EmailSource
│   ├── IMAP
│   └── Microsoft Graph
├── UVMoodleSource
├── UAHBlackboardSource
└── CalendarSource
```

Esto permitirá reutilizar detección de cambios, IA, almacenamiento y presentación.

## 22. Estrategia para futuras plataformas

Orden de preferencia:
1. API oficial.
2. Protocolo estándar.
3. Scraping/control HTTP estable.
4. Automatización de navegador solo cuando sea necesaria.

No se usará un agente autónomo si una interfaz estable puede resolver el problema.

## 23. Agentes

El núcleo inicial será determinista:

```text
obtener → filtrar → analizar → estructurar → guardar
```

La IA se utilizará para clasificación, extracción, resumen, acciones y cambios. Los agentes de navegador quedan como posible herramienta futura para fuentes especialmente difíciles.

## 24. Testing

### Unit tests
- filtros;
- deduplicación;
- sincronización incremental;
- parsing;
- validación de IA;
- fechas;
- prioridad;
- detección de cambios.

### Integration tests
- SQLite;
- providers de correo;
- providers de IA;
- scheduler;
- ejecución completa.

Los providers deberán poder sustituirse por mocks/fakes.

## 25. Primera versión técnica

```text
Tauri
├── React + TypeScript + Tailwind
│
└── Rust
    ├── SQLite
    ├── Account management
    ├── IMAP provider
    ├── Email normalization
    ├── Incremental sync
    ├── Deterministic filtering
    ├── AIProvider
    ├── Local AI provider
    ├── External API provider
    ├── Scheduler
    ├── Windows notifications
    └── Run logging
```

El primer hito funcional deberá permitir:
1. añadir una cuenta;
2. validar conexión;
3. ejecutar una comprobación manual;
4. obtener mensajes nuevos;
5. filtrar;
6. analizar candidatos;
7. validar y guardar resultados;
8. mostrar novedades;
9. marcar resultados como vistos;
10. registrar la ejecución;
11. notificar en Windows;
12. repetir automáticamente el proceso.
