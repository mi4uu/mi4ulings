# NAME
mi4ulings-config

# LOCATION
crates/config

# DESCRIPTION
store configs for itself and other crates in .config directory of workspace root, as .toml files, one for each of crates.
implement save,load to file,get_location and load_or_default .
if load_or_default is used and default is returned than file is also saved.
before save it always do backup of files that is going to be overrited adding date and time as suffix to file name and moving it to folder .backup inside config folder.
other crates uses this lib, they need to define struct for settings. each setting need to defer serde serialize and deserialize.

# STACK
- toml
- serde


# CONFIG
-  cleanup_backups_after_days (default 30)

# PLAN (Historical - See CONFIG_worklog.md for details)
[x] create config implementation & implement program
[-] create unit tests for planed program
