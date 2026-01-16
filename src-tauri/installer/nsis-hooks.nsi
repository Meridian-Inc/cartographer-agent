; NSIS hooks for Cartographer Agent installer
; This script handles cleanup during uninstallation

!macro NSIS_HOOK_PREUNINSTALL
  ; Clean up Cartographer credentials and state files on uninstall
  ; This ensures the user is effectively "disconnected" when the app is removed
  
  ; Delete credentials from the config directory
  Delete "$LOCALAPPDATA\cartographer\credentials.json"
  RMDir "$LOCALAPPDATA\cartographer"
  
  ; Delete agent state from the data directory
  Delete "$LOCALAPPDATA\cartographer-agent\agent_state.json"
  RMDir "$LOCALAPPDATA\cartographer-agent"
  
  ; Also try the roaming appdata location (some systems use this)
  Delete "$APPDATA\cartographer\credentials.json"
  RMDir "$APPDATA\cartographer"
  Delete "$APPDATA\cartographer-agent\agent_state.json"
  RMDir "$APPDATA\cartographer-agent"
!macroend
