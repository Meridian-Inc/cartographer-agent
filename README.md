# Cartographer Agent

A standalone desktop application that monitors your local network and syncs device information with Cartographer Cloud.

## Features

- **Network Discovery**: Automatically scans your local network to discover devices
- **Background Monitoring**: Runs in the system tray, performing periodic scans
- **Cloud Sync**: Uploads scan results to Cartographer Cloud for visualization
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Easy Setup**: Simple sign-in flow to link your agent to your cloud account

## Installation

### Prerequisites

- **Windows**: Windows 10 or later
- **macOS**: macOS 10.15 or later
- **Linux**: Most modern distributions with system tray support

### Download

Download the latest release for your platform from the [Releases](https://github.com/your-org/cartographer-agent/releases) page:

- **Windows**: `Cartographer Agent.exe` installer
- **macOS**: `Cartographer Agent.app` (may require Gatekeeper approval)
- **Linux**: `cartographer-agent.AppImage` or `.deb` package

### Building from Source

#### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Node.js](https://nodejs.org/) (v18 or later)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)

#### Setup

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri:dev

# Build for production
npm run tauri:build
```

## Usage

### First Launch

1. Launch the application
2. Click "Sign In" to link your agent to your Cartographer Cloud account
3. Complete authentication in your browser
4. The app will minimize to the system tray and begin background scanning

### System Tray

The app runs in the background and can be accessed via the system tray:

- **Windows/Linux**: System tray (bottom right)
- **macOS**: Menu bar (top right)

**Tray Menu Options:**
- **Open Dashboard**: View scan results and device list
- **Scan Now**: Manually trigger a network scan
- **View in Cloud**: Open your dashboard in Cartographer Cloud
- **Preferences**: Configure scan interval, notifications, etc.
- **Start at Login**: Enable/disable automatic startup
- **Quit**: Exit the application

### CLI Mode (Headless)

For servers without a display, use CLI mode:

```bash
# Sign in
cartographer-agent login

# Run a scan
cartographer-agent scan

# Check status
cartographer-agent status

# Sign out
cartographer-agent logout

# Run as background daemon
cartographer-agent --headless
```

## Configuration

### Scan Interval

Default: 5 minutes

You can change the scan interval in Preferences:
- 1 minute
- 5 minutes (default)
- 10 minutes
- 15 minutes
- 30 minutes
- 1 hour

### Start at Login

Enable this option to automatically start the agent when you log in to your computer.

### Notifications

Get notified when:
- New devices are discovered
- Devices go offline
- Network changes are detected

## Network Requirements

The agent requires network access to:
- Scan your local network (ARP/ping)
- Upload results to Cartographer Cloud
- Authenticate with Cartographer Cloud

**Note**: On some systems, network scanning may require elevated privileges. The agent will attempt to use the most appropriate method available.

## Troubleshooting

### "Failed to scan network"

- Ensure you're connected to a network
- On Linux/macOS, ARP scanning may require root privileges
- Try running with elevated permissions if needed

### "Authentication failed"

- Check your internet connection
- Ensure Cartographer Cloud is accessible
- Try signing out and signing in again

### App won't start

- Check that all dependencies are installed
- Review logs in `~/.config/cartographer/` (Linux/macOS) or `%APPDATA%\Cartographer\` (Windows)
- Ensure your system meets the minimum requirements

## Development

### Project Structure

```
cartographer-agent/
├── src/              # Vue.js frontend
├── src-tauri/        # Rust backend
│   ├── src/
│   │   ├── auth/     # Authentication
│   │   ├── cloud/    # Cloud API client
│   │   ├── scanner/  # Network scanning
│   │   ├── scheduler/# Background tasks
│   │   └── tray.rs   # System tray
│   └── Cargo.toml
└── package.json
```

### Running Tests

```bash
# Frontend tests
npm test

# Backend tests
cd src-tauri
cargo test
```
