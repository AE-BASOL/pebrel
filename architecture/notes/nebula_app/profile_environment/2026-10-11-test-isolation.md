# Owned profile regression fixture isolation

Profile model/store is a standalone API foundation. Its existing bare-WSL workspace
regression is retained but is run only in the launch layer's isolated child, not
in the pure model plan. The fixture program being missing cannot prevent Windows
WSL hook preparation: that worker independently uses `wsl.exe`.

The cfg(test) profile fixture starts the exact test in a fresh process and writes
owned `pebrel_settings.txt` before configuration OnceLocks or constructors. It
asserts all AgentHook preferences plus the legacy switch are false; automatic
updates, session restore/resume and environment refresh are disabled. Config/data
paths must resolve to the owned directory. It clears open-document, agent socket,
PTY-record, provider identity redirects and actual runtime/notify/helper/token
markers without redefining host HOME. Their names are removed without reading values.

`platform::dirs::ssh_config_home` is a narrow cfg(test) override consumed by both
SSH config path/probe and host discovery, preserving the production home resolver.
The child asserts the path, read and discovery routes all see its empty owned SSH
file. This is not an override for native stored SSH password/keychain access:
Auto-auth and mixed SSH fixtures remain excluded from the exact plan.

Workspace/PTY/editor scenarios use synthetic argv, environment values and owned
files. Neither this isolation nor Mac synthetic WSL argv tests establish native
Windows WSL behavior. No native credentials are read or written.
