# PROJECT NAME
MI4ULINGS

## DESCRIPTION
workspace for small projects around ai, agentic tools, documentation, etc.
workspace will keep dependences and optional dependences for packages crates inside
all program will use package crates/config to serialize/deserialize/save&load configs, that will be stored in .config directory of workspace, each crate in this workspace will have own .toml file that will have stored all of needed configs. 
all crates uses anyhow with tracing to handle result .

## PROGRAMING LANGUAGE
rust

## STACK
- tokio 
- serde
- serde_json
- toml
- anyhow
- reqwest


### STACK - tui
- clap

### STACK - CARGO SUBCOMMANDS GLOBAL
- cargo-doc2readme
- cargo-readme 
- cargo-instruments
- cargo-docs
- cargo-tally
- cargo-nextest
- cargo-insta
- cargo-whatfeatures
- cargo-codspeed
- cargo-chef
- cargo-audit 
- cargo-shear
- cargo-px


## STRUCTURE
### WORKSPACE CRATES:
./crates/* 
### WORKSPACE CONFIGS:
./config/*.toml
### CRATES CONFIG MANAGEMENT:
./crates/config
### DEVELOPEMENT
#### INSTALL BARE MINIMUM TO INSTALL REST OF THE TOOLS
./init-install.sh
#### INSTALL REST
just init

## WORKSPACE CRATES:
 - [config](./CONFIG.md)
 - [docling](./DOCLING.md)



## PLAN:

### v.0.1

[*] create rust workspace, with shared crates
[*] write a detailed plan of execution after this point
[*] review and implement [mi4uling-config](./CONFIG.md)
[*] review and implement [mi4uling-docling](./DOCLING.md)
[*] write a detailed plan of execution up to this point
[*] review whole project

### v.0.2

[] Move implementation summary from every doc in docs/dev into separate file with the same name but with suffix _worklog.md for example
for file PLAN-01.md it will be PLAN-01_worklog.md, and after each point in PLAN list change it will result in entry in new file in format:
```*date and time of entry*

- [REF](./LINK_TO_PLAN.md) - quoted text from changed plan line  (CHANGE STATE_BEFORE -> STATE_AFTER)

- SUMMARY: 
    - one line description summarize change

- DESCRIPTION: 
    - longer explanation of what was change

- REASONING: 
    - non mandatory reasoning explanation

- STATUS: 
    - error/success/test fail/test passed/blocked

- STATUS_EXPLANATION:
    - add only if needed, for example on error/blocked/fail
```

for example:


*02.04.2025 12:45*

[REF](./PLAN-01.md#v01) - write a detailed plan of execution up to this point  (CHANGE [] -> [*])

- SUMMARY: 
    - added summary to the PLAN-01

DESCRIPTION: 
    - added detailed plan of execution up to this point, which includes:
        1. Implemented configuration management system in the config crate
        2. Implemented web crawler with configurable depth in the docling crate
        3. Added HTML to Markdown conversion with multiple methods
        4. Created content processor for cleaning and combining Markdown
        5. Implemented error handling with retry mechanism
        6. Built CLI interface with all required commands

STATUS: success


- from now on act like that after every change in plan change or action leading to change this point

## IMPLEMENTATION SUMMARY:

### Config Crate
- Implemented configuration management system using TOML files
- Added backup functionality with timestamp-based versioning
- Created cleanup mechanism for old backups
- Provided generic trait for easy implementation in other crates

### Docling Crate
- Implemented web crawler with configurable depth using spider library
- Added HTML to Markdown conversion with three methods (htmd, fast_html2md, jina_reader)
- Created content processor for cleaning and combining Markdown
- Implemented robust error handling with retry mechanism
- Built CLI interface with all required commands (add, stop, list, remove, start)
