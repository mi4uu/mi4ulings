# NAME
mi4ulings-docling

# LOCATION
crates/docling

# DESCRIPTION
store in toml file list of urls to download with
last download date and time, last try date and time, last fail date and time, how deep it should crawl, 
status (enabled, disabled, failed - halted), version, name
it has bin that take actions:
    - add [url]
    - stop [name]
    - list 
    - remove [name]
    - start [name]

- when element is started, program should use spider library and tokio, and try to gather all links in provided url, to the given depth,
links need to starts with provided url.
- next it will go thru all of the links, download pages and convert to markdown using twars-url2md. each of them will

# STACK
- spider
- tokio


# CONFIG
-  inputs_path 
-  outputs_path_partials
- output_path_results
-  retry_count (default 3)
- retry_delay (array of retry_count elements in seconds - default: 10, 40, 200)
- refetch after days (default 100)
- default deep


# PLAN
[] plan step by step program execution with details
[] extending this plan with details, and fix mistakes
[] create config implementation
[] create unit tests for planed program
[] implement program
[] test program