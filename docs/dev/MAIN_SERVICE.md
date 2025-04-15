# NAME
mi4ulings

# LOCATION
src/main.rs
# DESCRIPTION
main service using tokio, and ntex web server. it will use cdn tailwind and daisyui for interface.
main route will display dashboard and menu,
one of menu will show that we can do all of commands that we can on crates/docling, including adding new entry, edit entry, start or start all, check logs and view or download results.
another menu will allow to see and edit any config from application.
app will use maud template system: https://maud.lambda.xyz

# STACK
- tokio
- ntex
- maud


# CONFIG
-  bind address (default 0.0.0.0)
- port (default 9911)
-


# PLAN
[] Define API endpoints for docling actions (add, list, start, stop, remove, view logs, view results).
[] Define API endpoints for viewing/editing configurations (`mi4ulings-config`, `mi4ulings-docling`).
[] Design UI structure using Maud templates (Dashboard, Docling Management, Config Management).
[] Implement basic Ntex web server setup.
[] Implement handlers for API endpoints, integrating with `mi4ulings-docling` and `mi4ulings-config` crates.
[] Implement Maud template rendering for UI views.
[] Integrate Tailwind/DaisyUI via CDN links in templates.
[] Add state management (e.g., shared application state for docling entries, config).
[] Implement background task management for starting/monitoring docling processes.
[] Add basic authentication/authorization (if needed later).
[] Create unit/integration tests for API handlers and core logic.
[] Implement comprehensive logging for the web service.
[] Test the complete web application.