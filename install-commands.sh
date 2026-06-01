# Memryzed: delete + fresh install (WSL, root, ~/.memryzed)

# --- DELETE ---
# 1. Clean uninstall (removes MCP server entries from each client)
~/.memryzed/bin/memryzed uninstall 2>/dev/null

# 2. Stop any running Memryzed MCP server
pkill -f "memryzed serve" 2>/dev/null; sleep 1

# 3. Delete binary, database, and all Memryzed data
rm -rf ~/.memryzed

# 4. Remove the steering rule (uninstall does not remove this)
rm -f ~/.kiro/steering/memryzed.md
# (Do NOT sed the agent/settings JSON files - it breaks JSON.
#  Leftover @memryzed / mcp__memryzed entries are harmless; a
#  fresh install re-adds them cleanly.)

# --- INSTALL ---
# 5. Fresh install of the latest build
curl -fsSL https://memryzed.com/install.sh | MEMRYZED_ALLOW_ROOT=1 bash

# 6. Set up
export PATH="$HOME/.memryzed/bin:$PATH"
memryzed init
memryzed install
memryzed doctor

# 7. Restart Kiro and Claude Code so they reload the MCP server,
#    steering rule, and auto-approve trust.
