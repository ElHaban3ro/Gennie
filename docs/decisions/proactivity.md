# Proactividad bajo demanda

2026-10-26. `apps/proactivity` proporciona un cliente bloqueante de decisiones bajo demanda, con `typesafe/jev-1.14` y `openai/gpt-6-luna-decisions` seleccionados explícitamente por llamada. Ambos usan `https://openrouter.ai/api/alpha/decisions`; el cliente limita preguntas y tamaños, tiempo de espera y redirecciones, y no registra contenido, respuestas ni credenciales. La respuesta se conserva estructurada para el futuro consumidor; no decide acciones ni inventa umbrales.

Bootstrap inyecta la clave opcional `GENNIE_OPENROUTER_API_KEY` por `Secrets`, sin incorporarla a `Config`, `AppState` o SQLite. No se escribe ni configura una clave por cuenta del usuario. La ausencia de clave no impide el arranque; una clasificación expresa el error al intentarse. No se crean observadores, scheduler, consulta de encargos ni llamadas de red al iniciar. No hay aún consumidor de interfaz ni automatización.

Fuentes: [OpenRouter Jev](https://openrouter.ai/blog/insights/what-is-jev/) y [OpenRouter GPT-6 Luna Decisions](https://openrouter.ai/openai/gpt-6-luna-decisions/). La prueba de contrato local no demuestra acceso a la cuenta, disponibilidad del proveedor ni calidad de decisión.
