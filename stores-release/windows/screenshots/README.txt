Windows Store Screenshots
========================

Required: At least one screenshot (1366 x 768 or larger, PNG format).
Recommended: 1920 x 1080 or higher.

How to capture:
  1. Build the app for Windows:
     cargo tauri build -p mcporb-runtime-app
  2. Run the built MCPOrb Runner
  3. Use Windows Snipping Tool (Win+Shift+S) or
     run the capture script:
     .\capture.ps1 -OutputName "library"
  4. Save screenshots into this directory as PNG

Suggested screenshots (capture these 4):
  1. library   — Library tab showing imported Orbs
  2. config    — MCP Config tab with generated STDIO snippets
  3. http      — HTTP tab with MCP server settings
  4. settings  — Settings tab showing configuration

File naming convention:
  mcporb-runner-win-{tab}-{resolution}.png
  Example: mcporb-runner-win-lib-1920x1080.png
